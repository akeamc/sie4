use std::{borrow::Cow, str::FromStr};

use codepage_437::{BorrowFromCp437, CP437_CONTROL};
use memchr::memchr3;
use nom::{
    branch::alt,
    bytes::streaming::{escaped, tag, take_while1},
    character::streaming::{char, none_of},
    combinator::map,
    error::{Error, ErrorKind, FromExternalError},
    sequence::delimited,
    Err, IResult, Slice,
};
use time::{format_description::FormatItem, macros::format_description, Date};

use crate::Span;

pub fn is_whitespace(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

pub fn is_line_break(c: u8) -> bool {
    c == b'\n' || c == b'\r'
}

pub const DATE_FORMAT: &[FormatItem] = format_description!("[year][month][day]");

pub fn in_curly_braces(i: Span) -> IResult<Span, Span> {
    delimited(char('{'), take_until_unbalanced(b'{', b'}'), char('}'))(i)
}

pub fn unquoted_text(i: Span) -> IResult<Span, Span> {
    take_while1(|c: u8| {
        !is_whitespace(c) && !is_line_break(c) && c != b'#' && c != b'{' && c != b'}'
    })(i)
}

pub fn quoted_text(i: Span) -> IResult<Span, Span> {
    let esc = escaped(none_of("\\\""), '\\', tag("\""));
    let esc_or_empty = alt((esc, tag("")));
    let (i, o) = delimited(tag("\""), esc_or_empty, tag("\""))(i)?;

    println!("{}", Cow::borrow_from_cp437(&o, &CP437_CONTROL));

    Ok((i, o))
}

pub fn text(i: Span) -> IResult<Span, String> {
    map(alt((quoted_text, unquoted_text)), |span| {
        let bytes: &[u8] = span.as_ref();
        println!("{:?} {:?}", bytes, std::str::from_utf8(bytes));
        Cow::borrow_from_cp437(bytes, &CP437_CONTROL).into_owned()
    })(i)
}

pub fn date(i: Span) -> IResult<Span, Date> {
    let s = Cow::borrow_from_cp437(&i, &CP437_CONTROL);
    let date = Date::parse(&s, DATE_FORMAT)
        .map_err(|e| nom::Err::Error(Error::from_external_error(i, ErrorKind::MapRes, e)))?;
    Ok((i.slice(i.len()..), date))
}

pub fn from_str<T: FromStr>(i: Span) -> IResult<Span, T> {
    let v = Cow::borrow_from_cp437(&i, &CP437_CONTROL)
        .parse()
        .map_err(|e| nom::Err::Error(Error::from_external_error(i, ErrorKind::MapRes, e)))?;
    Ok((i.slice(i.len()..), v))
}

pub fn take_until_unbalanced(opening: u8, closing: u8) -> impl Fn(Span) -> IResult<Span, Span> {
    move |i: Span| {
        let mut index = 0;
        let mut bracket_counter = 0;
        while let Some(n) = memchr3(opening, closing, b'\\', &i[index..]) {
            index += n;
            let mut it = i[index..].iter().copied();
            match it.next().unwrap_or_default() {
                b'\\' => {
                    // Skip the escape char `\`.
                    index += 1;
                    // Skip also the following char.
                    it.next();
                    index += 1;
                }
                c if c == opening => {
                    bracket_counter += 1;
                    index += 1;
                }
                c if c == closing => {
                    // Closing bracket.
                    bracket_counter -= 1;
                    index += 1;
                }
                _ => unreachable!(),
            };
            // We found the unmatched closing bracket.
            if bracket_counter == -1 {
                // We do not consume it.
                index -= 1;
                return Ok((i.slice(index..), i.slice(..index)));
            };
        }

        if bracket_counter == 0 {
            Ok((i.slice(i.len()..), i))
        } else {
            Err(Err::Incomplete(nom::Needed::Unknown))
        }
    }
}
