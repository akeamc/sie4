use std::io::{BufRead, Read};

use nom::{Err, Finish, Offset};

use crate::{
    item::{Group, Item},
    Span,
};

pub struct Reader<R: BufRead> {
    inner: R,
    // buf: Vec<u8>,
    // pos: usize,
    group: Group,
}

impl<R: BufRead> Reader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            group: Group::Flag,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(std::io::Error),
    #[error("parse error")]
    Parse,
}

impl<R: BufRead> Iterator for Reader<R> {
    type Item = Result<Item, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let buf = match self.inner.fill_buf() {
                Ok(buf) if buf.is_empty() => return None,
                Ok(buf) => buf,
                Err(e) => return Some(Err(Error::Io(e))),
            };

            match Item::parse(Span::new(buf)) {
                Ok((rest, item)) => {
                    let offset = rest.location_offset();
                    self.inner.consume(offset);

                    assert!(self.group <= item.group());

                    self.group = item.group();

                    return Some(Ok(item));
                }
                Err(nom::Err::Incomplete(_)) => (),
                Err(nom::Err::Error(_e) | nom::Err::Failure(_e)) => return Some(Err(Error::Parse)),
            }
        }
    }
}
