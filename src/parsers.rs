use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_till, take_while, take_while1},
    character::streaming::char,
    combinator::{map_res, opt},
    error::{Error, ErrorKind, ParseError},
    multi::many0,
    sequence::{delimited, preceded, terminated},
    Err, Finish, IResult,
};

use crate::{Account, Entry, Field};

fn label(i: &str) -> IResult<&str, &str> {
    preceded(char('#'), take_till(|c: char| c.is_whitespace()))(i)
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

fn maybe_quoted(i: &str) -> IResult<&str, &str> {
    alt((
        delimited(char('\"'), is_not("\""), char('\"')),
        take_while(|c: char| !is_field_sep(c)),
    ))(i)
}

fn field(i: &str) -> IResult<&str, Field> {
    dbg!(i);
    map_res(
        alt((
            delimited(char('{'), take_until_unbalanced('{', '}'), char('}')),
            maybe_quoted,
        )),
        |s| s.parse(),
    )(i)
}

pub fn item(i: &str) -> IResult<&str, Entry> {
    let (i, tag) = label(i)?;
    // todo: stop this many0 loop on '#'
    let (i, fields) = many0(preceded(take_while1(is_field_sep), field))(i)?;

    Ok((
        i,
        Entry {
            tag: tag.to_owned(),
            fields,
        },
    ))
}

pub fn items(i: &str) -> IResult<&str, Vec<Entry>> {
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
/// use parse_hyperlinks::take_until_unbalanced;
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

mod tests {
    use nom::Finish;

    use crate::{parsers::label, Account, Entry, Field};

    use super::item;

    #[test]
    fn parse_item() {
        let (i, Entry { tag, fields }) = item("#KONTO 1220 \"Inventarier och verktyg\"").unwrap();

        assert_eq!(i, "");
        assert_eq!(tag, "KONTO");
        assert_eq!(
            fields,
            vec![
                Field::Text("1220".to_owned()),
                Field::Text("Inventarier och verktyg".to_owned())
            ]
        );

        // let Account { no, name } = item("#KONTO 1220 \"Inventarier och verktyg\"").unwrap();

        // assert_eq!(no, 1220);
        // assert_eq!(name, "Inventarier och verktyg".to_owned());
    }

    #[test]
    fn parse_label() {
        assert_eq!(label("#KONTO 1220"), Ok((" 1220", "KONTO")))
    }
}
