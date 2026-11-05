//! Regression tests for the 0.4.7 security fixes (panics on untrusted input,
//! SSRF and path-traversal bypasses).

use vld::prelude::*;

#[test]
fn long_non_ascii_string_in_error_does_not_panic() {
    // 51 x "é" = 102 bytes; byte 97 is inside a character.
    let s = "é".repeat(51);
    let err = vld::string()
        .max(10)
        .parse_value(&serde_json::json!(s))
        .unwrap_err();
    assert!(!err.issues.is_empty());
    let long = "é".repeat(150);
    assert!(vld::string()
        .max(10)
        .parse_value(&serde_json::json!(long))
        .is_err());
}

#[test]
fn iso_formats_reject_non_ascii_and_truncated_input_without_panicking() {
    let date = vld::string().iso_date();
    for bad in ["123é56789", "2é24-01-01", "é"] {
        assert!(date.parse_value(&serde_json::json!(bad)).is_err(), "{bad}");
    }
    let time = vld::string().iso_time();
    assert!(time.parse_value(&serde_json::json!("1é:00")).is_err());
    let dt = vld::string().iso_datetime();
    for bad in ["2024-01-01T", "2024-01-01Té", "2024-01-01Té:00:00"] {
        assert!(dt.parse_value(&serde_json::json!(bad)).is_err(), "{bad}");
    }
    assert!(dt
        .parse_value(&serde_json::json!("2024-01-01T10:00:00Z"))
        .is_ok());
}

#[cfg(feature = "time")]
#[test]
fn time_backend_datetime_out_of_range_is_an_error_not_a_panic() {
    let dt = vld::datetime();
    assert!(dt
        .parse_value(&serde_json::json!("9999-12-31T23:00:00-05:00"))
        .is_err());
    assert!(dt
        .parse_value(&serde_json::json!("2024-01-01T10:00:00Z"))
        .is_ok());
}

#[cfg(feature = "string-advanced")]
#[test]
fn url_strict_blocks_ipv6_and_special_hosts() {
    let s = vld::string().url_strict();
    for bad in [
        "http://[::1]/",
        "http://[fd00::1]/",
        "http://[fe80::1]/",
        "http://[::]/",
        "http://[::ffff:127.0.0.1]/",
        "http://[::ffff:10.0.0.1]/",
        "http://0.0.0.0/",
        "http://localhost./",
        "http://api.localhost/",
        "http://127.0.0.1/",
        "http://10.1.2.3/",
    ] {
        assert!(s.parse_value(&serde_json::json!(bad)).is_err(), "{bad}");
    }
    for ok in [
        "https://example.com/",
        "http://[2606:4700::1111]/",
        "http://8.8.8.8/",
    ] {
        assert!(s.parse_value(&serde_json::json!(ok)).is_ok(), "{ok}");
    }
}

#[cfg(feature = "std")]
#[test]
fn path_within_rejects_traversal_of_missing_paths() {
    let base = std::env::temp_dir().join(format!("vld-hotfix-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let schema = vld::path().within(&base);
    let escape = format!("{}/../etc/passwd-does-not-exist", base.display());
    assert!(schema.parse_value(&serde_json::json!(escape)).is_err());
    assert!(schema
        .parse_value(&serde_json::json!("../outside-does-not-exist"))
        .is_err());
    assert!(schema
        .parse_value(&serde_json::json!("inside/new-file.txt"))
        .is_ok());
    std::fs::remove_dir_all(&base).ok();
}
