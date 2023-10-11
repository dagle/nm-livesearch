use std::{collections::BinaryHeap, io};

use notmuch::Sort;
use serde::Serialize;

use crate::{Result, time::compare_diff};

pub struct OrderMessage<T>(pub i64, pub Sort, pub T);

impl<T> PartialEq for OrderMessage<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl<T> Eq for OrderMessage<T> {
    fn assert_receiver_is_total_eq(&self) {}
}

impl<T> PartialOrd for OrderMessage<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.1 == Sort::OldestFirst {
            return other.0.partial_cmp(&self.0)
        }
        self.0.partial_cmp(&other.0)
    }
}

impl<T> Ord for OrderMessage<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.1 == Sort::OldestFirst {
            return other.0.cmp(&self.0)
        }
        self.0.cmp(&other.0)
    }
}

fn skips<T>(heap: &mut BinaryHeap<OrderMessage<T>>, skip: Option<usize>) -> Option<usize> {
    if let Some(mut s) = skip {
        while heap.pop().is_some() {
            s -= 1;
            if s == 0 {
                return None
            }
        }
        return Some(s)
    }
    None
}

pub fn flush_messages<W, T>(heap: &mut BinaryHeap<OrderMessage<T>>, sort: Sort, reference: i64, skip: &mut Option<usize>, limit: &mut Option<usize>, writer: &mut W) -> Result<bool>
where 
    W: io::Write,
    T: Serialize,
{
    *skip = skips(heap, *skip);
    while let Some(value) = heap.peek() {
        if !compare_diff(value.0, reference, sort) {
            if let Some(ref mut limit) = limit {
                if *limit == 0 {
                    return Ok(false)
                }
                *limit -= 1;
            }
            let top = heap.pop().unwrap();
            serde_json::to_writer(&mut *writer, &top.2)?;
            write!(writer,"\n")?;
        } else {
            break;
        }
    }
    Ok(false)
}
