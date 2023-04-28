use chrono::{format::{DelayedFormat, StrftimeItems}, NaiveDateTime, DateTime, Utc};
use notmuch::Sort;

pub fn show_time<'a>(date: i64, date_format: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
    let naive = NaiveDateTime::from_timestamp_opt(date, 0).expect("Couldn't parse datetime");
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    datetime.format(date_format)
}

pub fn compare_time(thread: &notmuch::Thread, sort: Sort) -> i64 {
    match sort {
        Sort::OldestFirst => {
            thread.oldest_date()
        }
        _ => {
            thread.newest_date()
        }
    }
}

pub fn compare_diff(current: i64, reference: i64, sort: Sort) -> bool {
    match sort {
        Sort::OldestFirst => {
            current < reference 
        }
        _ => {
            current > reference 
        }
    }
}
