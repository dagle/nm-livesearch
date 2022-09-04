use std::{borrow::Cow, io};
use std::{error, fmt, result};
extern crate home;
extern crate notmuch;
use notmuch::Sort;
use serde::Serialize;
use serde::ser::SerializeStruct;
use std::fmt::Debug;
extern crate chrono;

use chrono::{format::{DelayedFormat, StrftimeItems}, prelude::*};
use std::collections::BinaryHeap;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
enum Error {
    SerdeErr(serde_json::Error),
    NmError(notmuch::Error),
    IoError(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SerdeErr(e) => <serde_json::Error as fmt::Display>::fmt(e, f),
            Error::NmError(e) => <notmuch::Error as fmt::Display>::fmt(e, f),
            Error::IoError(e) => <io::Error as fmt::Display>::fmt(e,f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::SerdeErr(e) => Some(e),
            Error::NmError(e) => Some(e),
            Error::IoError(e) => Some(e),
        }
    }
}

impl std::convert::From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err)
    }
}

impl std::convert::From<notmuch::Error> for Error {
    fn from(err: notmuch::Error) -> Error {
        Error::NmError(err)
    }
}

impl std::convert::From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error::SerdeErr(err)
    }
}

fn show_time<'a>(date: i64) -> DelayedFormat<StrftimeItems<'a>> {
    let naive = NaiveDateTime::from_timestamp(date, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    datetime.format("%Y-%m-%d")
}

fn fix_subject(sub: &str) -> String {
    sub.chars()
        .map(|x| match x { 
            '\r' => ' ', 
            '\n' => ' ',
            _ => x
        }).collect()
}

fn show_threads<W>(db: &notmuch::Database, sort: Sort, search: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(&search)?;
    query.set_sort(sort);
    let threads = query.search_threads()?;
    for thread in threads {
        let id = thread.id();
        let subject = thread.subject();
        let subfixed = fix_subject(&subject);
        let authors = thread.authors();
        let total = thread.total_messages();
        let matched = thread.matched_messages();
        let date = thread.newest_date();
        let newdate = show_time(date);
        let tags: Vec<String> = thread.tags().collect();
        let str = format!("{} [{:02}/{:02}] {:25}│ {} ({})", newdate, matched, total, authors.join(" "), subfixed, tags.join(","));
        let tuple = (id, str);
        serde_json::to_writer(&mut *writer, &tuple)?;
        write!(writer,"\n")?;
    }
    Ok(())
}

struct OrderMessage(i64, Sort, (String, String, bool));

impl PartialEq for OrderMessage {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl Eq for OrderMessage {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialOrd for OrderMessage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.1 == Sort::OldestFirst {
            return other.0.partial_cmp(&self.0)
        }
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderMessage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.1 == Sort::OldestFirst {
            return other.0.cmp(&self.0)
        }
        self.0.cmp(&other.0)
    }
}

fn flush_messages<W>(heap: &mut BinaryHeap<OrderMessage>, sort: Sort, reference: i64, writer: &mut W) -> Result<()>
    where W: io::Write {
    loop {
        match heap.peek() {
            Some(value) => {
                if !compare_diff(value.0, reference, sort) {
                    let top = heap.pop().unwrap();
                    serde_json::to_writer(&mut *writer, &top.2)?;
                    write!(writer,"\n")?;
                } else {
                    break;
                }
            }
            None => {
                break;
            }
        }
    }
    Ok(())
}

fn compare_time(thread: &notmuch::Thread, sort: Sort) -> i64 {
    match sort {
        Sort::OldestFirst => {
            thread.oldest_date()
        }
        _ => {
            thread.newest_date()
        }
    }
}

fn compare_diff(current: i64, reference: i64, sort: Sort) -> bool {
    match sort {
        Sort::OldestFirst => {
            current < reference 
        }
        _ => {
            current > reference 
        }
    }
}

fn show_messages<W>(db: &notmuch::Database, sort: Sort, search: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(&search)?;
    query.set_sort(sort);
    let threads = query.search_threads()?;
    let mut heap = BinaryHeap::new();
    for thread in threads {
        let messages = thread.messages();
        let reference = compare_time(&thread, sort);
        let total = thread.total_messages();
        flush_messages(&mut heap, sort, reference, writer)?;
        let mut counter = 0;
        for message in messages {
            counter = counter + 1;
            let matched = message.get_flag(notmuch::MessageFlag::Match);
            if !matched {
                continue;
            }
            let id = message.id();
            let subject = message.header("Subject")?.unwrap_or_default();
            let subfixed = fix_subject(&subject);
            let tags: Vec<String> = message.tags().collect();
            let from = message.header("From")?.unwrap_or_default();
            let date = message.date();
            let newdate = show_time(date);
            let str = format!("{} [{:02}/{:02}] {:25}│ {} ({})", newdate, counter, total, from, subfixed, tags.join(","));
            let tuple = (id.to_string(), str, matched);
            if compare_diff(date, reference, sort) {
                let om = OrderMessage(date, sort, tuple);
                heap.push(om)
            } else {
                serde_json::to_writer(&mut *writer, &tuple)?;
                write!(writer,"\n")?;
            }
        }
    }
    Ok(())
}

fn show_message_tree(messages: &Vec<notmuch::Message>, level: i32, prestring: String, num: i32, total: i32, vec: &mut Vec<(String, String, bool)>) -> Result<i32>
    {
    let mut j = 1;
    let length = messages.len();
    let mut n = num;
    for message in messages {
        let mut newstring: String = prestring.clone();
        if n == 0 {
        } else if j == length {
            newstring.push_str("└─")
        } else {
            newstring.push_str("├─")
        }

        let replies = message.replies();
        let replies_vec: Vec<notmuch::Message> = replies.collect();

        if replies_vec.len()  > 0 {
            newstring.push_str("┬")
        } else {
            newstring.push_str("─")
        }

        let tags: Vec<String> = message.tags().collect();
        let from = message.header("From")?.unwrap_or_default();
        let subject = message.header("Subject")?.unwrap_or_default();
        let subfixed = fix_subject(&subject);
        let matched = message.get_flag(notmuch::MessageFlag::Match);
        let id = message.id();
        let date = message.date();
        let newdate = show_time(date);

        if level > 0 && n > 0 {
            let str = format!("{} [{:02}/{:02}] {:25}│ {}▶ ({})", newdate, n+1, total, from, newstring, tags.join(","));
            let tuple = (id.to_string(), str, matched);
            vec.push(tuple)
        } else {
            let str = format!("{} [{:02}/{:02}] {:25}│ {} ({})", newdate, n+1, total, from, subfixed, tags.join(","));
            let tuple = (id.to_string(), str, matched);
            vec.push(tuple)
        }

        let mut newstring: String = prestring.clone();
        if n == 0 {
        } else if length > j {
            newstring.push_str("│ ")
        } else {
            newstring.push_str("  ")
        }
        n = show_message_tree(&replies_vec, level + 1, newstring, n + 1, total, vec)?;
        j += 1;
    }
    Ok(n)
}

fn show_thread_tree<W>(db: &notmuch::Database, sort: Sort, search: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(search)?;
    query.set_sort(sort);
    let threads = query.search_threads()?;
    for thread in threads {
        let total = thread.total_messages();
        let messages = thread.toplevel_messages();
        let mut vec = Vec::new();
        let mvec = messages.collect();
        show_message_tree(&mvec, 0, "".to_string(), 0, total, &mut vec)?;
        serde_json::to_writer(&mut *writer, &vec)?;
        write!(writer,"\n")?;
    }
    Ok(())
}

struct Thread<'a>(&'a notmuch::Thread);

