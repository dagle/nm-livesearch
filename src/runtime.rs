use std::{io, collections::BinaryHeap};

use chrono::{DateTime, Utc, NaiveDateTime};
use chrono_humanize::HumanTime;
use notmuch::{Database, Sort, Messages};
use regex::{Regex, Captures};
use serde::Serialize;

use crate::{Result, message::Message, highlight::Highlight, thread::Thread, time::{compare_time, compare_diff, show_time}, ordered::{flush_messages, OrderMessage}};

pub struct Runtime<'a> {
    pub db: Database,
    pub templ: Templ<'a>,
    pub sort: Sort,
    pub highlight: Option<Highlight>,
    pub date_format: String,
    pub humanize_range: DateTime<Utc>,
}

pub struct Templ<'a> {
    pub regex: Regex,
    pub templ_message: &'a str,
    pub templ_respons: &'a str,
}


#[derive(Serialize, Debug)]
pub struct Show {
    id: String,
    entry: String,
    highlight: bool,
}

fn fix_subject(sub: &str) -> String {
    sub.chars()
        .map(|x| match x { 
            '\r' => ' ', 
            '\n' => ' ',
            _ => x
        }).collect()
}

fn show_thread<W>(thread: &notmuch::Thread, writer: &mut W) -> Result<()> 
    where W: io::Write {
    let ser = Thread(&thread);
    serde_json::to_writer(&mut *writer, &ser)?;
    write!(writer, "\n")?;
    Ok(())
}

impl<'a> Runtime<'a> {
    fn humanize(&self, date: i64, pad: usize) -> String {
        // let naive = NaiveDateTime::from_timestamp(date, 0);
        let naive = NaiveDateTime::from_timestamp_opt(date, 0).expect("Couldn't humanize string");
        let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);

