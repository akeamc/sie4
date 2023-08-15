use std::fmt::Debug;

use codepage_437::{BorrowFromCp437, CP437_CONTROL};
use iso_currency::Currency;
use nom::{
    branch::alt,
    bytes::streaming::{tag, take_while},
    character::streaming::{char, digit1},
    combinator::{complete, cut, map, map_res, opt, recognize},
    error::context,
    multi::many0,
    sequence::preceded,
    IResult,
};
use rust_decimal::Decimal;
use time::{format_description::FormatItem, macros::format_description, Date};

use crate::{
    parsers::{self, date, in_curly_braces, is_line_break, is_whitespace, text, unquoted_text},
    Span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Flag,
    Identification,
    Account,
    Balance,
}

type Amount = Decimal;

trait ParsableItem {
    fn parse(i: Span) -> IResult<Span, Self>
    where
        Self: Sized;
}

pub const DATE_FORMAT: &[FormatItem] = format_description!("[year][month][day]");

trait ParseField {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized;
}

#[allow(clippy::module_name_repetitions)]
pub trait Sie4Item {
    const LABEL: &'static str;

    const GROUP: Group;

    /// Parse an item from the input.
    /// 
    /// # Errors
    /// 
    /// As with other nom parsers, this function returns an error if the
    /// input is not a valid item or if the input is incomplete.
    fn parse_item(i: Span) -> IResult<Span, Self>
    where
        Self: Sized;
}

impl ParseField for String {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        map(text, |s| s.to_string())(i)
    }
}

impl ParseField for bool {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        alt((map(tag("0"), |_| false), map(tag("1"), |_| true)))(i)
    }
}

impl ParseField for Date {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        let (i, s) = unquoted_text(i)?;
        let (_, date) = cut(date)(s)?;
        Ok((i, date))
    }
}

impl ParseField for Currency {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        let (i, s) = unquoted_text(i)?;
        let (_, currency) = cut(parsers::from_str)(s)?;
        Ok((i, currency))
    }
}

impl ParseField for Decimal {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        let (i, s) = unquoted_text(i)?;
        let (_, value) = cut(parsers::from_str)(s)?;
        Ok((i, value))
    }
}

