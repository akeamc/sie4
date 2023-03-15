use std::{borrow::Cow, env, fs::File, io::Read, time::Instant};

use codepage_437::{BorrowFromCp437, CP437_CONTROL};
use sie4::items;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file = File::open(&args[1])?;
    let mut cp437 = Vec::new();

    file.read_to_end(&mut cp437)?;

    let data = Cow::borrow_from_cp437(&cp437, &CP437_CONTROL);

    let before = Instant::now();

    dbg!(items(&data).unwrap().1);

    println!("{:?}", before.elapsed());

    Ok(())
}
