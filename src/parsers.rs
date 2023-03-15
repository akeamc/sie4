use nom::{
    branch::alt,
    bytes::complete::{is_not, take_till, take_while, take_while1},
    character::complete::char,
    combinator::{map},
    error::{Error, ErrorKind, ParseError},
    multi::many0,
    sequence::{delimited, preceded},
    Err, IResult,
};

use crate::item::{Field, Any};

fn label(i: &str) -> IResult<&str, &str> {
    preceded(char('#'), take_till(is_whitespace))(i)
}

fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn is_newline(c: char) -> bool {
    c == '\n' || c == '\r'
}

fn is_field_sep(c: char) -> bool {
    is_whitespace(c) || is_newline(c)
}

fn complex_field(i: &str) -> IResult<&str, Vec<Any>> {
    let (i, o) = delimited(char('{'), take_until_unbalanced('{', '}'), char('}'))(i)?;
    let (_, o) = items(o)?;
    Ok((i, o)) // forward everything after the ending delimeter
}

fn field(i: &str) -> IResult<&str, Field> {
    alt((
        map(complex_field, Field::Complex),
        map(
            delimited(char('\"'), is_not("\""), char('\"')),
            |s: &str| Field::Text(s.to_owned()),
        ),
        map(
            take_while1(|c: char| !is_field_sep(c) && c != '#'),
            |s: &str| Field::Text(s.to_owned()),
        ),
    ))(i)
}

pub fn item(i: &str) -> IResult<&str, Any> {
    let (i, _) = take_while(is_field_sep)(i)?;
    let (i, tag) = label(i)?;
    let (i, fields) = many0(preceded(take_while1(is_field_sep), field))(i)?;

    Ok((
        i,
        Any {
            tag: tag.to_owned(),
            fields,
        },
    ))
}

pub fn items(i: &str) -> IResult<&str, Vec<Any>> {
    many0(item)(i)
}

/// A parser similar to `nom::bytes::complete::take_until()`, except that this
/// one does not stop at balanced opening and closing tags. It is designed to
/// work inside the `nom::sequence::delimited()` parser.
///
/// # Basic usage
/// ```
/// use nom::bytes::complete::tag;
/// use nom::sequence::delimited;
/// use sie4::parsers::take_until_unbalanced;
///
/// let mut parser = delimited(tag("<"), take_until_unbalanced('<', '>'), tag(">"));
/// assert_eq!(parser("<<inside>inside>abc"), Ok(("abc", "<inside>inside")));
/// ```
/// It skips nested brackets until it finds an extra unbalanced closing bracket. Escaped brackets
/// like `\<` and `\>` are not considered as brackets and are not counted. This function is
/// very similar to `nom::bytes::complete::take_until(">")`, except it also takes nested brackets.
pub fn take_until_unbalanced(
    opening_bracket: char,
    closing_bracket: char,
) -> impl Fn(&str) -> IResult<&str, &str> {
    move |i: &str| {
        let mut index = 0;
        let mut bracket_counter = 0;
        while let Some(n) = &i[index..].find(&[opening_bracket, closing_bracket, '\\'][..]) {
            index += n;
            let mut it = i[index..].chars();
            match it.next().unwrap_or_default() {
                c if c == '\\' => {
                    // Skip the escape char `\`.
                    index += '\\'.len_utf8();
                    // Skip also the following char.
                    let c = it.next().unwrap_or_default();
                    index += c.len_utf8();
                }
                c if c == opening_bracket => {
                    bracket_counter += 1;
                    index += opening_bracket.len_utf8();
                }
                c if c == closing_bracket => {
                    // Closing bracket.
                    bracket_counter -= 1;
                    index += closing_bracket.len_utf8();
                }
                // Can not happen.
                _ => unreachable!(),
            };
            // We found the unmatched closing bracket.
            if bracket_counter == -1 {
                // We do not consume it.
                index -= closing_bracket.len_utf8();
                return Ok((&i[index..], &i[0..index]));
            };
        }

        if bracket_counter == 0 {
            Ok(("", i))
        } else {
            Err(Err::Error(Error::from_error_kind(i, ErrorKind::TakeUntil)))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::item::Any;

    use super::*;

    macro_rules! text {
        ($lit:literal) => {
            Field::Text($lit.to_owned())
        };
    }

    #[test]
    fn parse_item() {
        let (i, Any { tag, fields }) = item("#KONTO 1220 \"Inventarier och verktyg\"").unwrap();

        assert_eq!(i, "");
        assert_eq!(tag, "KONTO");
        assert_eq!(
            fields,
            vec![text!("1220"), text!("Inventarier och verktyg")]
        );

        let (_i, Any { tag, fields }) = item(
            "
#VER A 42 20230314 \"Pi Day\" 20230314
{
    #TRANS 1930 {} -72.00 20230228 \"Pie\"
    #TRANS 4007 {} 72.00 20230228 \"Pie\"
}

# VER A 43",
        )
        .unwrap();

        assert_eq!(tag, "VER");
        assert_eq!(
            fields,
            vec![
                text!("A"),
                text!("42"),
                text!("20230314"),
                text!("Pi Day"),
                text!("20230314"),
                Field::Complex(vec![
                    Any {
                        tag: "TRANS".to_owned(),
                        fields: vec![
                            text!("1930"),
                            Field::Complex(vec![]),
                            text!("-72.00"),
                            text!("20230228"),
                            text!("Pie"),
                        ]
                    },
                    Any {
                        tag: "TRANS".to_owned(),
                        fields: vec![
                            text!("4007"),
                            Field::Complex(vec![]),
                            text!("72.00"),
                            text!("20230228"),
                            text!("Pie")
                        ]
                    }
                ])
            ]
        )
    }

    #[test]
    fn parse_label() {
        assert_eq!(label("#KONTO 1220"), Ok((" 1220", "KONTO")))
    }

    #[test]
    fn parse_complex() {
        assert_eq!(complex_field(
            "{
#TRANS 4015 {} 185.00
#TRANS 1930 {} -185.00
        }",
        ), Ok(("", vec![Any {
            tag: "TRANS".to_owned(),
            fields: vec![
                text!("4015"),
                Field::Complex(vec![]),
                text!("185.00"),
            ]
        }, Any {
            tag: "TRANS".to_owned(),
            fields: vec![
                text!("1930"),
                Field::Complex(vec![]),
                text!("-185.00")
            ]
        }])));
    }

    #[test]
    fn parse_items_strange_input() {
        assert_eq!(items(""), Ok(("", vec![])));
    }
}
