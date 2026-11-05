//! Email / URL / IP / CIDR checks: no panic, and the network formats agree
//! with the standard library parsers.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vld::prelude::*;

fuzz_target!(|s: &str| {
    let v = serde_json::Value::String(s.to_owned());
    let ok = |schema: ZString| schema.parse_value(&v).is_ok();

    if ok(vld::string().email()) {
        assert_eq!(s.matches('@').count(), 1, "email accepted {s:?}");
    }
    if ok(vld::string().url()) {
        assert!(s.starts_with("http://") || s.starts_with("https://"), "url accepted {s:?}");
    }
    assert_eq!(
        ok(vld::string().ipv4()),
        s.parse::<std::net::Ipv4Addr>().is_ok(),
        "ipv4 {s:?}"
    );
    assert_eq!(
        ok(vld::string().ipv6()),
        s.parse::<std::net::Ipv6Addr>().is_ok(),
        "ipv6 {s:?}"
    );
    if ok(vld::string().cidr()) {
        let (addr, prefix) = s.split_once('/').expect("cidr has a /");
        assert!(addr.parse::<std::net::IpAddr>().is_ok(), "cidr {s:?}");
        assert!(prefix.parse::<u8>().is_ok(), "cidr {s:?}");
    }
    let _ = ok(vld::string().hostname());
    let _ = ok(vld::string().uuid());
});
