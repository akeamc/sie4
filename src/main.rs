use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    path::PathBuf,
};

use clap::Parser;
use codepage_437::{BorrowFromCp437, CP437_CONTROL};
use colored::Colorize;
use prettytable::{format, row, Table};
use rust_decimal::Decimal;
use sie4::{item::Item, parse_items};
use time::{macros::format_description, Date};

fn parse_date(s: &str) -> Result<Date, String> {
    Date::parse(s, format_description!("[year]-[month]-[day]")).map_err(|e| e.to_string())
}

#[derive(Debug, Parser)]
struct Args {
    book: PathBuf,
    budget: PathBuf,
    #[clap(long, value_parser = parse_date)]
    from: Option<Date>,
}

#[derive(Debug)]
struct Transaction {
    date: Date,
    comment: String,
    amount: Decimal,
}

mod budget {
    use std::{collections::HashMap, path::Path};

    use rust_decimal::Decimal;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Row {
        account: u32,
        amount: Decimal,
    }

    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<HashMap<u32, Decimal>> {
        csv::Reader::from_path(path)?
            .into_deserialize()
            .map(|res| res.map(|Row { account, amount }| (account, amount)))
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut book = File::open(&args.book)?;
    let mut cp437 = Vec::new();

    let budget = budget::read(&args.budget)?;

    book.read_to_end(&mut cp437)?;

    let data = Cow::borrow_from_cp437(&cp437, &CP437_CONTROL);

    let mut account_names = BTreeMap::new();
    let mut account_transactions = HashMap::<_, Vec<Transaction>>::new();

    let items = parse_items(&data).unwrap().1;

    for item in items {
        match item {
            Item::Account(account) => {
                account_names.insert(account.name, account.no);
            }
            Item::Verification(verification) => {
                for transaction in verification.transactions {
                    let date = transaction.date.unwrap_or(verification.date);

                    if let Some(from) = args.from {
                        if date < from {
                            continue;
                        }
                    }

                    account_transactions
                        .entry(transaction.account_no)
                        .or_default()
                        .push(Transaction {
                            date,
                            amount: transaction.amount,
                            comment: verification.name.clone(),
                        });
                }
            }
            _ => continue,
        }
    }

    for (account_name, account_no) in account_names {
        let Some(rows) = account_transactions.get_mut(&account_no) else {
            continue;
        };

        let Some(budget) = budget.get(&account_no) else {
            println!("no budget for #{account_no} \"{account_name}\"");
            println!();
            continue;
        };

        rows.sort_by_key(|r| r.date);

        let mut table = Table::new();
        println!("{}", account_name.bold().underline().blue());

        let format = format::FormatBuilder::new()
            .column_separator('|')
            .padding(1, 1)
            .build();
        table.set_format(format);

        table.set_titles(row!(
            "Datum",
            "Kommentar",
            "Belopp",
            "Σ",
            format!("Budget ({budget})")
        ));

        let mut sum = Decimal::ZERO;
        for (
            i,
            Transaction {
                date,
                comment,
                amount,
            },
        ) in rows.iter().enumerate()
        {
            sum += *amount;

            let mut budget_remaining = (budget - sum).to_string();

            if i == rows.len() - 1 {
                budget_remaining = budget_remaining.on_yellow().to_string();
            }

            table.add_row(row![
                date,
                i -> comment,
                r -> amount,
                br -> sum,
                br -> budget_remaining,
            ]);
        }

        table.printstd();
        println!();
    }

    Ok(())
}
