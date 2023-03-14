use std::{borrow::Cow, collections::HashMap, env, fs::File, io::Read, iter::Once};

use codepage_437::{BorrowFromCp437, CP437_CONTROL};
use once_cell::sync::OnceCell;
use regex::Regex;
use rust_decimal::Decimal;
use time::{macros::format_description, Date};

type AccountNo = u32;

#[derive(Debug)]
struct Account {
    no: AccountNo,
    name: String,
}

fn accounts_iter(s: &str) -> impl Iterator<Item = Account> + '_ {
    static RE: OnceCell<Regex> = OnceCell::new();

    RE.get_or_init(|| Regex::new("#KONTO (?P<no>[0-9]+) \"(?P<name>.*)\"").unwrap())
        .captures_iter(s)
        .map(|capture| Account {
            no: capture["no"].parse().unwrap(),
            name: capture["name"].to_owned(),
        })
}

#[derive(Debug)]
struct Transaction {
    account: u32,
    amount: Decimal,
}

fn transactions_iter(s: &str) -> impl Iterator<Item = anyhow::Result<Transaction>> + '_ {
    static RE: OnceCell<Regex> = OnceCell::new();

    RE.get_or_init(|| {
        Regex::new("#TRANS (?P<account>[0-9]+) \\{\\} (?P<amount>-?\\d*\\.{0,1}\\d+)").unwrap()
    })
    .captures_iter(s)
    .map(|capture| {
        Ok(Transaction {
            account: capture["account"].parse()?,
            amount: capture["amount"].parse()?,
        })
    })
}

#[derive(Debug)]
struct Verification {
    series: String,
    no: u32,
    date: Date,
    text: String,
    transactions: Vec<Transaction>,
}

fn verifications_iter(s: &str) -> impl Iterator<Item = anyhow::Result<Verification>> + '_ {
    static RE: OnceCell<Regex> = OnceCell::new();

    RE.get_or_init(|| {
        Regex::new("#VER (?P<series>[^\\s]+) (?P<no>[0-9]+) (?P<date>[0-9]{8}) \"(?P<text>.*)\" ([0-9]{8})\\r\\n\\{(?P<transactions>(.|\\n)*?)\\r\\n\\}")
            .unwrap()
    })
    .captures_iter(s)
    .map(|capture| {
        let fd = format_description!("[year][month][day]");

        let date = Date::parse(&capture["date"], fd).unwrap();

        Ok(Verification {
            series: capture["series"].to_owned(),
            no: capture["no"].parse()?,
            date,
            text: capture["text"].to_owned(),
            transactions: transactions_iter(&capture["transactions"]).collect::<Result<Vec<_>, _>>()?,
        })
    })
}

#[derive(Debug)]
struct AccountTransaction {
    amount: Decimal,
    text: String,
}

#[derive(Debug, Default)]
struct Book {
    accounts: HashMap<AccountNo, Account>,
    account_transactions: HashMap<AccountNo, Vec<AccountTransaction>>,
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file = File::open(&args[1])?;
    let mut cp437 = Vec::new();

    file.read_to_end(&mut cp437)?;

    let data = Cow::borrow_from_cp437(&cp437, &CP437_CONTROL);

    // println!("{data}");

    let mut book = Book::default();

    for acc in accounts_iter(&data) {
        book.accounts.insert(acc.no, acc);
    }

    for ver in verifications_iter(&data) {
        let ver = ver?;
        for transaction in ver.transactions {
            book.account_transactions
                .entry(transaction.account)
                .or_default()
                .push(AccountTransaction {
                    amount: transaction.amount,
                    text: ver.text.clone(),
                });
        }
    }

    // println!("Hello, world!");

    dbg!(book);

    Ok(())
}
