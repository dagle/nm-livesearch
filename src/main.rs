use std::path::Path;
use std::env;
extern crate home;
extern crate notmuch;
use notmuch::Query;
use serde::{Deserialize, Serialize};
use serde_json::Result;

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

fn show_messages(db: &notmuch::Database, str: &str) -> Result<()> {
    let query = db.create_query(&str).unwrap();
    let messages = query.search_messages().unwrap();
    for message in messages {
        show_message(&message)?;
    }
    Ok(())
}

fn show_threads(db: &notmuch::Database, str: &str) -> Result<()> {
    let query = db.create_query(&str).unwrap();
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

// A bit clunky atm but works using in tools
fn main() -> Result<()>{
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("nm needs 3 argumnts or more")
    }
    let none: Option<&Path> = None;
    let db = notmuch::Database::open(none, notmuch::DatabaseMode::ReadOnly).unwrap();
    match args[1].as_str() {
        "message" => show_messages(&db, &args[2..].join(" "))?,
        "thread" => show_threads(&db, &args[2..].join(" "))?,
        "message-before" => {
            if args.len() > 3 {
                let str = args[3..].join(" ");
                if str != "" {
                    show_before_message(&db, &args[2], &Some(&str))?;
                    return Ok(())
                }
            }
            show_before_message(&db, &args[2], &None)?
        },
        "message-after" => {
            if args.len() > 3 {
                let str = args[3..].join(" ");
                if str != "" {
                    show_after_message(&db, &args[2], &Some(&str))?;
                    return Ok(())
                }
            }
            show_after_message(&db, &args[2], &None)?
        },
        _ => {
            println!("Need a print mode:message|thread|thread-messages|message-before|message-after");
            return Ok(()) 
        }
    }
    Ok(())
}
