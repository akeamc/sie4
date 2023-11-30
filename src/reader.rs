use std::io::{BufRead, Read};

use nom_bufreader::bufreader::BufReader;

use crate::{
    item::{Group, Item},
    Span,
};

const BUF_SIZE: usize = 8192;

pub struct Reader<R: Read> {
    inner: BufReader<R>,
    group: Group,
}

impl<R: Read> Reader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: BufReader::with_capacity(BUF_SIZE, reader),
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
    /// SIE4 items must be ordered in ascending order by group
    /// (see [`crate::item::Group`]).
    #[error("items out of order")]
    OutOfOrder,
}

impl<R: Read> Iterator for Reader<R> {
    type Item = Result<Item, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let buf = self.inner.buffer();
            let before_len = buf.len();

            match Item::parse(Span::new(buf)) {
                Ok((rest, item)) => {
                    let offset = rest.location_offset();
                    self.inner.consume(offset);

                    if self.group > item.group() {
                        return Some(Err(Error::OutOfOrder));
                    }

                    self.group = item.group();

                    return Some(Ok(item));
                }
                Err(nom::Err::Incomplete(_)) => match self.inner.fill_buf() {
                    Ok(buf) if buf.len() == before_len => return None,
                    Ok(_) => continue,
                    Err(e) => return Some(Err(Error::Io(e))),
                },
                Err(nom::Err::Error(_e) | nom::Err::Failure(_e)) => return Some(Err(Error::Parse)),
            }
        }
    }
}
