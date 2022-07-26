use std::{borrow::Cow, io};
use std::{error, fmt, result};
extern crate home;
extern crate notmuch;
use notmuch::Sort;
use serde::Serialize;
use serde::ser::SerializeStruct;
use std::fmt::Debug;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
enum Error {
    SerdeErr(serde_json::Error),
    NmError(notmuch::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SerdeErr(e) => <serde_json::Error as fmt::Display>::fmt(e, f),
            Error::NmError(e) => <notmuch::Error as fmt::Display>::fmt(e, f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::SerdeErr(e) => Some(e),
            Error::NmError(e) => Some(e),
        }
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

struct Message<'a>(&'a notmuch::Message);

impl<'a> Serialize for Message<'a> {
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
            mes.serialize_field("filnames", &filenames)?;
            let tags: Vec<String> = self.0.tags().collect();
            mes.serialize_field("tags", &tags)?;
            let from = self.0.header("From").map_err(Error::custom)?.unwrap_or_else(|| Cow::from(""));
            mes.serialize_field("from", &from)?;
            let subject = self.0.header("Subject").map_err(Error::custom)?.unwrap_or_else(|| Cow::from(""));
            mes.serialize_field("subject", &subject)?;
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

fn show_message<W>(message: &notmuch::Message, writer: &mut W) -> Result<()> 
    where W: io::Write {
    let ser = Message(message);
    serde_json::to_writer(writer, &ser)?;
    Ok(())
}

fn show_thread<W>(thread: &notmuch::Thread, writer: &mut W) -> Result<()> 
    where W: io::Write {
    let ser = Thread(&thread);
    serde_json::to_writer(writer, &ser)?;
    Ok(())
}

fn show_messages<W>(db: &notmuch::Database, sort: Sort, str: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(str)?;
    query.set_sort(sort);
    let messages = query.search_messages()?;
    for message in messages {
        show_message(&message, writer)?;
    }
    Ok(())
}

fn show_threads<W>(db: &notmuch::Database, sort: Sort, str: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
    let query = db.create_query(str)?;
    query.set_sort(sort);
    let threads = query.search_threads()?;
    for thread in threads {
        show_thread(&thread, writer)?;
    }
    Ok(())
}

fn show_before_message<W>(db: &notmuch::Database, id: &str, filter: &Option<&str>, writer: &mut W) -> Result<()>
    where W: io::Write {
    let mut query = format!("thread:{{mid:{}}}", id);
    if let Some(str) = filter {
        query.push_str(" and ");
        query.push_str(str);
    }
    let q = db.create_query(&query)?;
    let messages = q.search_messages()?;
    for message in messages {
        if message.id() == id {
            break;
        }
        show_message(&message, writer)?;
    }
    Ok(())
}

fn show_after_message<W>(db: &notmuch::Database, id: &str, filter: &Option<&str>, writer: &mut W) -> Result<()>
    where W: io::Write {
    let mut query = format!("thread:{{mid:{}}}", id);
    if let Some(str) = filter {
        query.push_str(" and ");
        query.push_str(str);
    }
    let q = db.create_query(&query)?;
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
        show_message(&message, writer)?;
    }
    Ok(())
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, value_name = "oldest|newest")]
    #[clap(default_value_t = String::from("oldest"))]
    sort: String,

    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    db_path: Option<PathBuf>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Message {
        #[clap(required = true)]
        search: Vec<String>,
    },
    Thread {
        #[clap(required = true)]
        search: Vec<String>,
    },
    MessageBefore {
        id: String,
        search: Vec<String>,
    },
    MessageAfter {
        id: String,
        search: Vec<String>,
    },
}


fn main() -> Result<()>{
    let args = Cli::parse();

    let db = notmuch::Database::open(args.db_path, notmuch::DatabaseMode::ReadOnly)?;
    let sort = from_str(&args.sort);
    let mut writer = std::io::BufWriter::new(io::stdout().lock());

    match &args.command {
        Commands::Message{search} => show_messages(&db, sort, &search.join(" "), &mut writer)?,
        Commands::Thread{search} => show_threads(&db, sort, &search.join(" "), &mut writer)?,
        Commands::MessageBefore{id, search} => {
            if search.is_empty() {
                show_before_message(&db, id, &None, &mut writer)?;
                return Ok(())
            }
            let args = search.join(" ");
            show_before_message(&db, id, &Some(&args), &mut writer)?
        },
        Commands::MessageAfter{id, search} => {
            if search.is_empty() {
                show_after_message(&db, id, &None, &mut writer)?;
                return Ok(())
            }
            let args = search.join(" ");
            show_after_message(&db, id, &Some(&args), &mut writer)?
        },
    }
    Ok(())
}
