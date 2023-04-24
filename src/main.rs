use std::io;
extern crate home;
extern crate notmuch;
use chrono::Duration;
use nm_livesearch::{Result, highlight::Highlight, runtime::{Runtime, Templ}};
use notmuch::Sort;
extern crate chrono;
use regex::*;

use chrono::prelude::*;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

static REGEXSTR: &str = r"\{([^:]*?):?(\d+)?\}";

macro_rules! check_none {
    ( $( $x:expr ),* ) => {
        {
            $(
                if let Some(_) = $x {
                    return false;
                }  
            )*
            true
        }
    }
}

fn check(hl: &Highlight) -> bool {
    check_none!(hl.id, hl.date, hl.num, hl.total, hl.from, hl.subject, hl.tags, hl.matched, hl.excluded)
}

fn empty(hl: Option<Highlight>) -> Option<Highlight> {
    if let Some(ref x) = hl {
        if check(x) {
            return None;
        }
        return hl;
    }
    return None;
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, value_name = "oldest|newest")]
    #[clap(default_value_t = String::from("newest"))]
    sort: String,

    #[clap(long, short)]
    #[clap(default_value_t = 5)]
    humanize_limit: i64,

    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    db_path: Option<PathBuf>,

    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    conf_path: Option<PathBuf>,

    #[clap(short, long)]
    profile: Option<String>,

    #[clap(subcommand)]
    command: Commands,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("{date} [{index:02}/{total:02}] {from:25}│ {subject} ({tags})"))]
    entry_fmt: String,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("{date} [{index:02}/{total:02}] {from:25}│ {response}▶ ({tags})"))]
    response_fmt: String,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("%Y-%m-%d"))]
    date_format: String,

    #[clap(short, long)]
    highlight: Option<String>,
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
        #[clap(required = true)]
        id: String,
        search: Vec<String>,
    },
    MessagesAfter {
        #[clap(required = true)]
        id: String,
        search: Vec<String>,
    },
    ShowTree {
        #[clap(required = true)]
        search: Vec<String>,
    },
    ShowSingleTree {
        #[clap(required = true)]
        search: Vec<String>,
    },
    ShowMessage {
        #[clap(required = true)]
        search: Vec<String>,
    },
    ShowThread {
        #[clap(required = true)]
        search: Vec<String>,
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

fn main() -> Result<()>{
    let args = Cli::parse();

    let db = notmuch::Database::open_with_config(args.db_path, notmuch::DatabaseMode::ReadOnly, args.conf_path, args.profile.as_deref())?;
    let sort = from_str(&args.sort);
    let mut writer = std::io::BufWriter::new(io::stdout().lock());
    let regex = Regex::new(REGEXSTR).unwrap();
    let templ = Templ {
        regex,
        templ_message: &args.entry_fmt,
        templ_respons: &args.response_fmt,
    };
    let highlight: Option<Highlight> = empty(args.highlight.
        map(|x| serde_json::from_str(x.as_ref()).expect("Parsing json highlighting failed")));

    let now = Utc::now();
    let humanize_range = now - Duration::days(args.humanize_limit);

    let runtime = Runtime {
        db,
        templ,
        sort,
        highlight,
        humanize_range,
        date_format: args.date_format,
    };

    match &args.command {
        Commands::Messages{search} => runtime.messages(&search.join(" "), &mut writer)?,
        Commands::Threads{search} => runtime.threads(&search.join(" "), &mut writer)?,
        Commands::ShowTree{search} => runtime.show_thread_tree(&search.join(" "), &mut writer)?,
        Commands::ShowSingleTree{search} => runtime.show_thread_single(&search.join(" "), &mut writer)?,
        Commands::ShowMessage{search} => runtime.show_messages(&search.join(" "), &mut writer)?,
        Commands::ShowThread{search} => runtime.show_threads(&search.join(" "), &mut writer)?,
        Commands::MessagesBefore{id, search} => {
            if search.is_empty() {
                runtime.show_before_message(id, None, &mut writer)?;
                return Ok(())
            }
            let args = search.join(" ");
            runtime.show_before_message(id, Some(&args), &mut writer)?
        },
        Commands::MessagesAfter{id, search} => {
            if search.is_empty() {
                runtime.show_after_message(id, None, &mut writer)?;
                return Ok(())
            }
            let args = search.join(" ");
            runtime.show_after_message(id, Some(&args), &mut writer)?
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::str;
    use notmuch::{Sort, Database};
    use std::io;
    use crate::*;

    static N: u8 = '\n' as u8;
    struct LineCount {
        lines: usize,
    }

    impl LineCount {
        fn new() -> LineCount {
            LineCount { lines: 0 }
        }
    }

    impl io::Write for LineCount {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut i = 0;
            for c in buf {
                if *c == N {
                    self.lines = self.lines + 1;
                }
                i = i + 1;
            }
            Ok(i)
        }
        // do we need to do anything? We don't buffer
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    static TESTSEARCH: &str = "tag:important";

    fn nm_runner(output: &str, nm_search: &str) -> usize {
        let cmd = format!("notmuch search --output={} {} | wc -l", output, nm_search);
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .expect("failed to execute process");
        let s = str::from_utf8(&output.stdout).expect("cmd: failed to parse utf8 string");
        let fixed = s.trim();
        let num: usize = fixed.parse().expect("cmd: Couldn't parse number");
        num
    }
    
    // use the default database
    // this is safe with the correct search, since this program never write
    fn open_db() -> notmuch::Database {
        let db_path: Option<String> = None;
        let config_path: Option<String>  = None;
        let profile: Option<&str>  = None;

        notmuch::Database::open_with_config(db_path, notmuch::DatabaseMode::ReadOnly, config_path, profile).expect("Couldn't open db")
    }
    
    fn mock_runtime<'a>(db: Database) -> Runtime<'a> {
        let regex = Regex::new(REGEXSTR).unwrap();
        let sort = Sort::OldestFirst;
        let templ = Templ {
            regex,
            templ_message: "{date} [{index:02}/{total:02}] {from:25}│ {subject} ({tags})",
            templ_respons: "{date} [{index:02}/{total:02}] {from:25}│ {response}▶ ({tags})",
        };
        let highlight: Option<Highlight> = None;
        let humanize_range = Utc::now() - Duration::days(5);

        let runtime = Runtime {
            db,
            templ,
            sort,
            highlight,
            humanize_range,
            date_format: "%Y-%m-%d".to_string(),
        };

        runtime
    }

    #[test]
    fn messages_num() {
        let num = nm_runner("messages", TESTSEARCH);
        let mut linecounter = LineCount::new();
        let db = open_db();
        let rt = mock_runtime(db);
        rt.messages(TESTSEARCH, &mut linecounter).expect("nm-live: Couldn't fetch messages");
        assert_eq!(num, linecounter.lines);
    }

    #[test]
    fn show_messages_num() {
        let db = open_db();
        let mut linecounter = LineCount::new();
        let rt = mock_runtime(db);
        rt.messages(TESTSEARCH, &mut linecounter).expect("nm-live: Couldn't fetch messages");
        let mut linecounter2 = LineCount::new();
        rt.show_messages(TESTSEARCH, &mut linecounter2).expect("nm-live: Couldn't show messages");
        assert_eq!(linecounter.lines, linecounter2.lines);
    }

    #[test]
    fn thread_num() {
        let num = nm_runner("threads", TESTSEARCH);
        let mut linecounter = LineCount::new();
        let db = open_db();
        let rt = mock_runtime(db);
        rt.threads(TESTSEARCH, &mut linecounter).expect("nm-live: Couldn't fetch messages");
        assert_eq!(num, linecounter.lines);
    }

    #[test]
    fn show_thread_num() {
        let db = open_db();
        let mut linecounter = LineCount::new();
        let rt = mock_runtime(db);
        rt.threads(TESTSEARCH, &mut linecounter).expect("nm-live: Couldn't fetch messages");
        let mut linecounter2 = LineCount::new();
        rt.show_threads(TESTSEARCH, &mut linecounter2).expect("nm-live: Couldn't show messages");
        assert_eq!(linecounter.lines, linecounter2.lines);
    }
}
