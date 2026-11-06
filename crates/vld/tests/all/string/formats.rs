//! String format checks — one accept/reject table per format.
//!
//! Every rejected input must yield exactly one `invalid_string` issue with
//! the format's `StringValidation`. Tables include empty, boundary and
//! non-ASCII inputs (full-width / Arabic-Indic digits must not count).

use serde_json::json;
use vld::error::{IssueCode, StringValidation};
use vld::prelude::*;

#[track_caller]
fn check(
    schema: impl VldSchema<Output = String>,
    validation: StringValidation,
    ok: &[&str],
    bad: &[&str],
) {
    for input in ok {
        match schema.parse_value(&json!(input)) {
            Ok(v) => assert_eq!(v, *input),
            Err(e) => panic!("{validation:?}: {input:?} rejected: {e}"),
        }
    }
    let want = IssueCode::InvalidString {
        validation: validation.clone(),
    };
    for input in bad {
        let err = schema
            .parse_value(&json!(input))
            .err()
            .unwrap_or_else(|| panic!("{validation:?}: {input:?} accepted"));
        assert_eq!(err.issues.len(), 1, "{input:?}: {err}");
        assert_eq!(err.issues[0].code, want, "{input:?}");
        assert!(err.issues[0].path.is_empty());
        assert_eq!(err.issues[0].received, Some(json!(input)));
    }
}

