use std::{borrow::Cow, collections::HashMap, env, fs::File, io::Read, iter::Once};

use codepage_437::{BorrowFromCp437, CP437_CONTROL};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file = File::open(&args[1])?;
    let mut cp437 = Vec::new();

    file.read_to_end(&mut cp437)?;

    let data = Cow::borrow_from_cp437(&cp437, &CP437_CONTROL);

    // println!("{data}");

    // println!("{:?}", hello_parser("#KONTO 1228 \"Ackumulerade nedskrivningar på inventarier och verktyg\""));
    // println!("{:?}", hello_parser("#HEJ"));
    // println!("{:?}", hello_parser("goodbye hello again"));

    Ok(())
}
