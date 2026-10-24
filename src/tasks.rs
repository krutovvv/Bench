//! Tasks as Bench shows them (the Tasks mode, after Things), kept in Apple Reminders
//! (`crate::reminders`). A reminder carries what Reminders has fields for — title, notes, a
//! day, completion and its date, priority (here: "important", the red flag) — and Bench keeps
//! the rest as plain lines at the end of the notes, readable anywhere:
//!
//! ```text
//! Whatever the notes say.
//!
//! Tags: client-a, Дом
//! When: evening
//! Spent: 1h 30m
//! ```
//!
//! Tags are names as they are (spaces, any language), comma-separated; a line made only of
//! `#words` counts as tags too (typed on the phone). `When: evening` puts a task in Today's
//! "This Evening", `When: someday` in Someday; `Timer: <unix>` keeps a running timer across
//! restarts. An agent writes and reads the same lines.

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Task {
    /// The reminder's identifier ("" for one not saved yet).
    pub id: String,
    pub title: String,
    /// The notes without Bench's lines.
    pub notes: String,
    pub tags: Vec<String>,
    /// The day it is planned for (the reminder's date): in Today from then on.
    pub day: Option<NaiveDate>,
    pub evening: bool,
    pub someday: bool,
    /// The red flag (Reminders' high priority).
    pub important: bool,
    pub completed: Option<NaiveDate>,
    /// Seconds spent, a running timer not counted.
    pub spent: i64,
    /// A running timer's start (Unix seconds).
    pub timer: Option<i64>,
    /// Unix seconds, for a stable order.
    pub created: i64,
    /// The project it belongs to (its id); "" = none.
    pub project: String,
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

impl Task {
    pub fn done(&self) -> bool {
        self.completed.is_some()
    }

    /// Time spent, a running timer counted in.
    pub fn spent_now(&self) -> i64 {
        self.spent + self.timer.map_or(0, |t| (now() - t).max(0))
    }

    pub fn in_list(&self, list: &List, day: NaiveDate) -> bool {
        match list {
            List::Today => !self.done() && self.day.is_some_and(|d| d <= day),
            List::Upcoming => !self.done() && self.day.is_some_and(|d| d > day),
            // "Someday" is not a list of its own (the person found it superfluous): those tasks are here.
            List::Anytime => !self.done() && self.day.is_none_or(|d| d <= day),
            List::Logbook => self.done(),
        }
    }

    /// The notes as stored: the person's text, then Bench's lines.
    #[cfg(test)]
    pub fn stored_notes(&self) -> String {
        let mut lines: Vec<String> = vec![];
        if !self.tags.is_empty() {
            lines.push(format!("Tags: {}", self.tags.join(", ")));
        }
        if self.evening {
            lines.push("When: evening".into());
        } else if self.someday {
            lines.push("When: someday".into());
        }
        if self.spent > 0 {
            lines.push(format!("Spent: {}", show_spent(self.spent)));
        }
        if let Some(t) = self.timer {
            lines.push(format!("Timer: {t}"));
        }
        let text = self.notes.trim_end();
        match (text.is_empty(), lines.is_empty()) {
            (_, true) => text.to_string(),
            (true, false) => lines.join("\n"),
            (false, false) => format!("{text}\n\n{}", lines.join("\n")),
        }
    }

    /// Read Bench's lines back out of stored notes (any line made only of #tags counts,
    /// wherever it is, so tags typed on the phone are picked up too).
    pub fn read_notes(&mut self, stored: &str) {
        let mut keep: Vec<&str> = vec![];
        self.tags.clear();
        self.evening = false;
        self.someday = false;
        self.spent = 0;
        self.timer = None;
        for line in stored.lines() {
            let t = line.trim();
            let words: Vec<&str> = t.split_whitespace().collect();
            if let Some(v) = t.strip_prefix("Tags:") {
                for tag in parse_tags(v) {
                    if !self.tags.iter().any(|x| x.eq_ignore_ascii_case(&tag)) {
                        self.tags.push(tag);
                    }
                }
            } else if let Some(v) = t.strip_prefix("When:") {
                match v.trim().to_lowercase().as_str() {
                    "evening" => self.evening = true,
                    "someday" => self.someday = true,
                    _ => keep.push(line),
                }
            } else if !words.is_empty() && words.iter().all(|w| w.len() > 1 && w.starts_with('#')) {
                for w in words {
                    let tag = &w[1..];
                    match tag.to_lowercase().as_str() {
                        "evening" => self.evening = true,
                        "someday" => self.someday = true,
                        _ => {
                            if !self.tags.iter().any(|x| x.eq_ignore_ascii_case(tag)) {
                                self.tags.push(tag.to_string());
                            }
                        }
                    }
                }
            } else if let Some(v) = t.strip_prefix("Spent:") {
                self.spent = parse_spent(v).unwrap_or(0);
            } else if let Some(v) = t.strip_prefix("Timer:") {
                self.timer = v.trim().parse().ok();
            } else {
                keep.push(line);
            }
        }
        self.notes = keep.join("\n").trim().to_string();
    }
}

/// The lists of the sidebar.
/// The lists in their order (the sidebar's Tasks group).
pub const LISTS: [List; 4] = [List::Today, List::Upcoming, List::Anytime, List::Logbook];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum List {
    Today,
    Upcoming,
    Anytime,
    Logbook,
}

impl List {
    pub fn name(&self) -> String {
        match self {
            List::Today => "Today".into(),
            List::Upcoming => "Upcoming".into(),
            List::Anytime => "Anytime".into(),
            List::Logbook => "Logbook".into(),
        }
    }

    /// Where a task created in this list goes.
    pub fn place(&self, task: &mut Task, day: NaiveDate) {
        match self {
            List::Today => task.day = Some(day),
            List::Upcoming => task.day = Some(day + Duration::days(1)),
            List::Anytime | List::Logbook => {}
        }
    }
}

// ---- dates and durations, as typed and shown -----------------------------------------------

/// "today", "tomorrow", "fri", "+3", "2w", "25.10", "25.10.2026", "2026-10-25" (and the Russian
/// day words) → a date; "" → none.
pub fn parse_date(s: &str, day: NaiveDate) -> Option<Option<NaiveDate>> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return Some(None);
    }
    let weekday = |w: Weekday| {
        let ahead = (w.num_days_from_monday() + 7 - day.weekday().num_days_from_monday()) % 7;
        day + Duration::days(if ahead == 0 { 7 } else { ahead as i64 })
    };
    let d = match s.as_str() {
        "today" | "сегодня" | "tod" => day,
        "tomorrow" | "завтра" | "tom" => day + Duration::days(1),
        "mon" | "monday" | "пн" | "понедельник" => weekday(Weekday::Mon),
        "tue" | "tuesday" | "вт" | "вторник" => weekday(Weekday::Tue),
        "wed" | "wednesday" | "ср" | "среда" => weekday(Weekday::Wed),
        "thu" | "thursday" | "чт" | "четверг" => weekday(Weekday::Thu),
        "fri" | "friday" | "пт" | "пятница" => weekday(Weekday::Fri),
        "sat" | "saturday" | "сб" | "суббота" => weekday(Weekday::Sat),
        "sun" | "sunday" | "вс" | "воскресенье" => weekday(Weekday::Sun),
        _ => {
            if let Some(n) = s.strip_prefix('+').and_then(|n| n.parse::<i64>().ok()) {
                day + Duration::days(n)
            } else if let Some(n) = s.strip_suffix('d').and_then(|n| n.parse::<i64>().ok()) {
                day + Duration::days(n)
            } else if let Some(n) = s.strip_suffix('w').and_then(|n| n.parse::<i64>().ok()) {
                day + Duration::weeks(n)
            } else if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                d
            } else if let Ok(d) = NaiveDate::parse_from_str(&s, "%d.%m.%Y") {
                d
            } else {
                let (dd, mm) = s.split_once('.')?;
                let (dd, mm): (u32, u32) = (dd.parse().ok()?, mm.parse().ok()?);
                let this = NaiveDate::from_ymd_opt(day.year(), mm, dd)?;
                // A day already past this year means next year's.
                if this < day {
                    NaiveDate::from_ymd_opt(day.year() + 1, mm, dd)?
                } else {
                    this
                }
            }
        }
    };
    Some(Some(d))
}

