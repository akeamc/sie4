use std::str::FromStr;

use nom::multi::many0;
mod parsers;

pub type AccountNo = u32;

pub struct Account {
    pub no: AccountNo,
    pub name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Field {
    Text(String),
}

impl FromStr for Field {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::Text(s.to_owned()))
    }
}

#[derive(Debug)]
pub struct Entry {
    pub tag: String,
    pub fields: Vec<Field>,
}

pub use parsers::items;