        if datetime > self.humanize_range {
            let ht = HumanTime::from(datetime);
            format!("{:pad$}", ht)
        } else {
            format!("{:pad$}", datetime.format(&self.date_format))
        }
    }

    pub fn messages<W>(&self, str: &str, writer: &mut W) -> Result<()>
        where W: io::Write {
            let query = self.db.create_query(str)?;
            query.set_sort(self.sort);
            let messages = query.search_messages()?;
            for message in messages {
                let mes = Message(message.clone(), 1, 1);
                mes.show_message(writer)?;
            }
            Ok(())
    }

    pub fn threads<W>(&self, str: &str, writer: &mut W) -> Result<()>
        where W: io::Write {
            let query = self.db.create_query(str)?;
            query.set_sort(self.sort);
            let threads = query.search_threads()?;
            for thread in threads {
                show_thread(&thread, writer)?;
            }
            Ok(())
    }

    pub fn show_thread_tree<W>(&self, search: &str, writer: &mut W) -> Result<()>
        where W: io::Write {
            let query = self.db.create_query(search)?;
            query.set_sort(self.sort);
            let threads = query.search_threads()?;
            for thread in threads {
                let total = thread.total_messages();
                let messages = thread.toplevel_messages();
                let mut vec = Vec::new();
                let mvec: Vec<notmuch::Message> = messages.collect();
                self.show_message_tree(&mvec, 0, "".to_string(), 0, total, &mut vec)?;
                serde_json::to_writer(&mut *writer, &vec)?;
                write!(writer,"\n")?;
            }
            Ok(())
    }

    fn show_message_tree(&self, messages: &[notmuch::Message], level: i32, prestring: String, num: i32, total: i32, vec: &mut Vec<Show>) -> Result<i32>
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

            let id = message.id();


            if level > 0 && n > 0 {
                let highlight = self.highlight.as_ref().map_or(Ok(false), |hl| hl.message(message, n+1, total))?;
                let str = self.template_message(&self.templ.templ_respons, &message, Some(newstring), n+1, total)?;
                let show = Show { id: id.to_string(), entry: str, highlight};
                vec.push(show)
            } else {
                let highlight = self.highlight.as_ref().map_or(Ok(false), |hl| hl.message(message, n+1, total))?;
                let str = self.template_message(&self.templ.templ_message, &message, None, n+1, total)?;
                let show = Show { id: id.to_string(), entry: str, highlight};
                vec.push(show)
            }

            let mut newstring: String = prestring.clone();
            if n == 0 {
            } else if length > j {
                newstring.push_str("│ ")
            } else {
                newstring.push_str("  ")
            }
            n = self.show_message_tree(&replies_vec, level + 1, newstring, n + 1, total, vec)?;
            j += 1;
        }
        Ok(n)
    }

    fn show_message_tree_single<W>(&self, messages: &[notmuch::Message], level: i32, prestring: String, num: i32, total: i32, writer: &mut W) -> Result<i32>
    where W: io::Write {
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

            let matched = message.get_flag(notmuch::MessageFlag::Match);
            let id = message.id();

            if level > 0 && n > 0 {
                if matched {
                    let highlight = self.highlight.as_ref().map_or(Ok(false), |hl| hl.message(message, n+1, total))?;
                    let str = self.template_message(&self.templ.templ_respons, &message, Some(newstring), n+1, total)?;
                    let show = Show { id: id.to_string(), entry: str, highlight };
                    serde_json::to_writer(&mut *writer, &show)?;
                    write!(writer,"\n")?;
                    return Ok(-1);
                }
            } else {
                if matched {
                    let highlight = self.highlight.as_ref().map_or(Ok(false), |hl| hl.message(message, n+1, total))?;
                    let str = self.template_message(&self.templ.templ_message, &message, None, n+1, total)?;
                    let show = Show { id: id.to_string(), entry: str, highlight};
                    serde_json::to_writer(&mut *writer, &show)?;
                    write!(writer,"\n")?;
                    return Ok(-1);
                }
            }

            let mut newstring: String = prestring.clone();
            if n == 0 {
            } else if length > j {
                newstring.push_str("│ ")
            } else {
                newstring.push_str("  ")
            }
            n = self.show_message_tree_single(&replies_vec, level + 1, newstring, n + 1, total, writer)?;
            if n == -1 {
                return Ok(-1);
            }
            j += 1;
        }
        Ok(n)
    }

    pub fn show_thread_single<W>(&self, search: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
        let query = self.db.create_query(search)?;
        query.set_sort(self.sort);
        let threads = query.search_threads()?;
        for thread in threads {
            let total = thread.total_messages();
            let messages = thread.toplevel_messages();
            let mvec: Vec<notmuch::Message> = messages.collect();
            self.show_message_tree_single(&mvec, 0, "".to_string(), 0, total, writer)?;
        }
        Ok(())
    }

    pub fn show_messages<W>(&self, search: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
        let query = self.db.create_query(&search)?;
        query.set_sort(self.sort);
        let threads = query.search_threads()?;
        let mut heap = BinaryHeap::new();
        for thread in threads {
            let reference = compare_time(&thread, self.sort);
            let total = thread.total_messages();
            flush_messages(&mut heap, self.sort, reference, writer)?;
            let top = thread.toplevel_messages();
            let mut counter = 0;
            for message in top {
                counter = self.show_messages_helper(&message, reference, counter, total, writer, &mut heap)?;
            }
        }
        Ok(())
    }

    fn show_messages_helper<W>(&self, message: &notmuch::Message, reference: i64, num: i32, 
        total: i32, writer: &mut W, heap: &mut BinaryHeap<OrderMessage<Show>>) -> Result<i32>
    where W: io::Write 
    {
            let mut counter = num + 1;
            let matched = message.get_flag(notmuch::MessageFlag::Match);
            if matched {
                let id = message.id();
                let unix_date = message.date();
                let highlight = self.highlight.as_ref().map_or(Ok(false), |hl| hl.message(message, counter, total))?;
                let str = self.template_message(&self.templ.templ_message, &message, None, counter, total)?;
                let show = Show { id: id.to_string(), entry: str, highlight};
                if compare_diff(unix_date, reference, self.sort) {
                    let om = OrderMessage(unix_date, self.sort, show);
                    heap.push(om)
                } else {
                    serde_json::to_writer(&mut *writer, &show)?;
                    write!(writer,"\n")?;
                }
            }
            for message in message.replies() {
                counter = self.show_messages_helper(&message, reference, counter, total, writer, heap)?;
            }
            Ok(counter)
        }

    pub fn show_threads<W>(&self, search: &str, writer: &mut W) -> Result<()>
    where W: io::Write {
        let query = self.db.create_query(&search)?;
        query.set_sort(self.sort);
        let threads = query.search_threads()?;
        for thread in threads {
            let id = thread.id();
            let str = self.template_thread(&self.templ.templ_message, &thread)?;
            // TODO add highlight!
            let tuple = Show { id: id.to_string(), entry: str, highlight: false };
            serde_json::to_writer(&mut *writer, &tuple)?;
            write!(writer,"\n")?;
        }
        Ok(())
    }

    pub fn show_before_message<W>(&self, id: &str, filter: Option<&str>, writer: &mut W) -> Result<()>
    where W: io::Write {
        let mut query = format!("mid:{}", id);
        if let Some(str) = filter {
            query.push_str(" and ");
            query.push_str(str);
        }
        let q = self.db.create_query(&query)?;
        let mut threads = q.search_threads()?;
        if let Some(thread) = threads.next() {
            let messages = thread.toplevel_messages();
            let bef = Self::forward(id, messages);

            if let Some(bef) = bef {
                for message in bef {
                    let mes = Message(message, 1, 1);
                    mes.show_message(writer)?;
                }
            }
        }
        Ok(())
    }

    fn forward(id: &str, messages: Messages) -> Option<Vec<notmuch::Message>> {
        for message in messages {
            if message.id() == id {
                return Some(vec![])
            }
            let messages = message.replies();
            let path = Self::forward(id, messages);
            if let Some(mut path) = path {
                path.push(message);
                return Some(path);
            }
        }
        None
    }

    fn search(id: &str, messages: Messages) -> Option<notmuch::Message> {
        for message in messages {
            if message.id() == id {
                return Some(message);
            }
            let messages = message.replies();
            let path = Self::search(id, messages);
            if let Some(path) = path {
                return Some(path);
            }
        }
        None
    }

    fn print_tree<W>(messages: Messages, writer: &mut W) -> Result<()>
    where W: io::Write {
        for message in messages {
            let rep = message.replies();
            let mes = Message(message, 1, 1);
            mes.show_message(writer)?;
            Self::print_tree(rep, writer)?;
        }
        Ok(())
    }

    pub fn show_after_message<W>(&self, id: &str, filter: Option<&str>, writer: &mut W) -> Result<()>
    where W: io::Write {
        let mut query = format!("mid:{}", id);
        if let Some(str) = filter {
            query.push_str(" and ");
            query.push_str(str);
        }
        let q = self.db.create_query(&query)?;
        let mut threads = q.search_threads()?;
        if let Some(thread) = threads.next() {
            let messages = thread.toplevel_messages();
            let after = Self::search(id, messages);

            if let Some(after) = after {
                let reps = after.replies();
                Self::print_tree(reps, writer)?;
            }
        }
        Ok(())
    }

    // TODO: Merge things that are in common
    fn template_message(&self, template: &str, message: &notmuch::Message, response: Option<String>, num: i32, total: i32) -> Result<String> {
        // let id = message.id();
        let from = message.header("From")?.unwrap_or_default();
        // let to = message.header("To")?.unwrap_or_default();

        let tags: Vec<String> = message.tags().collect();
        let tags_string = tags.join(", ");
        let unix_date = message.date();
        let date = show_time(unix_date, &self.date_format);

        let subject = message.header("Subject")?.unwrap_or_default();
        let subfixed = fix_subject(&subject);

        let template = self.templ.regex.replace_all(template, |caps: &Captures| {
            let str = caps.get(1).map(|x| x.as_str().trim());
            let pad: usize = caps.get(2).map(|x| x.as_str().parse().expect("Couldn't parse padding")).unwrap_or(0);

            let string = match str {
                Some("Date") => self.humanize(unix_date, pad),
                Some("date") => format!("{:pad$}", date),
                Some("index") => format!("{:0>pad$}", num),
                Some("total") => format!("{:0>pad$}", total),
                Some("from") => format!("{:pad$}", from),
                Some("subject") => format!("{:pad$}", subfixed),
                Some("response") => {
                    match response {
                        Some(ref response) => format!("{:pad$}", response),
                        None => panic!("Trying to use response outside of tree")
                    }
                },
                Some("tags") => format!("{:pad$}", tags_string),
                Some(x) => panic!("Tag {} not supported", x),
                _ => panic!("Syntax error, couldn't match template")
            };
            string
        });
        Ok(template.to_string())
    }

    fn template_thread(&self, template: &str, thread: &notmuch::Thread) -> Result<String> {
        // let id = thread.id();
        let subject = thread.subject();
        let subfixed = fix_subject(&subject);
        let authors = thread.authors().join(", ");
        let total = thread.total_messages();
        let matched = thread.matched_messages();
        let unix_date = thread.newest_date();
        let date = show_time(unix_date, &self.date_format);
        let tags: Vec<String> = thread.tags().collect();
        let tags_string = tags.join(", ");

        let template = self.templ.regex.replace_all(template, |caps: &Captures| {
            let str = caps.get(1).map(|x| x.as_str().trim());
            let pad: usize = caps.get(2).map(|x| x.as_str().parse().unwrap()).unwrap_or(0);

            let string = match str {
                Some("Date") => self.humanize(unix_date, pad),
                Some("date") => format!("{:pad$}", date),
                Some("index") => format!("{:0>pad$}", matched),
                Some("total") => format!("{:0>pad$}", total),
                Some("from") => format!("{:pad$}", authors),
                Some("subject") => format!("{:pad$}", subfixed),
                Some("tags") => format!("{:pad$}", tags_string),
                Some(x) => panic!("Tag {} not supported", x),
                _ => panic!("Syntax error, couldn't match template")
            };
            string
        });
        Ok(template.to_string())
    }
}