/// "Today", "Tomorrow", "Fri" (within a week), "25 Oct", "25 Oct 2027".
pub fn show_date(d: NaiveDate, day: NaiveDate) -> String {
    let diff = (d - day).num_days();
    match diff {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        2..=6 => d.format("%a").to_string(),
        _ if d.year() == day.year() => d.format("%-d %b").to_string(),
        _ => d.format("%-d %b %Y").to_string(),
    }
}

/// "1h 30m", "90m", "1:30", "45" (minutes), "2h" → seconds; "" → 0.
pub fn parse_spent(s: &str) -> Option<i64> {
    let s = s.trim().to_lowercase().replace(' ', "");
    if s.is_empty() {
        return Some(0);
    }
    if let Some((h, m)) = s.split_once(':') {
        return Some(h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60);
    }
    if let Ok(m) = s.parse::<i64>() {
        return Some(m * 60);
    }
    let mut total = 0;
    let mut num = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' => num.push(c),
            'h' | 'ч' => {
                total += num.parse::<i64>().ok()? * 3600;
                num.clear();
            }
            'm' | 'м' => {
                total += num.parse::<i64>().ok()? * 60;
                num.clear();
            }
            _ => return None,
        }
    }
    num.is_empty().then_some(total)
}

pub fn show_spent(secs: i64) -> String {
    let m = secs / 60;
    match (m / 60, m % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// "work, Client X,  urgent" → ["work", "Client X", "urgent"], no duplicates.
pub fn parse_tags(s: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn dates_as_typed() {
        let day = d("2026-10-23"); // a Friday
        assert_eq!(parse_date("", day), Some(None));
        assert_eq!(parse_date("tomorrow", day), Some(Some(d("2026-10-24"))));
        assert_eq!(parse_date("завтра", day), Some(Some(d("2026-10-24"))));
        assert_eq!(parse_date("mon", day), Some(Some(d("2026-10-26"))));
        assert_eq!(parse_date("fri", day), Some(Some(d("2026-10-30"))));
        assert_eq!(parse_date("+3", day), Some(Some(d("2026-10-26"))));
        assert_eq!(parse_date("2w", day), Some(Some(d("2026-11-06"))));
        assert_eq!(parse_date("25.10", day), Some(Some(d("2026-10-25"))));
        assert_eq!(parse_date("01.02", day), Some(Some(d("2027-02-01"))));
        assert_eq!(parse_date("2026-12-31", day), Some(Some(d("2026-12-31"))));
        assert_eq!(parse_date("nonsense", day), None);
        assert_eq!(show_date(d("2026-10-24"), day), "Tomorrow");
        assert_eq!(show_date(d("2026-10-26"), day), "Mon");
        assert_eq!(show_date(d("2026-11-05"), day), "5 Nov");
    }

    #[test]
    fn spent_as_typed() {
        assert_eq!(parse_spent("1h 30m"), Some(5400));
        assert_eq!(parse_spent("90m"), Some(5400));
        assert_eq!(parse_spent("1:30"), Some(5400));
        assert_eq!(parse_spent("45"), Some(2700));
        assert_eq!(parse_spent("2ч"), Some(7200));
        assert_eq!(parse_spent("abc"), None);
        assert_eq!(show_spent(5400), "1h 30m");
        assert_eq!(show_spent(600), "10m");
        assert_eq!(show_spent(7200), "2h");
    }

    #[test]
    fn notes_keep_benchs_lines() {
        let mut t = Task {
            title: "Report".into(),
            notes: "Call first.\nThen write.".into(),
            tags: vec!["work".into(), "Client X".into()],
            evening: true,
            spent: 5400,
            ..Default::default()
        };
        let stored = t.stored_notes();
        assert_eq!(
            stored,
            "Call first.\nThen write.\n\nTags: work, Client X\nWhen: evening\nSpent: 1h 30m"
        );
        let mut back = Task::default();
        back.read_notes(&stored);
        assert_eq!(back.notes, t.notes);
        assert_eq!(back.tags, vec!["work", "Client X"]);
        assert!(back.evening && back.spent == 5400);
        // Tags typed as a line anywhere count; ordinary text with a # stays text.
        back.read_notes("#home\nBuy milk #2 today\nTags: Домашние дела, home\n");
        assert_eq!(back.tags, vec!["home", "Домашние дела"]);
        assert_eq!(back.notes, "Buy milk #2 today");
        t.tags.clear();
        t.evening = false;
        t.spent = 0;
        assert_eq!(t.stored_notes(), "Call first.\nThen write.");
    }

    #[test]
    fn lists() {
        let day = d("2026-10-23");
        let mut t = Task::default();
        assert!(t.in_list(&List::Anytime, day));
        List::Today.place(&mut t, day);
        assert!(t.in_list(&List::Today, day));
        t.day = Some(d("2026-10-30"));
        assert!(t.in_list(&List::Upcoming, day) && !t.in_list(&List::Today, day));
        t.day = None;
        t.someday = true;
        assert!(t.in_list(&List::Anytime, day));
        t.completed = Some(day);
        assert!(t.in_list(&List::Logbook, day) && !t.in_list(&List::Anytime, day));
    }
}
