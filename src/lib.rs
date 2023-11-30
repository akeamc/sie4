//! SIE4 is a Swedish file format for accounting data.
//! See [the specification](https://sie.se/wp-content/uploads/2020/05/SIE_filformat_ver_4B_ENGLISH.pdf)
//! for more information.

#![warn(clippy::pedantic)]

mod parsers;

pub mod item;
pub mod reader;

pub use item::Item;
pub use reader::Reader;

/// See [`nom_locate::LocatedSpan`].
pub type Span<'a> = nom_locate::LocatedSpan<&'a [u8]>;
