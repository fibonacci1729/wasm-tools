#![no_main]

use libfuzzer_sys::fuzz_target;
use wasm_compose::document;

fuzz_target!(|data: &[u8]| {
    drop(env_logger::try_init());

    let data = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    drop(document::parse("foo", &data));
});
