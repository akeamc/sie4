use nom::{
    branch::alt,
    bytes::complete::{is_not, take_till, take_while, take_while1},
    character::complete::char,
    combinator::map,
    error::{Error, ErrorKind, ParseError},
    multi::many0,
    sequence::{delimited, preceded},
    Err, IResult,
};

pub fn label(i: &str) -> IResult<&str, &str> {
    preceded(char('#'), take_till(is_whitespace))(i)
}

fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn is_newline(c: char) -> bool {
    c == '\n' || c == '\r'
}

pub mod field {
    use std::str::FromStr;

    use nom::{
        combinator::{map_res, opt},
        multi::separated_list0,
    };
    use time::{format_description::FormatItem, macros::format_description, Date};

    use super::*;

    pub const DATE_FORMAT: &[FormatItem] = format_description!("[year][month][day]");

    pub fn is_sep(c: char) -> bool {
        is_whitespace(c) || is_newline(c)
    }

    // pub fn take_sep0(i: &str) -> IResult<&str, &str> {
    //     take_while(is_sep)(i)
    // }

    pub fn take_sep1(i: &str) -> IResult<&str, &str> {
        take_while1(is_sep)(i)
    }

    fn in_curly_braces(i: &str) -> IResult<&str, &str> {
        delimited(char('{'), take_until_unbalanced('{', '}'), char('}'))(i)
    }

    pub fn list(i: &str) -> IResult<&str, Vec<&str>> {
        let (i, o) = in_curly_braces(i)?;
        separated_list0(take_sep1, text)(o).map(|(_, o)| (i, o))
    }

    pub fn sub_items<O, F>(f: F) -> impl Fn(&str) -> IResult<&str, Vec<O>>
    where
        F: Fn(&str) -> IResult<&str, O>,
    {
        move |i: &str| {
            let (i, o) = in_curly_braces(i)?;
            many0(|s| f(s))(o).map(|(_, o)| (i, o))
        }
    }

    pub fn text(i: &str) -> IResult<&str, &str> {
        alt((
            delimited(char('\"'), is_not("\""), char('\"')),
            take_while1(|c: char| !is_sep(c) && c != '#'),
        ))(i)
    }

    pub fn next<'a, O, F>(f: F) -> impl Fn(&'a str) -> IResult<&'a str, O>
    where
        F: Fn(&'a str) -> IResult<&'a str, O>,
    {
        move |i: &'a str| {
            let (i, _) = take_sep1(i)?;
            f(i)
        }
    }

    pub fn parse_next<T>(i: &str) -> IResult<&str, T>
    where
        T: FromStr,
        T::Err: std::fmt::Debug,
    {
        map_res(next(text), |s| s.parse())(i)
    }

    pub fn next_string(i: &str) -> IResult<&str, String> {
        map(next(text), |s| s.to_owned())(i)
    }

    pub fn next_string_opt(i: &str) -> IResult<&str, Option<String>> {
        map(opt(next(text)), |s| s.map(ToOwned::to_owned))(i)
    }

    pub fn next_date(i: &str) -> IResult<&str, Date> {
        map_res(next(text), |s| Date::parse(s, DATE_FORMAT))(i)
    }

    // pub fn any(i: &str) -> IResult<&str, Field> {
    //     alt((
    //         map(list, |v| {
    //             Field::List(v.into_iter().map(|s| s.to_owned()).collect())
    //         }),
    //         map(text, |s| Field::Text(s.to_owned())),
    //     ))(i)
    // }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parse_list() {
            assert_eq!(
                list("{482 \"423\" 14}",),
                Ok(("", vec!["482", "423", "14"]))
            );
        }
    }
}

pub fn take_till_label(i: &str) -> IResult<&str, &str> {
    take_while(|c| c != '#')(i)
}

/// A parser similar to `nom::bytes::complete::take_until()`, except that this
/// one does not stop at balanced opening and closing tags. It is designed to
/// work inside the `nom::sequence::delimited()` parser.
///
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
    use super::*;

    #[test]
    fn parse_label() {
        assert_eq!(label("#KONTO 1220"), Ok((" 1220", "KONTO")))
    }
}
