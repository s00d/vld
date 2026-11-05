//! ISO date / datetime / time checks: no panic on any input (char-boundary
//! slicing), and a valid datetime always has a valid date part.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vld::prelude::*;

fuzz_target!(|s: &str| {
    let v = serde_json::Value::String(s.to_owned());
    let date_ok = vld::string().iso_date().parse_value(&v).is_ok();
    let time_ok = vld::string().iso_time().parse_value(&v).is_ok();
    let datetime_ok = vld::string().iso_datetime().parse_value(&v).is_ok();

    if date_ok {
        assert_eq!(s.len(), 10, "iso_date accepted {s:?}");
    }
    if datetime_ok {
        let (date, time) = s.split_once(['T', 't']).expect("datetime has a T");
        let date_part = serde_json::Value::String(date.to_owned());
        assert!(
            vld::string().iso_date().parse_value(&date_part).is_ok(),
            "iso_datetime accepted {s:?} with a bad date"
        );
        assert!(!time.is_empty());
    }
    let _ = time_ok;
});
