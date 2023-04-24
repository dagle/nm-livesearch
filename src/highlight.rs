use serde::Deserialize;

use crate::{Result, time::show_time};

#[derive(Deserialize, Debug)]
pub struct Highlight {
    // mode (if we should have and/or semantics)
    pub id: Option<String>,
    pub date: Option<String>,
    pub num: Option<i32>,
    pub total: Option<i32>,
    pub from: Option<String>,
    // can't do this one, it's extremely slow
    // to: Option<String>,
    pub subject: Option<String>,
    pub tags: Option<Vec<String>>,
    pub matched: Option<bool>,
    pub excluded: Option<bool>,
}

macro_rules! match_ret {
    ( $x:expr ) => {
        {
           if !$x {
               return Ok(false)
           }
        }
    };
}

impl Highlight {
    pub fn message<'a>(&self, message: &notmuch::Message, num: i32, total: i32) -> Result<bool> {
        if let Some(ref mid) = self.id {
            let id = message.id();
            match_ret!(mid == id.as_ref());
        }
        // we convert date to string to a string
        // so we can match 2021-11-09 agains 2021-11 and get a match
        if let Some(ref mdate) = self.date {
            let unix_date = message.date();
            let date = show_time(unix_date, "%Y-%m-%d");
            match_ret!(date.to_string().contains(mdate));
        }
        if let Some(mnum) = self.num {
            match_ret!(mnum == num);
        }
        if let Some(mtotal) = self.total {
            match_ret!(mtotal == total);
        }
        // when we do a match on email, the maddress need to be just the address:
        // so "apa@bep.com" would match "Mr Apa <apa@bep.com>" or "apa@bep.com"
        // it can't handle idn
        if let Some(ref mfrom) = self.from {
            let from = message.header("From")?.unwrap_or_default();
            match_ret!(from.contains(mfrom));
        }

        // Can't do this one! Since nm does't save the to address in the db
        // if let Some(ref mto) = mv.from {
        //     let to = message.header("To")?.unwrap_or_default();
        //     match_ret!(to.contains(mto));
        // }

        if let Some(ref msubject) = self.subject {
            let subject = message.header("Subject")?.unwrap_or_default();
            match_ret!(msubject == subject.as_ref());
        }
        if let Some(ref mtags) = self.tags {
            let tags: Vec<String> = message.tags().collect();
            for mtag in mtags {
                match_ret!(tags.contains(mtag));
            }
        }
        if let Some(mmatched) = self.matched {
            let nmatched = message.get_flag(notmuch::MessageFlag::Match);
            match_ret!(mmatched == nmatched);
        }
        if let Some(mexclude) = self.excluded {
            let exclude = message.get_flag(notmuch::MessageFlag::Excluded);
            match_ret!(mexclude == exclude);
        }

        return Ok(true);
    }
}