macro_rules! formats {
    ($( $(#[$m:meta])* $method:ident => $variant:ident { ok: [$($ok:expr),* $(,)?], bad: [$($bad:expr),* $(,)?] $(,)? } )*) => {
        $(
            $(#[$m])*
            #[test]
            fn $method() {
                check(
                    vld::string().$method(),
                    StringValidation::$variant,
                    &[$($ok),*],
                    &[$($bad),*],
                );
            }
        )*
    };
}

formats! {
    email => Email {
        ok: ["test@example.com", "user+tag@sub.example.co.uk", "a.b@x.com"],
        bad: [
            "", "not-an-email", "@no-local.com", "no-domain@", "spaces @example.com",
            "a..b@x.com", ".a@x.com", "a.@x.com", "a@b@c.com",
        ],
    }
    url => Url {
        ok: ["https://example.com", "http://example.com/path?q=1", "https://例え.jp/パス"],
        bad: ["", "not-a-url", "ftp://example.com", "https://", "example.com"],
    }
    uuid => Uuid {
        ok: ["550e8400-e29b-41d4-a716-446655440000", "00000000-0000-0000-0000-000000000000"],
        bad: [
            "", "not-a-uuid", "550e8400-e29b-41d4-a716", "550e8400-e29b-41d4-a716-44665544000g",
            "550e8400e29b41d4a716446655440000", "550e8400-e29b-41d4-a716-44665544000é",
        ],
    }
    ipv4 => Ipv4 {
        ok: ["192.168.1.1", "0.0.0.0", "255.255.255.255"],
        bad: [
            "", "256.0.0.1", "1.2.3", "1.2.3.4.5", "not-an-ip", "1.2.3.+4", "01.2.3.4",
            " 1.2.3.4", "1.2.3.٤", "１.2.3.4", "::1",
        ],
    }
    ipv6 => Ipv6 {
        ok: [
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334", "::1", "::", "fe80::1", "::ffff:1.2.3.4",
        ],
        bad: ["", "not-ipv6", "1.2.3.4", ":::", "2001:db8::g", "fe80::é"],
    }
    ip => Ip {
        ok: ["127.0.0.1", "::ffff:1.2.3.4", "::1"],
        bad: ["", "localhost", "1.2.3.256", "1.2.3.4/8"],
    }
    cidr => Cidr {
        ok: [
            "10.0.0.0/8", "10.0.0.0/24", "0.0.0.0/0", "192.168.1.1/32", "2001:db8::/32",
            "::/128", "::/0",
        ],
        bad: [
            "", "10.0.0.0", "10.0.0.0/+8", "10.0.0.0/08", "10.0.0.0/33", "2001:db8::/129",
            "10.0.0.0/٨", "::/129", "x/8", "10.0.0.0/",
        ],
    }
    mac => Mac {
        ok: ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"],
        bad: ["", "aa:bb:cc:dd:ee", "gg:bb:cc:dd:ee:ff", "aa:bb:cc:dd:ee:ff:00", "aa:bb:cc:dd:ee:é"],
    }
    hex => Hex {
        ok: ["deadBEEF", "0"],
        bad: ["xyz", "é", "0x1f", "de ad"],
    }
    base64 => Base64 {
        ok: ["SGVsbG8=", "aGVsbG8=", "YQ=="],
        bad: ["", "not base64!", "YQ=", "YQ", "é===", "SGVsbG8=="],
    }
    iso_date => IsoDate {
        ok: ["2024-01-15", "2024-12-31", "2024-02-29", "2000-02-29"],
        bad: [
            "", "not-a-date", "2024-1-5", "2024-13-01", "2024-00-01", "2024-01-00",
            "2023-02-29", "2024-02-31", "2024-04-31", "1900-02-29", "2024-01-01T",
            "2024-01-01T00:00:00Z", "２０２４-01-01", "2024-01-01é", "123é56789", "é",
        ],
    }
    iso_datetime => IsoDatetime {
        ok: [
            "2024-01-15T10:30:00Z", "2024-01-15T10:30:00+02:00", "2024-01-15T10:30:00-05:30",
            "2024-01-15t10:30:00z", "2024-01-15T10:30:00", "2024-01-15T10:30:00.123Z",
        ],
        bad: [
            "", "not-datetime", "2024-01-01", "2024-01-01T", "2024-01-01Té",
            "2024-01-01T10:00+garbage", "2024-01-01T10:00:00Zjunk", "2024-02-31T10:00:00Z",
            "2024-01-01T24:00:00Z", "2024-01-01T10:60Z", "2024-01-01T10:00:00+25:00",
            "2024-01-01T1é:00:00Z", "2024-01-15T10:30Z",
        ],
    }
    iso_time => IsoTime {
        ok: ["10:30", "10:30:00", "23:59:59", "00:00", "10:30:00.5"],
        bad: ["", "10", "10:3", "25:00", "24:00", "12:60", "12:00:60", "1é:00", "12:00Z"],
    }
    hostname => Hostname {
        ok: [
            "example.com", "sub.example.com", "localhost", "a-b.c", "a-b.c-d.io",
            "xn--80ak6aa92e.com", &"a".repeat(63),
        ],
        bad: [
            "", "-invalid.com", "invalid-.com", "exa mple.com", "a..b", "a_b.com", "é.com",
            &"a".repeat(64),
        ],
    }
    base64url => Base64Url {
        ok: ["QQ", "QUI", "QUJD", "QQ==", "-_-_", "aGVsbG8gd29ybGQ"],
        bad: ["", "Q", "a+b", "a+b/", "QQ=x", "QUJD=", "a b", "é"],
    }
    json => Json {
        ok: ["{}", "[1,2]", r#"{"a": [true, null]}"#, "1", r#""s""#, " null "],
        bad: ["", "{", "{'a': 1}", "[1,]", "1 2", "undefined"],
    }
    credit_card => CreditCard {
        ok: ["4111111111111111", "4111 1111 1111 1111", "4111-1111-1111-1111"],
        bad: ["", "4111111111111112", "hello 4111111111111111", "41111", "４111111111111111"],
    }
    phone => Phone {
        ok: ["+14155552671", "+1 (234) 567-8900"],
        bad: ["", "14155552671", "+1abc2345678", "+1", "+１4155552671"],
    }
    semver => Semver {
        ok: ["1.2.3", "v1.2.3", "0.0.0"],
        bad: ["", "1.2", "a.b.c", "1.2.é"],
    }
    jwt => Jwt {
        ok: ["aaaa.bbbb.cccc", "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln"],
        bad: ["", "aaaa.bbbb", "a b.c.d", "aaaa.bbbb.cc!c"],
    }
    ascii => Ascii {
        ok: ["hello", "", "~!@ 123"],
        bad: ["é", "日本", "a\u{00a0}b"],
    }
    alpha => Alpha {
        ok: ["Hello", "abc"],
        bad: ["abc1", "é", "hello world"],
    }
    alphanumeric => Alphanumeric {
        ok: ["abc123", "ABC"],
        bad: ["abc-123", "é1", "a b"],
    }
    lowercase => Lowercase {
        ok: ["hello", "héllo", "123", ""],
        bad: ["Hello", "É"],
    }
    uppercase => Uppercase {
        ok: ["HELLO", "É", "123"],
        bad: ["Hello", "é"],
    }
    slug => Slug {
        ok: ["hello-world-123", "a"],
        bad: ["Hello", "hello world", "café", "hello_world"],
    }
    color => Color {
        ok: ["#AABBCC", "#aabbccdd", "rgb(255, 0, 12)", "hsl(120, 50%, 40%)"],
        bad: ["", "AABBCC", "#GGHHII", "#ABCDE"],
    }
    currency_code => Currency {
        ok: ["USD", "EUR"],
        bad: ["", "usd", "US", "USDT", "ÜSD"],
    }
    country_code => CountryCode {
        ok: ["US", "DE"],
        bad: ["", "us", "USA", "Ü1"],
    }
    locale => Locale {
        ok: ["en", "en-US"],
        bad: ["", "e", "english", "en-USA"],
    }
    cron => Cron {
        ok: ["*/5 * * * *", "0 0 * * * *"],
        bad: ["", "* * *", "hello", "* * * * * * * *"],
    }
    cuid2 => Cuid2 {
        ok: ["clh3a8e9g000008l5fkbc1z1n", "abc123def456", "tz4a98xxat96iws9zmbrgj3a"],
        bad: [
            "", "a", "ABC123", "abc-123", "1abc", "é", "abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ],
    }
    ulid => Ulid {
        ok: ["01ARZ3NDEKTSV4RRFFQ69G5FAV"],
        bad: ["", "01ARZ3", "01ARZ3NDEKTSV4RRFFQ69GILOL", "01ARZ3NDEKTSV4RRFFQ69G5FAVX"],
    }
    nanoid => Nanoid {
        ok: ["V1StGXR8_Z5jdHi6B-myT"],
        bad: ["", "hello world", "é"],
    }
    emoji => Emoji {
        ok: ["😀", "⭐", "😀👍"],
        bad: ["", "hello world", "hello 😀", "hello😀"],
    }
    #[cfg(feature = "string-advanced")]
    url_strict => UrlStrict {
        ok: ["https://example.com/a", "http://[2606:4700::1111]/"],
        bad: [
            "", "ftp://example.com/", "http://[::1]/", "http://0.0.0.0/", "http://localhost./",
            "http://LOCALHOST/", "http://[::ffff:192.168.0.1]/", "http://[::ffff:127.0.0.1]/",
            "http://[fe80::1]/", "http://10.0.0.1/", "http://127.0.0.1/",
        ],
    }
    #[cfg(feature = "string-advanced")]
    uri => Uri {
        ok: ["https://example.com/path", "urn:isbn:0451450523"],
        bad: ["", "not a uri", "/relative/path"],
    }
    #[cfg(feature = "string-advanced")]
    uuid_v1 => UuidV1 {
        ok: ["c232ab00-9414-11ec-b3c8-9f6bdeced846"],
        bad: ["", "550e8400-e29b-41d4-a716-446655440000", "01890f57-5a7b-7f8b-bfd3-63f8e7c6f4b8"],
    }
    #[cfg(feature = "string-advanced")]
    uuid_v4 => UuidV4 {
        ok: ["550e8400-e29b-41d4-a716-446655440000"],
        bad: ["", "c232ab00-9414-11ec-b3c8-9f6bdeced846", "01890f57-5a7b-7f8b-bfd3-63f8e7c6f4b8"],
    }
    #[cfg(feature = "string-advanced")]
    uuid_v7 => UuidV7 {
        ok: ["01890f57-5a7b-7f8b-bfd3-63f8e7c6f4b8"],
        bad: ["", "550e8400-e29b-41d4-a716-446655440000"],
    }
    #[cfg(feature = "string-advanced")]
    semver_full => SemverFull {
        ok: ["1.2.3", "1.2.3-alpha.1+build.5"],
        bad: ["", "v1.2.3", "1.2", "01.2.3"],
    }
    #[cfg(feature = "string-advanced")]
    phone_e164_strict => PhoneE164 {
        ok: ["+14155552671", "+442071838750"],
        bad: ["", "14155552671", "+1", "+1abc"],
    }
}

/// 253 characters is the hostname limit (`received` is truncated, so not in the table).
#[test]
fn hostname_total_length_limit() {
    let long = ["abcdefghi"; 26].join("."); // 259 chars
    let err = vld::string()
        .hostname()
        .parse_value(&json!(long))
        .unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidString {
            validation: StringValidation::Hostname
        }
    );
}
