use std::{env, path::PathBuf};
extern crate home;
extern crate notmuch;
// extern crate xdg;
// use notmuch::Query;
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

fn show_messages(query: &notmuch::Query) -> Result<()> {
    let messages = query.search_messages().unwrap();
    for message in messages {
        let ser = Message {
            id: message.id().to_string(),
            date: message.date(),
            filename: message.filename().to_str().unwrap_or("").to_owned(),
            tags: message.tags().collect(),
            subject: message.header("Subject").unwrap().unwrap().to_string()
        };
        let j = serde_json::to_string(&ser)?;
        println!("{}", j);
    }
    Ok(())
}

fn show_threads(query: &notmuch::Query) -> Result<()> {
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
fn dbpath() -> PathBuf {
    let mut mail_path = home::home_dir().unwrap();
    mail_path.push("mail");
    // let dirs = xdg::BaseDirectories::with_profile("notmuch", "default").unwrap();
    // let conf_path = match dirs.find_config_file("config") {
    //     Some(path) => path,
    //     // $HOME/.notmuch-config
    //     None => PathBuf::new(),
    // };
    return mail_path
}

fn main() -> Result<()>{
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("nm needs 3 argumnts or more")
    }
    let mail_path = dbpath();
    let db = notmuch::Database::open(&mail_path, notmuch::DatabaseMode::ReadOnly).unwrap();
    let query = db.create_query(&args[2..].join(" ")).unwrap();
    match args[1].as_str() {
        "message" => show_messages(&query)?,
        "thread" => show_threads(&query)?,
        _ => {
            println!("Need a print mode:message|thread");
            return Ok(()) 
        }
    }
    Ok(())
}
