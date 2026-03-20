use serde_json::json;
use vld::prelude::*;

// === String ===

#[test]
fn string_basic() {
    let s = vld::string();
    assert_eq!(s.parse_value(&json!("hello")).unwrap(), "hello");
    assert!(s.parse_value(&json!(42)).is_err());
    assert!(s.parse_value(&json!(null)).is_err());
}

#[test]
fn string_min_max() {
    let s = vld::string().min(3).max(10);
    assert!(s.parse_value(&json!("ab")).is_err());
    assert!(s.parse_value(&json!("abc")).is_ok());
    assert!(s.parse_value(&json!("abcdefghijk")).is_err());
}

#[test]
fn string_email() {
    let s = vld::string().email();
    assert!(s.parse_value(&json!("test@example.com")).is_ok());
    assert!(s.parse_value(&json!("user+tag@sub.example.co.uk")).is_ok());
    assert!(s.parse_value(&json!("not-an-email")).is_err());
    assert!(s.parse_value(&json!("@no-local.com")).is_err());
    assert!(s.parse_value(&json!("no-domain@")).is_err());
    assert!(s.parse_value(&json!("spaces @example.com")).is_err());
}

#[test]
fn string_transforms() {
    let s = vld::string().trim().to_lowercase();
    assert_eq!(s.parse_value(&json!("  Hello  ")).unwrap(), "hello");
}

#[test]
fn string_coerce() {
    let s = vld::string().coerce();
    assert_eq!(s.parse_value(&json!(42)).unwrap(), "42");
    assert_eq!(s.parse_value(&json!(true)).unwrap(), "true");
}

#[test]
fn string_ipv4() {
    let s = vld::string().ipv4();
    assert!(s.parse_value(&json!("192.168.1.1")).is_ok());
    assert!(s.parse_value(&json!("0.0.0.0")).is_ok());
    assert!(s.parse_value(&json!("255.255.255.255")).is_ok());
    assert!(s.parse_value(&json!("256.0.0.1")).is_err());
    assert!(s.parse_value(&json!("1.2.3")).is_err());
    assert!(s.parse_value(&json!("1.2.3.4.5")).is_err());
    assert!(s.parse_value(&json!("not-an-ip")).is_err());
}

#[test]
fn string_ipv6() {
    let s = vld::string().ipv6();
    assert!(s
        .parse_value(&json!("2001:0db8:85a3:0000:0000:8a2e:0370:7334"))
        .is_ok());
    assert!(s.parse_value(&json!("::1")).is_ok());
    assert!(s.parse_value(&json!("::")).is_ok());
    assert!(s.parse_value(&json!("fe80::1")).is_ok());
    assert!(s.parse_value(&json!("not-ipv6")).is_err());
}

#[test]
fn string_base64() {
    let s = vld::string().base64();
    assert!(s.parse_value(&json!("SGVsbG8=")).is_ok());
    assert!(s.parse_value(&json!("aGVsbG8=")).is_ok());
    assert!(s.parse_value(&json!("YQ==")).is_ok());
    assert!(s.parse_value(&json!("not base64!")).is_err());
    assert!(s.parse_value(&json!("")).is_err());
}

#[test]
fn string_iso_date() {
    let s = vld::string().iso_date();
    assert!(s.parse_value(&json!("2024-01-15")).is_ok());
    assert!(s.parse_value(&json!("2024-12-31")).is_ok());
    assert!(s.parse_value(&json!("2024-1-5")).is_err());
    assert!(s.parse_value(&json!("2024-13-01")).is_err());
    assert!(s.parse_value(&json!("2024-00-01")).is_err());
    assert!(s.parse_value(&json!("not-a-date")).is_err());
}

#[test]
fn string_iso_datetime() {
    let s = vld::string().iso_datetime();
    assert!(s.parse_value(&json!("2024-01-15T10:30:00Z")).is_ok());
    assert!(s.parse_value(&json!("2024-01-15T10:30:00+02:00")).is_ok());
    assert!(s.parse_value(&json!("2024-01-15T10:30:00-05:30")).is_ok());
    assert!(s.parse_value(&json!("2024-01-15T10:30Z")).is_ok());
    assert!(s.parse_value(&json!("not-datetime")).is_err());
}

