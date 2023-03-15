use std::fmt::{self, Debug};

#[derive(PartialEq, Eq)]
pub enum Field {
    Text(String),
    Complex(Vec<Any>),
}

impl Debug for Field {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      match self {
          Self::Text(s) => Debug::fmt(s, f),
          Self::Complex(v) => Debug::fmt(v, f),
      }
  }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Any {
    pub tag: String,
    pub fields: Vec<Field>,
}

pub struct Program {
  name: String,
  version: String,
}

pub struct Type {
  no: u8,
}

pub enum Parsed {
  Program(Program),
  Type(Type)
}