impl<'a> Serialize for Thread<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
            let mut mes = serializer.serialize_struct("Message", 5)?;
            let id = self.0.id();
            mes.serialize_field("id", &id)?;
            let date = self.0.newest_date();
            mes.serialize_field("date", &date)?;
            let tags: Vec<String> = self.0.tags().collect();
            mes.serialize_field("tags", &tags)?;
            let authors = self.0.authors();
            mes.serialize_field("authors", &authors)?;
            let subject = self.0.subject();
            mes.serialize_field("subject", &subject)?;
            mes.end()
    }
}

struct Message(notmuch::Message, i32, i32);

// TODO currently we are just using Subject and id
impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
            use serde::ser::Error;
            let mut mes = serializer.serialize_struct("Message", 7)?;
            let id = self.0.id();
            mes.serialize_field("id", &id)?;
            let date = self.0.date();
            mes.serialize_field("date", &date)?;
            let filename = self.0.filename();
            mes.serialize_field("filename", &filename)?;
            let filenames: Vec<PathBuf> = self.0.filenames().collect();
            mes.serialize_field("filenames", &filenames)?;
            let tags: Vec<String> = self.0.tags().collect();
            mes.serialize_field("tags", &tags)?;
            let from = self.0.header("From").map_err(Error::custom)?.unwrap_or_else(|| Cow::from(""));
            mes.serialize_field("from", &from)?;
            let subject = self.0.header("Subject").map_err(Error::custom)?.unwrap_or_else(|| Cow::from(""));
            mes.serialize_field("subject", &subject)?;
            let tid = self.0.thread_id();
            mes.serialize_field("tid", &tid)?;
            let index = self.1;
            mes.serialize_field("index", &index)?;
            let total = self.2;
            mes.serialize_field("total", &total)?;
            let keys : Vec<(String, String)> =  self.0.properties("session-key", true).collect();
            mes.serialize_field("keys", &keys)?;
            mes.end()
    }
}

