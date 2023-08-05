#![warn(clippy::pedantic)]

pub mod item;
mod parsers;
mod reader;

use nom_locate::LocatedSpan;
pub use reader::Reader;

type Span<'a> = LocatedSpan<&'a [u8]>;
