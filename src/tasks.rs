//! A task as Bench shows it (after Things): title, notes, tags, a day, "this evening" or
//! "someday", the red flag, completion, time spent and a running timer. Kept by the agent in
//! bench.sqlite (`crate::task_store`).

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Task {
    /// The store's identifier ("" for one not saved yet).
    pub id: String,
    pub title: String,
    /// The notes without Bench's lines.
    pub notes: String,
    pub tags: Vec<String>,
    /// The day it is planned for: in Today from then on.
    pub day: Option<NaiveDate>,
    pub evening: bool,
    pub someday: bool,
    /// The red flag.
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

/// A project's tag: its name in Latin letters, lower case, words joined by "-" ("Ассистент ТГ"
/// → "assistent-tg"), as a slug. The agent's `slug` (builtin/tasks.ts) is the same.
pub fn slug(name: &str) -> String {
    let latin: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            match c {
                'а' => "a",
                'б' => "b",
                'в' => "v",
                'г' => "g",
                'д' => "d",
                'е' | 'ё' => "e",
                'ж' => "zh",
                'з' => "z",
                'и' => "i",
                'й' => "y",
                'к' => "k",
                'л' => "l",
                'м' => "m",
                'н' => "n",
                'о' => "o",
                'п' => "p",
                'р' => "r",
                'с' => "s",
                'т' => "t",
                'у' => "u",
                'ф' => "f",
                'х' => "h",
                'ц' => "ts",
                'ч' => "ch",
                'ш' => "sh",
                'щ' => "sch",
                'ъ' | 'ь' => "",
                'ы' => "y",
                'э' => "e",
                'ю' => "yu",
                'я' => "ya",
                c if c.is_ascii_alphanumeric() => return c.to_string(),
                _ => "-",
            }
            .to_string()
        })
        .collect();
    latin
        .split('-')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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

    #[test]
    fn slugs() {
        assert_eq!(slug("Ассистент ТГ"), "assistent-tg");
        assert_eq!(slug("Skorozvon"), "skorozvon");
        assert_eq!(slug("  Deep Research! "), "deep-research");
        assert_eq!(slug("Щука и ёж"), "schuka-i-ezh");
    }

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