#[test]
fn string_iso_time() {
    let s = vld::string().iso_time();
    assert!(s.parse_value(&json!("10:30")).is_ok());
    assert!(s.parse_value(&json!("10:30:00")).is_ok());
    assert!(s.parse_value(&json!("23:59:59")).is_ok());
    assert!(s.parse_value(&json!("00:00")).is_ok());
    assert!(s.parse_value(&json!("25:00")).is_err());
    assert!(s.parse_value(&json!("12:60")).is_err());
}

#[test]
fn string_hostname() {
    let s = vld::string().hostname();
    assert!(s.parse_value(&json!("example.com")).is_ok());
    assert!(s.parse_value(&json!("sub.example.com")).is_ok());
    assert!(s.parse_value(&json!("localhost")).is_ok());
    assert!(s.parse_value(&json!("-invalid.com")).is_err());
    assert!(s.parse_value(&json!("")).is_err());
}

#[test]
fn string_uuid() {
    let s = vld::string().uuid();
    assert!(s
        .parse_value(&json!("550e8400-e29b-41d4-a716-446655440000"))
        .is_ok());
    assert!(s.parse_value(&json!("not-a-uuid")).is_err());
    assert!(s.parse_value(&json!("550e8400-e29b-41d4-a716")).is_err());
}

#[test]
fn string_url() {
    let s = vld::string().url();
    assert!(s.parse_value(&json!("https://example.com")).is_ok());
    assert!(s.parse_value(&json!("http://example.com/path?q=1")).is_ok());
    assert!(s.parse_value(&json!("not-a-url")).is_err());
    assert!(s.parse_value(&json!("ftp://example.com")).is_err());
}

#[test]
fn string_extra_validators() {
    assert!(vld::string().ip().parse_value(&json!("127.0.0.1")).is_ok());
    assert!(vld::string()
        .cidr()
        .parse_value(&json!("10.0.0.0/24"))
        .is_ok());
    assert!(vld::string()
        .mac()
        .parse_value(&json!("aa:bb:cc:dd:ee:ff"))
        .is_ok());
    assert!(vld::string().hex().parse_value(&json!("deadBEEF")).is_ok());
    assert!(vld::string()
        .credit_card()
        .parse_value(&json!("4111111111111111"))
        .is_ok());
    assert!(vld::string()
        .phone()
        .parse_value(&json!("+14155552671"))
        .is_ok());
    assert!(vld::string().semver().parse_value(&json!("1.2.3")).is_ok());
    assert!(vld::string()
        .jwt()
        .parse_value(&json!("aaaa.bbbb.cccc"))
        .is_ok());
    assert!(vld::string().ascii().parse_value(&json!("hello")).is_ok());
    assert!(vld::string().alpha().parse_value(&json!("Hello")).is_ok());
    assert!(vld::string()
        .alphanumeric()
        .parse_value(&json!("abc123"))
        .is_ok());
    assert!(vld::string()
        .lowercase()
        .parse_value(&json!("hello"))
        .is_ok());
    assert!(vld::string()
        .uppercase()
        .parse_value(&json!("HELLO"))
        .is_ok());
}

// === Number ===

#[test]
fn number_basic() {
    let n = vld::number();
    assert_eq!(n.parse_value(&json!(42.5)).unwrap(), 42.5);
    assert!(n.parse_value(&json!("hello")).is_err());
}

#[test]
fn number_min_max() {
    let n = vld::number().min(0.0).max(100.0);
    assert!(n.parse_value(&json!(-1)).is_err());
    assert!(n.parse_value(&json!(0)).is_ok());
    assert!(n.parse_value(&json!(101)).is_err());
}

#[test]
fn number_positive_negative() {
    assert!(vld::number().positive().parse_value(&json!(1)).is_ok());
    assert!(vld::number().positive().parse_value(&json!(0)).is_err());
    assert!(vld::number().negative().parse_value(&json!(-1)).is_ok());
    assert!(vld::number().negative().parse_value(&json!(0)).is_err());
}

#[test]
fn int_validation() {
    let n = vld::number().int().min(0).max(100);
    assert_eq!(n.parse_value(&json!(42)).unwrap(), 42);
    assert!(n.parse_value(&json!(42.5)).is_err());
    assert!(n.parse_value(&json!(-1)).is_err());
}

