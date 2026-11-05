//! A failing schema echoes the received string truncated on a char
//! boundary: at most 100 chars and a prefix of the input.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vld::prelude::*;

fuzz_target!(|s: &str| {
    let v = serde_json::Value::String(s.to_owned());
    let Err(err) = vld::string().max(0).parse_value(&v) else {
        return;
    };
    let got = err.issues[0].received.as_ref().unwrap().as_str().unwrap();
    assert!(got.chars().count() <= 100);
    let kept = got.strip_suffix("...").unwrap_or(got);
    assert!(s.starts_with(kept), "{got:?} is not a prefix of {s:?}");
    let _ = err.to_string();
    let _ = vld::format::prettify_error(&err);
});
