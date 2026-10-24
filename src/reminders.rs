//! Tasks live in Apple Reminders (iCloud: the Reminders app, iPhone, Calendar); Bench reads and
//! writes them through EventKit. What Reminders has no field for (tags, time spent, "this
//! evening") Bench keeps as text lines in a reminder's notes.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use block2::RcBlock;
use chrono::{Local, NaiveDate, TimeZone as _};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore, EKReminder};
use objc2_foundation::{
    NSArray, NSDate, NSDateComponentUndefined, NSDateComponents, NSError, NSString,
};

use crate::tasks::Task;

/// What the window asks of Reminders.
pub enum Cmd {
    /// All open tasks and those completed since that day.
    Load {
        done_since: NaiveDate,
    },
    /// Create (no id) or change a task; `token` comes back with a new task's id.
    Save {
        task: Task,
        token: u64,
    },
    Delete(String),
}

pub enum Evt {
    Loaded(Vec<Task>),
    Created { token: u64, id: String },
    Error(String),
}

/// A thread that owns the store (EventKit objects stay on it) and answers commands.
pub struct Reminders {
    tx: mpsc::Sender<Cmd>,
}

impl Reminders {
    pub fn start(evt: async_channel::Sender<Evt>) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::spawn(move || {
            let store = match open() {
                Ok(s) => s,
                Err(e) => {
                    let _ = evt.send_blocking(Evt::Error(format!("{e:#}")));
                    return;
                }
            };
            for cmd in rx {
                let r = match cmd {
                    Cmd::Load { done_since } => load(&store, done_since).map(Evt::Loaded),
                    Cmd::Save { task, token } => {
                        save(&store, &task).map(|id| Evt::Created { token, id })
                    }
                    Cmd::Delete(id) => delete(&store, &id).map(|_| Evt::Created { token: 0, id }),
                };
                let e = r.unwrap_or_else(|e| Evt::Error(format!("{e:#}")));
                if evt.send_blocking(e).is_err() {
                    break;
                }
            }
        });
        Self { tx }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

fn day_of(date: &NSDate) -> NaiveDate {
    let secs = date.timeIntervalSince1970() as i64;
    Local
        .timestamp_opt(secs, 0)
        .single()
        .map(|t| t.date_naive())
        .unwrap_or_default()
}

/// Noon of the day, local time (safe from time zone edges).
fn ns_date(day: NaiveDate) -> Retained<NSDate> {
    let secs = day
        .and_hms_opt(12, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).single())
        .map_or(0, |t| t.timestamp());
    NSDate::dateWithTimeIntervalSince1970(secs as f64)
}

fn to_task(r: &EKReminder) -> Task {
    let mut t = Task {
        id: unsafe { r.calendarItemIdentifier() }.to_string(),
        title: unsafe { r.title() }.to_string(),
        ..Default::default()
    };
    t.read_notes(
        &unsafe { r.notes() }
            .map(|n| n.to_string())
            .unwrap_or_default(),
    );
    t.day = unsafe { r.dueDateComponents() }.and_then(|c| {
        let (y, m, d) = (c.year(), c.month(), c.day());
        (y != NSDateComponentUndefined
            && m != NSDateComponentUndefined
            && d != NSDateComponentUndefined)
            .then(|| NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32))
            .flatten()
    });
    // Reminders' priorities: 1–4 high, 5 medium, 6–9 low, 0 none.
    t.important = (1..=4).contains(&unsafe { r.priority() });
    if unsafe { r.isCompleted() } {
        t.completed =
            Some(unsafe { r.completionDate() }.map_or_else(crate::tasks::today, |d| day_of(&d)));
    }
    t.created = unsafe { r.creationDate() }.map_or(0, |d| d.timeIntervalSince1970() as i64);
    t
}

fn apply(r: &EKReminder, t: &Task) {
    unsafe {
        r.setTitle(Some(&NSString::from_str(&t.title)));
        let notes = t.stored_notes();
        r.setNotes(
            (!notes.is_empty())
                .then(|| NSString::from_str(&notes))
                .as_deref(),
        );
        match t.day {
            Some(day) => {
                let c = NSDateComponents::new();
                c.setYear(chrono::Datelike::year(&day) as isize);
                c.setMonth(chrono::Datelike::month(&day) as isize);
                c.setDay(chrono::Datelike::day(&day) as isize);
                r.setDueDateComponents(Some(&c));
            }
            None => r.setDueDateComponents(None),
        }
        let p = r.priority();
        if t.important {
            r.setPriority(1);
        } else if (1..=4).contains(&p) {
            r.setPriority(0);
        }
        match t.completed {
            Some(day) => {
                if !r.isCompleted() {
                    r.setCompleted(true);
                }
                // Completed on a day of the person's choosing (back-dating works).
                if r.completionDate().is_none_or(|d| day_of(&d) != day) {
                    r.setCompletionDate(Some(&ns_date(day)));
                }
            }
            None => {
                if r.isCompleted() {
                    r.setCompleted(false);
                }
            }
        }
    }
}

fn find(store: &EKEventStore, id: &str) -> Option<Retained<EKReminder>> {
    let item = unsafe { store.calendarItemWithIdentifier(&NSString::from_str(id)) }?;
    item.downcast::<EKReminder>().ok()
}