#[test]
fn int_non_positive() {
    let n = vld::number().int().non_positive();
    assert!(n.parse_value(&json!(0)).is_ok());
    assert!(n.parse_value(&json!(-3)).is_ok());
    assert!(n.parse_value(&json!(1)).is_err());
}

// === Boolean ===

#[test]
fn boolean_basic() {
    let b = vld::boolean();
    assert!(b.parse_value(&json!(true)).unwrap());
    assert!(b.parse_value(&json!("true")).is_err());
}

#[test]
fn boolean_coerce() {
    let b = vld::boolean().coerce();
    assert!(b.parse_value(&json!("true")).unwrap());
    assert!(!b.parse_value(&json!(0)).unwrap());
}

// === Literal ===

#[test]
fn literal_string() {
    let l = vld::literal("admin");
    assert_eq!(l.parse_value(&json!("admin")).unwrap(), "admin");
    assert!(l.parse_value(&json!("user")).is_err());
}

#[test]
fn literal_int() {
    let l = vld::literal(42i64);
    assert_eq!(l.parse_value(&json!(42)).unwrap(), 42);
    assert!(l.parse_value(&json!(43)).is_err());
}

#[test]
fn literal_bool() {
    let l = vld::literal(true);
    assert!(l.parse_value(&json!(true)).unwrap());
    assert!(l.parse_value(&json!(false)).is_err());
}

// === Enum ===

#[test]
fn enum_basic() {
    let e = vld::enumeration(&["red", "green", "blue"]);
    assert_eq!(e.parse_value(&json!("red")).unwrap(), "red");
    assert!(e.parse_value(&json!("yellow")).is_err());
    assert!(e.parse_value(&json!(42)).is_err());
}

// === Any ===

#[test]
fn any_basic() {
    let a = vld::any();
    assert_eq!(a.parse_value(&json!("hello")).unwrap(), json!("hello"));
    assert_eq!(a.parse_value(&json!(42)).unwrap(), json!(42));
    assert_eq!(a.parse_value(&json!(null)).unwrap(), json!(null));
}

// === Bytes ===

#[test]
fn bytes_array_mode() {
    let b = vld::bytes().min_len(2).max_len(4);
    assert_eq!(b.parse_value(&json!([1, 2, 3])).unwrap(), vec![1, 2, 3]);
    assert!(b.parse_value(&json!([1])).is_err());
    assert!(b.parse_value(&json!([1, 2, 3, 4, 5])).is_err());
    assert!(b.parse_value(&json!([256])).is_err());
}

#[test]
fn bytes_base64_mode() {
    let b = vld::bytes().base64().non_empty();
    assert_eq!(b.parse_value(&json!("AQID")).unwrap(), vec![1, 2, 3]);
    assert!(b.parse_value(&json!("@@@")).is_err());
}

#[cfg(feature = "std")]
#[test]
fn file_schema_validates_size_type_and_extension() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vld-file-{}.png", unique));

    // Minimal PNG signature bytes (enough for infer mime sniffing).
    let content = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    fs::write(&path, content).unwrap();

    let schema = vld::file()
        .non_empty()
        .max_size(1024)
        .extension("png")
        .media_type("image/png");

    let parsed = schema
        .parse_value(&json!(path.to_string_lossy().to_string()))
        .unwrap();
    assert_eq!(parsed.size(), 8);
    assert_eq!(parsed.extension(), Some("png"));
    assert_eq!(parsed.media_type(), Some("image/png"));

    let _ = fs::remove_file(path);
}

#[cfg(feature = "std")]
#[test]
fn file_schema_rejects_wrong_constraints() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vld-file-{}.txt", unique));
    fs::write(&path, b"hello").unwrap();

    let too_small = vld::file().min_size(10);
    assert!(too_small
        .parse_value(&json!(path.to_string_lossy().to_string()))
        .is_err());

    let bad_ext = vld::file().extension("png");
    assert!(bad_ext
        .parse_value(&json!(path.to_string_lossy().to_string()))
        .is_err());

    let _ = fs::remove_file(path);
}

// === Input ===

#[test]
fn vld_input_str() {
    assert_eq!(vld::string().parse("\"hello\"").unwrap(), "hello");
}

#[test]
fn vld_input_value() {
    let val = json!("test");
    assert_eq!(vld::string().parse(&val).unwrap(), "test");
}
