use std::{io, path::PathBuf, borrow::Cow};

use serde::{Serialize, ser::SerializeStruct};

use crate::Result;

pub struct Message(pub notmuch::Message, pub i32, pub i32);

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

impl Message {
    pub fn show_message<W>(&self, writer: &mut W) -> Result<()> 
        where W: io::Write {
            serde_json::to_writer(&mut *writer, self)?;
            write!(writer,"\n")?;
            Ok(())
    }
}