fn load(store: &EKEventStore, done_since: NaiveDate) -> Result<Vec<Task>> {
    let open = unsafe {
        store.predicateForIncompleteRemindersWithDueDateStarting_ending_calendars(None, None, None)
    };
    let done = unsafe {
        store.predicateForCompletedRemindersWithCompletionDateStarting_ending_calendars(
            Some(&ns_date(done_since - chrono::Duration::days(1))),
            None,
            None,
        )
    };
    let mut out: Vec<Task> = fetch(store, &open)?.iter().map(|r| to_task(r)).collect();
    out.extend(fetch(store, &done)?.iter().map(|r| to_task(r)));
    out.sort_by_key(|t| t.created);
    Ok(out)
}

/// Bench never adds alarms: a task in Reminders does not ring on the phone unless you set one.
fn save(store: &EKEventStore, t: &Task) -> Result<String> {
    let r = if t.id.is_empty() {
        let r = unsafe { EKReminder::reminderWithEventStore(store) };
        let list = unsafe { store.defaultCalendarForNewReminders() }
            .ok_or_else(|| anyhow!("Reminders has no default list"))?;
        unsafe { r.setCalendar(Some(&list)) };
        r
    } else {
        find(store, &t.id).ok_or_else(|| anyhow!("the task is no longer in Reminders"))?
    };
    apply(&r, t);
    unsafe { store.saveReminder_commit_error(&r, true) }
        .map_err(|e| anyhow!("{}", e.localizedDescription()))?;
    Ok(unsafe { r.calendarItemIdentifier() }.to_string())
}

fn delete(store: &EKEventStore, id: &str) -> Result<()> {
    let r = find(store, id).ok_or_else(|| anyhow!("the task is no longer in Reminders"))?;
    unsafe { store.removeReminder_commit_error(&r, true) }
        .map_err(|e| anyhow!("{}", e.localizedDescription()))?;
    Ok(())
}

/// The store, with full access to reminders granted (macOS asks once, for the signed app).
pub fn open() -> Result<Retained<EKEventStore>> {
    let store = unsafe { EKEventStore::new() };
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) };
    if status != EKAuthorizationStatus::FullAccess {
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
            let _ = tx.send(granted.as_bool());
        });
        unsafe {
            store.requestFullAccessToRemindersWithCompletion(RcBlock::as_ptr(&done));
        }
        let granted = rx
            .recv_timeout(Duration::from_secs(120))
            .map_err(|_| anyhow!("no answer to the Reminders permission request"))?;
        if !granted {
            bail!(
                "access to Reminders was not allowed (System Settings → Privacy & Security → Reminders)"
            );
        }
    }
    Ok(store)
}

/// Run a fetch and wait for its result.
fn fetch(
    store: &EKEventStore,
    predicate: &objc2_foundation::NSPredicate,
) -> Result<Vec<Retained<EKReminder>>> {
    let (tx, rx) = mpsc::channel();
    let done = RcBlock::new(move |found: *mut NSArray<EKReminder>| {
        let list = unsafe { found.as_ref() }
            .map(|a| a.to_vec())
            .unwrap_or_default();
        let _ = tx.send(list);
    });
    unsafe {
        store.fetchRemindersMatchingPredicate_completion(predicate, &done);
    }
    rx.recv_timeout(Duration::from_secs(60))
        .map_err(|_| anyhow!("Reminders did not answer"))
}

/// `--probe-reminders`: access, lists, open and recently completed reminders, with timings.
pub fn probe() -> Result<String> {
    let mut out = String::new();
    let t = Instant::now();
    let store = open()?;
    out.push_str(&format!("access: {:?}\n", t.elapsed()));

    let lists = unsafe { store.calendarsForEntityType(EKEntityType::Reminder) };
    let names: Vec<String> = lists
        .iter()
        .map(|c| unsafe { c.title() }.to_string())
        .collect();
    out.push_str(&format!("lists ({}): {}\n", names.len(), names.join(", ")));

    let t = Instant::now();
    let all_open = unsafe {
        store.predicateForIncompleteRemindersWithDueDateStarting_ending_calendars(None, None, None)
    };
    let open = fetch(&store, &all_open)?;
    out.push_str(&format!("open: {} in {:?}\n", open.len(), t.elapsed()));

    let t = Instant::now();
    let month_ago = NSDate::dateWithTimeIntervalSinceNow(-30. * 86400.);
    let recent = unsafe {
        store.predicateForCompletedRemindersWithCompletionDateStarting_ending_calendars(
            Some(&month_ago),
            None,
            None,
        )
    };
    let done = fetch(&store, &recent)?;
    out.push_str(&format!(
        "completed in 30 days: {} in {:?}\n",
        done.len(),
        t.elapsed()
    ));

    for r in open.iter().take(5) {
        let title = unsafe { r.title() }.to_string();
        let due = unsafe { r.dueDateComponents() }
            .map(|c| format!("{}-{:02}-{:02}", c.year(), c.month(), c.day()))
            .unwrap_or_else(|| "no date".into());
        let alarms = unsafe { r.hasAlarms() };
        out.push_str(&format!("  · {title} [{due}] alarms={alarms}\n"));
    }
    Ok(out)
}
