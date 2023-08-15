#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let reader = sie4::Reader::new(data);
    reader.for_each(drop);
});