fn from_str(s: &str) -> Sort {
        match s {
            "oldest" => Sort::OldestFirst,
            "newest" => Sort::NewestFirst,
            "message-id" => Sort::MessageID,
            "unsorted" => Sort::Unsorted,
            _ => panic!("Bad sort option")
        }
}

fn show_message<W>(message: &Message, writer: &mut W) -> Result<()> 
    where W: io::Write {
    serde_json::to_writer(&mut *writer, message)?;
    write!(writer,"\n")?;
    Ok(())
}

fn show_thread<W>(thread: &notmuch::Thread, writer: &mut W) -> Result<()> 
    where W: io::Write {
    let ser = Thread(&thread);
    serde_json::to_writer(&mut *writer, &ser)?;
    write!(writer, "\n")?;
    Ok(())
}

fn messages<W>(db: &notmuch::Database, sort: Sort, str: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(str)?;
    query.set_sort(sort);
    let messages = query.search_messages()?;
    for message in messages {
        let ser = Message(message.clone(), 1, 1);
        show_message(&ser, writer)?;
    }
    Ok(())
}

fn threads<W>(db: &notmuch::Database, sort: Sort, str: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(str)?;
    query.set_sort(sort);
    let threads = query.search_threads()?;
    for thread in threads {
        show_thread(&thread, writer)?;
    }
    Ok(())
}

fn show_before_message<W>(db: &notmuch::Database, sort: Sort, id: &str, filter: &Option<&str>, writer: &mut W) -> Result<()>
    where W: io::Write {
    let mut query = format!("thread:{{mid:{}}}", id);
    if let Some(str) = filter {
        query.push_str(" and ");
        query.push_str(str);
    }
    let q = db.create_query(&query)?;
    q.set_sort(sort);
    let messages = q.search_messages()?;
    for message in messages {
        if message.id() == id {
            break;
        }
        let ser = Message(message.clone(), 1, 1);
        show_message(&ser, writer)?;
    }
    Ok(())
}

fn show_after_message<W>(db: &notmuch::Database, sort: Sort, id: &str, filter: &Option<&str>, writer: &mut W) -> Result<()>
    where W: io::Write {
    let mut query = format!("thread:{{mid:{}}}", id);
    if let Some(str) = filter {
        query.push_str(" and ");
        query.push_str(str);
    }
    let q = db.create_query(&query)?;
    q.set_sort(sort);
    let messages = q.search_messages()?;
    let mut seen = false;
    for message in messages {
        if message.id() == id {
            seen = true;
            continue;
        }
        if !seen {
            continue;
        }
        let ser = Message(message.clone(), 1, 1);
        show_message(&ser, writer)?;
    }
    Ok(())
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, value_name = "oldest|newest")]
    #[clap(default_value_t = String::from("newest"))]
    sort: String,

    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    db_path: Option<PathBuf>,

    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    conf_path: Option<PathBuf>,

    #[clap(short, long)]
    profile: Option<String>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Messages {
        #[clap(required = true)]
        search: Vec<String>,
    },
    Threads {
        #[clap(required = true)]
        search: Vec<String>,
    },
    MessagesBefore {
        id: String,
        search: Vec<String>,
    },
    MessagesAfter {
        id: String,
        search: Vec<String>,
    },
    ShowTree {
        search: Vec<String>,
    },
    ShowMessage {
        search: Vec<String>,
    },
    ShowThread {
        search: Vec<String>,
    }
}


fn main() -> Result<()>{
    let args = Cli::parse();

    let db = notmuch::Database::open_with_config(args.db_path, notmuch::DatabaseMode::ReadOnly, args.conf_path, args.profile.as_deref())?;
    let sort = from_str(&args.sort);
    let mut writer = std::io::BufWriter::new(io::stdout().lock());

    match &args.command {
        Commands::Messages{search} => messages(&db, sort, &search.join(" "), &mut writer)?,
        Commands::Threads{search} => threads(&db, sort, &search.join(" "), &mut writer)?,
        Commands::ShowTree{search} => show_thread_tree(&db, sort, &search.join(" "), &mut writer)?,
        Commands::ShowMessage{search} => show_messages(&db, sort, &search.join(" "), &mut writer)?,
        Commands::ShowThread{search} => show_threads(&db, sort, &search.join(" "), &mut writer)?,
        Commands::MessagesBefore{id, search} => {
            if search.is_empty() {
                show_before_message(&db, sort, id, &None, &mut writer)?;
                return Ok(())
            }
            let args = search.join(" ");
            show_before_message(&db, sort, id, &Some(&args), &mut writer)?
        },
        Commands::MessagesAfter{id, search} => {
            if search.is_empty() {
                show_after_message(&db, sort, id, &None, &mut writer)?;
                return Ok(())
            }
            let args = search.join(" ");
            show_after_message(&db, sort, id, &Some(&args), &mut writer)?
        },
    }
    Ok(())
}
