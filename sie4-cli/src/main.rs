use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context;
use clap::Parser;
use rust_decimal::prelude::ToPrimitive;
use sie4::item::{Item, Trans};
use time::Date;
use xlsxwriter::{prelude::*, worksheet::conditional_format::ConditionalFormat};

#[derive(Debug, Parser)]
struct Args {
    /// Path to the SIE4 file to read.
    sie4: PathBuf,
    /// Path to the Excel workbook to create. If it exists, it will be
    /// overwritten. Defaults to the input path but with an .xlsx extension.
    #[clap(long, short)]
    output: Option<String>,
    /// The account number to have initially visible in the workbook.
    #[clap(long, default_value = "1930")]
    active_sheet: u32,

    #[clap(long)]
    open: bool,
}

const SERIES: u16 = 0;
const VER_NO: u16 = SERIES + 1;
const DATE: u16 = 2;
const AMOUNT: u16 = 3;
const AMOUNT_LETTER: char = ((AMOUNT + 65) as u8) as char;
const BALANCE: u16 = 4;
const ACCOUNT_NAME: u16 = 5;
const ACCOUNT_NO: u16 = 6;
const DESCRIPTION: u16 = 7;

struct TransactionsSheet<'a> {
    inner: Worksheet<'a>,
    row: u32,
}

fn accounting_fmt(sheet: &mut Worksheet<'_>, col: u16) -> Result<(), XlsxError> {
    sheet.set_column(
        col,
        col,
        12.,
        Some(&Format::new().set_num_format("_-* #,##0.00 kr")),
    )?;
    sheet.conditional_format_range(
        1,
        col,
        1000,
        col,
        &ConditionalFormat::cell_greater_than(0., Format::new().set_font_color(FormatColor::Green)),
    )?;
    sheet.conditional_format_range(
        1,
        col,
        1000,
        col,
        &ConditionalFormat::cell_less_than_or_equal_to(
            0.,
            Format::new().set_font_color(FormatColor::Red),
        ),
    )?;
    Ok(())
}

impl<'a> TransactionsSheet<'a> {
    const STARTING_ROW: u32 = 1;

    fn new(mut sheet: Worksheet<'a>) -> Result<Self, XlsxError> {
        sheet.merge_range(0, SERIES, 0, VER_NO, "Verifikation", None)?;
        sheet.write_string(0, DATE, "Datum", None)?;
        sheet.write_string(0, AMOUNT, "Belopp", None)?;
        sheet.write_string(0, ACCOUNT_NAME, "Konto", None)?;
        sheet.write_string(0, DESCRIPTION, "Beskrivning", None)?;
        sheet.write_string(0, ACCOUNT_NO, "Konto#", None)?;
        sheet.write_string(0, BALANCE, "Saldo", None)?;

        sheet.set_column(DESCRIPTION, DESCRIPTION, 30., None)?;
        sheet.set_column(
            DATE,
            DATE,
            10.,
            Some(&Format::new().set_num_format("yyyy-mm-dd")),
        )?;
        sheet.set_column(SERIES, SERIES, 1., None)?;
        sheet.set_column(VER_NO, VER_NO, 4., None)?;
        sheet.set_column(ACCOUNT_NAME, ACCOUNT_NAME, 30., None)?;

        accounting_fmt(&mut sheet, AMOUNT)?;
        accounting_fmt(&mut sheet, BALANCE)?;

        Ok(Self {
            inner: sheet,
            row: Self::STARTING_ROW,
        })
    }

    fn write(
        &mut self,
        trans: &Trans,
        series: &str,
        ver_no: u32,
        date: Date,
        account_name: &str,
    ) -> Result<(), XlsxError> {
        let sheet = &mut self.inner;
        sheet.write_string(self.row, SERIES, series, None)?;
        sheet.write_number(self.row, VER_NO, ver_no.into(), None)?;
        sheet.write_datetime(
            self.row,
            DATE,
            &DateTime {
                year: date.year().try_into().unwrap(),
                month: u8::from(date.month()).try_into().unwrap(),
                day: date.day().try_into().unwrap(),
                hour: 0,
                min: 0,
                second: 0.,
            },
            None,
        )?;
        sheet.write_number(self.row, AMOUNT, trans.amount.to_f64().unwrap(), None)?;
        sheet.write_string(self.row, ACCOUNT_NAME, &account_name, None)?;
        sheet.write_string(
            self.row,
            DESCRIPTION,
            trans.text.as_deref().unwrap_or(""),
            None,
        )?;
        sheet.write_number(self.row, ACCOUNT_NO, trans.account.into(), None)?;
        sheet.write_formula(
            self.row,
            BALANCE,
            &format!("=SUM({AMOUNT_LETTER}{}:{AMOUNT_LETTER}{})", 1, self.row + 1),
            None,
        )?;

        self.row += 1;

        Ok(())
    }

    fn touched(&self) -> bool {
        self.row > Self::STARTING_ROW
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let sie4 = File::open(&args.sie4)?;
    let reader = sie4::Reader::new(sie4);

    let output = args.output.unwrap_or_else(|| {
        let mut output = args.sie4.clone();
        output.set_extension("xlsx");
        output.to_str().unwrap().to_owned()
    });
    if Path::new(&output).try_exists()? {
        std::fs::remove_file(&output)?;
    }

    let workbook = Workbook::new(&output)?;
    let mut accounts = BTreeMap::new();

    for res in reader {
        match res? {
            Item::Konto(account) => {
                let name = account
                    .name
                    .chars()
                    .take(24)
                    .map(|c| match c {
                        '/' => '-',
                        c => c,
                    })
                    .collect::<String>();
                let name = format!("{} ({})", name, account.no);
                let sheet = TransactionsSheet::new(
                    workbook
                        .add_worksheet(Some(&name))
                        .with_context(|| format!("failed to add worksheet named {name:?}"))?,
                )?;
                accounts.insert(account.no, (account.name, sheet));
            }
            Item::Ver(ver) => {
                for trans in ver.transactions.0 {
                    let (account_name, sheet) = accounts.get_mut(&trans.account).unwrap();
                    sheet.write(
                        &trans,
                        &ver.series,
                        ver.no,
                        trans.date.unwrap_or(ver.date),
                        account_name,
                    )?;
                }
            }
            _ => (),
        }
    }

    for (no, (_, sheet)) in accounts.iter_mut() {
        if !sheet.touched() {
            sheet.inner.hide();
        }

        if *no == args.active_sheet {
            sheet.inner.activate();
        }
    }

    workbook.close()?;

    if args.open {
        Command::new("open")
            .arg(&output)
            .spawn()
            .with_context(|| format!("failed to open {output:?}"))?;
    }

    Ok(())
}