impl<T: ParseField> ParseField for Option<T> {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        opt(T::parse_field)(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SubItems<T>(pub Vec<T>);

impl<T: Sie4Item> ParseField for SubItems<T> {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        let (i, _) = take_while(|c| is_whitespace(c) || is_line_break(c))(i)?;
        let (i, o) = in_curly_braces(i)?;
        let (_, items) = many0(complete(|i| {
            let (i, _) = take_while(|c| c != b'#')(i)?;
            let (i, _) = preceded(char('#'), tag(T::LABEL))(i)?;
            T::parse_item(i)
        }))(o)?;

        Ok((i, Self(items)))
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct List<T>(pub Vec<T>);

impl<T: ParseField> ParseField for List<T> {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        let (i, o) = in_curly_braces(i)?;
        // many0(T::parse_field("")) will return an `Incomplete` error, but
        // we know that o is complete.
        let (_, o) = many0(complete(T::parse_field))(o)?;

        Ok((i, Self(o)))
    }
}

impl<T> From<Vec<T>> for List<T> {
    fn from(value: Vec<T>) -> Self {
        Self(value)
    }
}

macro_rules! parse_num_impl {
    ($ty:ty) => {
        impl ParseField for $ty {
            fn parse_field(i: Span) -> IResult<Span, Self>
            where
                Self: Sized,
            {
                map_res(recognize(preceded(opt(tag("-")), cut(digit1))), |b| {
                    std::borrow::Cow::borrow_from_cp437(&b, &CP437_CONTROL).parse()
                })(i)
            }
        }
    };
}

parse_num_impl!(i32);
parse_num_impl!(u32);

macro_rules! item_impl {
    ($name:ident ($group:ident) {
        $($field:ident: $ty:ty,)*
    }) => {
        paste::paste! {
            #[derive(Debug, PartialEq, Eq)]
            pub struct $name {
                $(
                    pub $field: $ty,
                )*
            }

            impl Sie4Item for $name {
                const LABEL: &'static str = stringify!([<$name:upper>]);

                const GROUP: Group = Group::$group;

                fn parse_item(i: Span) -> IResult<Span, Self> {
                    $(
                        let (i, _) = take_while(is_whitespace)(i)?;
                        let (i, $field) = context(stringify!($field), <$ty>::parse_field)(i)?;
                    )*

                    Ok((i, Self {
                        $($field,)*
                    }))
                }
            }
        }
    };
}

macro_rules! items_impl {
    {$($name:ident ($group:ident) $body:tt)*} => {
        #[derive(Debug, PartialEq, Eq)]
        pub enum Item {
            $(
                $name($name),
            )*
        }

        $(
            item_impl!($name ($group) $body);
        )*

        impl Item {
            /// Parse an item from the beginning of the input.
            /// 
            /// # Example
            /// 
            /// ```
            /// use sie4::{item::{Item, Program}, Span};
            /// let span = Span::new(b"#PROGRAM \"Vi iMproved\" 9.0\n");
            /// assert_eq!(
            ///     Item::parse(span).unwrap().1,
            ///     Item::Program(Program {
            ///         name: "Vi iMproved".to_owned(),
            ///         version: "9.0".to_owned(),
            ///     }),
            /// );
            /// ```
            /// 
            /// # Errors
            /// 
            /// Returns an error if the input is invalid or incomplete.
            pub fn parse(i: Span) -> IResult<Span, Self> {
                let (i, _) = take_while(|c| is_whitespace(c) || is_line_break(c))(i)?;

                paste::paste! {
                    preceded(tag("#"), alt(($(
                        map(
                            preceded(tag(stringify!([<$name:upper>])), $name::parse_item),
                            Self::$name,
                        ),
                    )*)))(i)
                }
            }

            #[must_use]
            pub const fn group(&self) -> Group {
                paste::paste! {
                    match self {
                        $(
                            Self::$name(_) => $name::GROUP,
                        )*
                    }
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FormatType {
    PC8,
}

impl ParseField for FormatType {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        map(tag("PC8"), |_| FormatType::PC8)(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TypeNo {
    SIE4,
}

impl ParseField for TypeNo {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        map(tag("4"), |_| TypeNo::SIE4)(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChartAccountsType {
    Bas95,
    Bas96,
    EuBas97,
    Ne2007,
}

impl ParseField for ChartAccountsType {
    fn parse_field(i: Span) -> IResult<Span, Self>
    where
        Self: Sized,
    {
        context(
            "account type",
            alt((
                map(tag("BAS95"), |_| ChartAccountsType::Bas95),
                map(tag("BAS96"), |_| ChartAccountsType::Bas96),
                map(tag("EUBAS97"), |_| ChartAccountsType::EuBas97),
                map(tag("NE2007"), |_| ChartAccountsType::Ne2007),
            )),
        )(i)
    }
}

items_impl! {
    Adress (Identification) {
        contact: String,
        distribution_address: String,
        postal_address: String,
        phone: String,
    }
    BKod (Identification) {
        sni: String,
    }
    Flagga (Flag) {
        read: bool,
    }
    FNamn (Identification) {
        name: String,
    }
    Format (Identification) {
        format: FormatType,
    }
    Gen (Identification) {
        date: Date,
        signature: Option<String>,
    }
    Ib (Balance) {
        year: i32,
        account: u32,
        balance: Amount,
        quantity: Option<String>,
    }
    Konto (Account) {
        no: u32,
        name: String,
    }
    KpTyp (Identification) {
        typ: ChartAccountsType,
    }
    Orgnr (Identification) {
        org_no: String,
    }
    Program (Identification) {
        name: String,
        version: String,
    }
    Rar (Identification) {
        no: i32,
        start: Date,
        end: Date,
    }
    Res (Balance) {
        year: i32,
        account: u32,
        balance: Amount,
        quantity: Option<String>,
    }
    SieTyp (Identification) {
        no: TypeNo,
    }
    Trans (Balance) {
        account: u32,
        objects: List<String>,
        amount: Amount,
        date: Option<Date>,
        text: Option<String>,
        quantity: Option<String>,
        signature: Option<String>,
    }
    Ub (Balance) {
        year: i32,
        account: u32,
        balance: Amount,
        quantity: Option<String>,
    }
    Valuta (Identification) {
        currency: Currency,
    }
    Ver (Balance) {
        series: String,
        no: u32,
        date: Date,
        text: Option<String>,
        reg_date: Option<Date>,
        sign: Option<String>,
        transactions: SubItems<Trans>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;
    use time::macros::date;

    #[test]
    fn optional() {
        // invalid date
        assert!(Option::<Date>::parse_field(Span::new(b"20201301 \"next\"")).is_err());

        // missing date
        assert_eq!(
            Option::<Date>::parse_field(Span::new(b" \"next\"")),
            Ok((Span::new(b" \"next\""), None))
        );

        // invalid currency
        assert!(Option::<Currency>::parse_field(Span::new(b"BTC \"next\"")).is_err());
    }

    #[test]
    fn parse_item() {
        assert_eq!(
            Item::parse(Span::new(b"#KONTO 1220 \"Inventarier och verktyg\"\n"))
                .unwrap()
                .1,
            Item::Konto(Konto {
                no: 1220,
                name: "Inventarier och verktyg".to_owned()
            })
        );

        assert_eq!(
            Item::parse(Span::new(
                b"#VER A 42 20230314 \"Pi Day\" 20230314
{
    #TRANS 1930 {} -72.00 20230228 \"Pie\"
    #TRANS 4007 {} 72.00 20230228 \"Pie\"
}

# VER A 43"
            ),)
            .unwrap()
            .1,
            Item::Ver(Ver {
                series: "A".to_owned(),
                no: 42,
                date: date!(2023 - 03 - 14),
                text: Some("Pi Day".to_owned()),
                reg_date: Some(date!(2023 - 03 - 14)),
                sign: None,
                transactions: SubItems(vec![
                    Trans {
                        account: 1930,
                        objects: List(vec![]),
                        amount: dec!(-72.00),
                        date: Some(date!(2023 - 02 - 28)),
                        text: Some("Pie".to_owned()),
                        quantity: None,
                        signature: None,
                    },
                    Trans {
                        account: 4007,
                        objects: List(vec![]),
                        amount: dec!(72.00),
                        date: Some(date!(2023 - 02 - 28)),
                        text: Some("Pie".to_owned()),
                        quantity: None,
                        signature: None,
                    }
                ])
            })
        );
    }

    #[test]
    fn parse_transaction() {
        assert_eq!(
            Trans::parse_item(Span::new(b" 1930 {} 192.00 20230320 \"Stonks\"\n"))
                .unwrap()
                .1,
            Trans {
                account: 1930,
                objects: Default::default(),
                amount: dec!(192.00),
                date: Some(date!(2023 - 03 - 20)),
                text: Some("Stonks".to_owned()),
                quantity: None,
                signature: None,
            }
        );

        assert_eq!(
            Trans::parse_item(Span::new(b" 1930 {}\t\t 583.52\n"))
                .unwrap()
                .1,
            Trans {
                account: 1930,
                objects: Default::default(),
                amount: dec!(583.52),
                date: None,
                text: None,
                quantity: None,
                signature: None,
            }
        );

        assert!(Trans::parse_item(Span::new(b" 1930 {} 583.52 \"Stonks\"")).is_err());
    }
}
