use std::{fmt::{Debug, self}, str::FromStr};

mod item;
pub mod parsers;

pub type AccountNo = u32;

pub struct Account {
    pub no: AccountNo,
    pub name: String,
}

pub use parsers::items;
