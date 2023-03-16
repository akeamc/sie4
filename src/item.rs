use std::fmt::Debug;

use nom::{bytes::complete::take_while1, combinator::map, IResult};
use rust_decimal::Decimal;
use time::Date;

use crate::parsers::{
    field::{list, next, next_date, next_string, parse_next, sub_items, text},
    label, take_till_label,
};

type Amount = Decimal;

#[derive(Debug, PartialEq, Eq)]
pub enum Field {
    Text(String),
    List(Vec<String>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Flag {
    pub read: bool,
}

impl ParsableItem for Flag {
    fn parse(i: &str) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, flag) = next(text)(i)?;
        let read = match flag {
            "0" => false,
            "1" => true,
            _ => panic!(),
        };

        Ok((i, Self { read }))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Program {
    pub name: String,
    pub version: String,
}

impl ParsableItem for Program {
    fn parse(i: &str) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, name) = next_string(i)?;
        let (i, version) = next_string(i)?;

        Ok((i, Self { name, version }))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub no: u32,
    pub name: String,
}

impl ParsableItem for Account {
    fn parse(i: &str) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, no) = parse_next(i)?;
        let (i, name) = next_string(i)?;

        Ok((i, Self { no, name }))
    }
}

/// SIE4 verification.
///
/// ```text
/// #VER A 1 20190101 "Test"
/// {
///     #TRANS 1930 {} 192.00 "Test"
///     #TRANS 8720 {} -192.00 "Test"
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Verification {
    pub series: String,
    pub no: u32,
    pub date: Date,
    pub name: String,
    pub transactions: Vec<Transaction>,
}

impl ParsableItem for Verification {
    fn parse(i: &str) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, series) = next_string(i)?;
        let (i, no) = parse_next(i)?;
        let (i, date) = next_date(i)?;
        let (i, name) = next_string(i)?;

        let (i, _) = take_while1(|c| c != '{')(i)?;
        let (i, transactions) = sub_items(|i| {
            let (i, _) = take_till_label(i)?;
            let (i, label) = label(i)?;
            if label != "TRANS" {
                panic!("VER sub-items must be TRANS");
            }
            Transaction::parse(i)
        })(i)?;

        if !transactions
            .iter()
            .map(|t| t.amount)
            .sum::<Decimal>()
            .is_zero()
        {
            panic!("VER transactions must sum to zero");
        }

        Ok((
            i,
            Self {
                series,
                no,
                date,
                name,
                transactions,
            },
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Transaction {
    pub account_no: u32,
    pub amount: Amount,
    pub date: Option<Date>,
}

impl ParsableItem for Transaction {
    fn parse(i: &str) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, account_no) = parse_next(i)?;
        let (i, _l) = next(list)(i)?;
        let (i, amount) = parse_next(i)?;
        let (i, date) = next_date(i)?;

        Ok((
            i,
            Self {
                account_no,
                amount,
                date: Some(date),
            },
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Item {
    Flag(Flag),
    Program(Program),
    Account(Account),
    Verification(Verification),
    Unknown(String, Vec<Field>),
}

trait ParsableItem {
    fn parse(i: &str) -> IResult<&str, Self>
    where
        Self: Sized;
}

fn parse_unknown_item(i: &str) -> IResult<&str, Vec<Field>> {
    Ok((i, vec![]))
}

macro_rules! labels {
  (
    $label:ident, $rest:ident,
    $($item_label:literal => $item:ident),*
  ) => {
    match $label {
        $($item_label => map($item::parse, Item::$item)($rest),)*
        _ => map(parse_unknown_item, |fields| Item::Unknown($label.to_owned(), fields))($rest)
    }
  }
}

impl Item {
    pub fn parse(i: &str) -> IResult<&str, Self> {
        let (i, label) = label(i)?;

        labels! {
            label, i,
            "FLAGGA" => Flag,
            "PROGRAM" => Program,
            "KONTO" => Account,
            "VER" => Verification
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::Month;

    use super::*;

    #[test]
    fn parse_item() {
        assert_eq!(
            Item::parse("#KONTO 1220 \"Inventarier och verktyg\""),
            Ok((
                "",
                Item::Account(Account {
                    no: 1220,
                    name: "Inventarier och verktyg".to_owned()
                })
            ))
        );

        assert_eq!(
            Item::parse(
                "#VER A 42 20230314 \"Pi Day\" 20230314
{
  #TRANS 1930 {} -72.00 20230228 \"Pie\"
  #TRANS 4007 {} 72.00 20230228 \"Pie\"
}

# VER A 43",
            ),
            Ok((
                "\n\n# VER A 43",
                Item::Verification(Verification {
                    series: "A".to_owned(),
                    no: 42,
                    date: Date::from_calendar_date(2023, Month::March, 14).unwrap(),
                    name: "Pi Day".to_owned(),
                    transactions: vec![
                        Transaction {
                            account_no: 1930,
                            amount: dec!(-72.00),
                            date: Some(
                                Date::from_calendar_date(2023, Month::February, 28).unwrap()
                            )
                        },
                        Transaction {
                            account_no: 4007,
                            amount: dec!(72.00),
                            date: Some(
                                Date::from_calendar_date(2023, Month::February, 28).unwrap()
                            )
                        }
                    ]
                })
            ))
        );
    }
}
