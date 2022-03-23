use std::{path::Path, str::FromStr};
use std::env;
extern crate home;
extern crate notmuch;
use notmuch::Query;
use notmuch::Sort;
use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::fmt::{Debug, Display};

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Serialize, Deserialize)]
struct Thread {
    id: String,
    date: i64,
    tags: Vec<String>,
    subject: String,
    authors: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    id: String,
    date: i64,
    filename: String,
    tags: Vec<String>,
    subject: String,
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


fn show_message(message: &notmuch::Message) -> Result<()> {
        let ser = Message {
            id: message.id().to_string(),
            date: message.date(),
            filename: message.filename().to_str().unwrap_or("").to_owned(),
            tags: message.tags().collect(),
            subject: message.header("Subject").unwrap().unwrap().to_string()
        };
        let j = serde_json::to_string(&ser)?;
        println!("{}", j);
    Ok(())
}

fn show_messages(db: &notmuch::Database, sort: Sort, str: &str) -> Result<()> {
    let query = db.create_query(&str).unwrap();
    query.set_sort(sort);
    // query.add_tag_exclude(tag)
    let messages = query.search_messages().unwrap();
    for message in messages {
        show_message(&message)?;
    }
    Ok(())
}

fn show_threads(db: &notmuch::Database, sort: Sort, str: &str) -> Result<()> {
    let query = db.create_query(&str).unwrap();
    query.set_sort(sort);
    let threads = query.search_threads().unwrap();
    for thread in threads {
        let ser = Thread {
            id: thread.id().to_string(),
            date: thread.newest_date(),
            tags: thread.tags().collect(),
            subject: thread.subject().to_string(),
            authors: thread.authors(),
        };
        let j = serde_json::to_string(&ser)?;
        println!("{}", j);
    }
    Ok(())
}

// XXX Ordering on both these functions are bad, we get junk in searches
fn show_before_message(db: &notmuch::Database, id: &str, filter: &Option<&str>) -> Result<()>{
    let mut query = format!("thread:{{mid:{}}}", id);
    if let Some(stri) = filter {
        query.push_str(" and ");
        query.push_str(&stri);
    }
    let q = db.create_query(&query).unwrap();
    let messages = q.search_messages().unwrap();
    for message in messages {
        if message.id().to_string() == id.to_string() {
            break;
        }
        show_message(&message)?;
    }
    // }
    Ok(())
}

fn show_after_message(db: &notmuch::Database, id: &str, filter: &Option<&str>) -> Result<()> {
    let mut query = format!("thread:{{mid:{}}}", id);
    if let Some(stri) = filter {
        query.push_str(" and ");
        query.push_str(&stri);
    }
    let q = db.create_query(&query).unwrap();
    let messages = q.search_messages().unwrap();
    let mut seen = false;
    for message in messages {
        if message.id().to_string() == id.to_string() {
            seen = true;
            continue;
        }
        if !seen {
            continue;
        }
        show_message(&message)?;
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


// A bit clunky atm but works using in tools
fn main() -> Result<()>{
    let args = Cli::parse();

    let db = notmuch::Database::open(args.db_path, notmuch::DatabaseMode::ReadOnly).unwrap();
    let sort = from_str(&args.sort);

    match &args.command {
        Commands::Message{search} => show_messages(&db, sort, &search.join(" "))?,
        Commands::Thread{search} => show_threads(&db, sort, &search.join(" "))?,
        Commands::MessageBefore{id, search} => {
            if search.is_empty() {
                show_before_message(&db, id, &None)?
            }
            let args = search.join(" ");
            show_before_message(&db, id, &Some(&args))?
        },
        Commands::MessageAfter{id, search} => {
            if search.is_empty() {
                show_after_message(&db, id, &None)?
            }
            let args = search.join(" ");
            show_after_message(&db, id, &Some(&args))?
        },
    }
    Ok(())
}
