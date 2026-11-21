//! Corpus: string formats (`string().email()`, `.uuid()`, `.iso_date()`, …), one accept/reject
//! row per format.
//!
//! An accepted input is returned unchanged; a rejected one yields exactly one root
//! `invalid_string` issue with the format's `StringValidation` and the input as `received`.
//! Rows include empty, boundary and non-ASCII inputs (full-width and Arabic-Indic digits must not
//! count as digits, non-ASCII must never panic the ISO scanners).

use serde_json::json;
use vld::error::{IssueCode, StringValidation as V};
use vld::prelude::*;

type Row = (ZString, V, &'static [&'static str], &'static [&'static str]);

fn rows() -> Vec<Row> {
    let s = vld::string;
    #[allow(unused_mut)] // extended only under `string-advanced`
    let mut rows: Vec<Row> = vec![
        (
            s().email(),
            V::Email,
            &[
                "test@example.com",
                "user+tag@sub.example.co.uk",
                "a.b@x.com",
            ],
            &[
                "",
                "not-an-email",
                "@no-local.com",
                "no-domain@",
                "spaces @example.com",
                "a..b@x.com",
                ".a@x.com",
                "a.@x.com",
                "a@b@c.com",
            ],
        ),
        (
            s().url(),
            V::Url,
            &[
                "https://example.com",
                "http://example.com/path?q=1",
                "https://例え.jp/パス",
            ],
            &[
                "",
                "not-a-url",
                "ftp://example.com",
                "https://",
                "example.com",
            ],
        ),
        (
            s().uuid(),
            V::Uuid,
            &[
                "550e8400-e29b-41d4-a716-446655440000",
                "00000000-0000-0000-0000-000000000000",
            ],
            &[
                "",
                "not-a-uuid",
                "550e8400-e29b-41d4-a716",
                "550e8400-e29b-41d4-a716-44665544000g",
                "550e8400e29b41d4a716446655440000",
                "550e8400-e29b-41d4-a716-44665544000é",
            ],
        ),
        (
            s().ipv4(),
            V::Ipv4,
            &["192.168.1.1", "0.0.0.0", "255.255.255.255"],
            &[
                "",
                "256.0.0.1",
                "1.2.3",
                "1.2.3.4.5",
                "not-an-ip",
                "1.2.3.+4",
                "01.2.3.4",
                " 1.2.3.4",
                "1.2.3.٤",
                "１.2.3.4",
                "::1",
            ],
        ),
        (
            s().ipv6(),
            V::Ipv6,
            &[
                "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
                "::1",
                "::",
                "fe80::1",
                "::ffff:1.2.3.4",
            ],
            &["", "not-ipv6", "1.2.3.4", ":::", "2001:db8::g", "fe80::é"],
        ),
        (
            s().ip(),
            V::Ip,
            &["127.0.0.1", "::ffff:1.2.3.4", "::1"],
            &["", "localhost", "1.2.3.256", "1.2.3.4/8"],
        ),
        (
            s().cidr(),
            V::Cidr,
            &[
                "10.0.0.0/8",
                "10.0.0.0/24",
                "0.0.0.0/0",
                "192.168.1.1/32",
                "2001:db8::/32",
                "::/128",
                "::/0",
            ],
            &[
                "",
                "10.0.0.0",
                "10.0.0.0/+8",
                "10.0.0.0/08",
                "10.0.0.0/33",
                "2001:db8::/129",
                "10.0.0.0/٨",
                "::/129",
                "x/8",
                "10.0.0.0/",
            ],
        ),
        (
            s().mac(),
            V::Mac,
            &["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"],
            &[
                "",
                "aa:bb:cc:dd:ee",
                "gg:bb:cc:dd:ee:ff",
                "aa:bb:cc:dd:ee:ff:00",
                "aa:bb:cc:dd:ee:é",
            ],
        ),
        (
            s().hex(),
            V::Hex,
            &["deadBEEF", "0"],
            &["xyz", "é", "0x1f", "de ad"],
        ),
        (
            s().base64(),
            V::Base64,
            &["SGVsbG8=", "aGVsbG8=", "YQ=="],
            &["", "not base64!", "YQ=", "YQ", "é===", "SGVsbG8=="],
        ),
        (
            s().iso_date(),
            V::IsoDate,
            &["2024-01-15", "2024-12-31", "2024-02-29", "2000-02-29"],
            &[
                "",
                "not-a-date",
                "2024-1-5",
                "2024-13-01",
                "2024-00-01",
                "2024-01-00",
                "2023-02-29",
                "2024-02-31",
                "2024-04-31",
                "1900-02-29",
                "2024-01-01T",
                "2024-01-01T00:00:00Z",
                "２０２４-01-01",
                "2024-01-01é",
                "2024-01-01Té",
                "123é56789",
                "1é:00",
                "é",
            ],
        ),
        (
            s().iso_datetime(),
            V::IsoDatetime,
            &[
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00+02:00",
                "2024-01-15T10:30:00-05:30",
                "2024-01-15t10:30:00z",
                "2024-01-15T10:30:00",
                "2024-01-15T10:30:00.123Z",
                "2024-01-15T10:30:00.5+02:00",
                "2024-01-01t10:00:00.5z",
            ],
            &[
                "",
                "not-datetime",
                "2024-01-01",
                "2024-01-01T",
                "2024-01-01Té",
                "2024-01-01T10:00+garbage",
                "2024-01-01T10:00:00+garbage",
                "2024-01-01T10:00+03:00",
                "2024-01-01T10:00:00Zjunk",
                "2024-02-31T10:00:00Z",
                "2024-01-01T24:00:00Z",
                "2024-01-01T10:60Z",
                "2024-01-01T10:00:00+25:00",
                "2024-01-01T1é:00:00Z",
                "2024-01-15T10:30Z",
                "2024-01-15T10:30",
                "2024-01-15 10:30:00Z",
                "123é56789",
                "1é:00",
                "é",
            ],
        ),
        (
            s().iso_time(),
            V::IsoTime,
            &["10:30", "10:30:00", "23:59:59", "00:00", "10:30:00.5"],
            &[
                "",
                "10",
                "10:3",
                "25:00",
                "24:00",
                "12:60",
                "12:00:60",
                "1é:00",
                "12:00Z",
                "123é56789",
                "2024-01-01T",
                "2024-01-01Té",
                "2024-01-01T1é:00:00Z",
                "é",
            ],
        ),
        (
            s().hostname(),
            V::Hostname,
            &[
                "example.com",
                "sub.example.com",
                "localhost",
                "a-b.c",
                "a-b.c-d.io",
                "xn--80ak6aa92e.com",
                "a1.b2",
                "123",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ],
            &[
                "",
                "-invalid.com",
                "invalid-.com",
                "exa mple.com",
                "a..b",
                "a_b.com",
                "é.com",
                "999.1.1.1",
                "1.2.3.4",
                "example.123",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "10.0.0.0/",
                "10.0.0.0/+8",
                "10.0.0.0/08",
                "10.0.0.0/0032",
                "10.0.0.0/33",
                "::/1000",
                "::/129",
            ],
        ),
        (
            s().base64url(),
            V::Base64Url,
            &["QQ", "QUI", "QUJD", "QQ==", "-_-_", "aGVsbG8gd29ybGQ"],
            &["", "Q", "a+b", "a+b/", "QQ=x", "QUJD=", "a b", "é"],
        ),
        (
            s().json(),
            V::Json,
            &[
                "{}",
                "[1,2]",
                r#"{"a": [true, null]}"#,
                "1",
                r#""s""#,
                " null ",
            ],
            &["", "{", "{'a': 1}", "[1,]", "1 2", "undefined"],
        ),
        (
            s().credit_card(),
            V::CreditCard,
            &[
                "4111111111111111",
                "4111 1111 1111 1111",
                "4111-1111-1111-1111",
                // 12 and 19 digits; doubled digits above 4 lose 9.
                "555555555559",
                "5678901234567890127",
                "378282246310005",
            ],
            &[
                "",
                "4111111111111112",
                "hello 4111111111111111",
                "41111",
                // Luhn-valid but 11 / 20 digits.
                "55555555550",
                "55555555555555555555",
                "４111111111111111",
            ],
        ),
        (
            s().phone(),
            V::Phone,
            &["+14155552671", "+1 (234) 567-8900"],
            &["", "14155552671", "+1abc2345678", "+1", "+１4155552671"],
        ),
        (
            s().semver(),
            V::Semver,
            &["1.2.3", "v1.2.3", "0.0.0"],
            &["", "1.2", "a.b.c", "1.2.é", " 1.2.3 ", "1.2.3 "],
        ),
        (
            s().jwt(),
            V::Jwt,
            &[
                "aaaa.bbbb.cccc",
                "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln",
                "a-b.c_d.-_",
            ],
            &["", "aaaa.bbbb", "a b.c.d", "aaaa.bbbb.cc!c"],
        ),
        (
            s().ascii(),
            V::Ascii,
            &["hello", "", "~!@ 123"],
            &["é", "日本", "a\u{00a0}b"],
        ),
        (
            s().alpha(),
            V::Alpha,
            &["Hello", "abc"],
            &["abc1", "é", "hello world"],
        ),
        (
            s().alphanumeric(),
            V::Alphanumeric,
            &["abc123", "ABC"],
            &["abc-123", "é1", "a b"],
        ),
        (
            s().lowercase(),
            V::Lowercase,
            &["hello", "héllo", "123", ""],
            &["Hello", "É"],
        ),
        (
            s().uppercase(),
            V::Uppercase,
            &["HELLO", "É", "123"],
            &["Hello", "é"],
        ),
        (
            s().slug(),
            V::Slug,
            &["hello-world-123", "a"],
            &["Hello", "hello world", "café", "hello_world"],
        ),
        (
            s().color(),
            V::Color,
            &[
                "#AABBCC",
                "#aabbccdd",
                "rgb(255, 0, 12)",
                "rgb(1,2,3)",
                "hsl(120, 50%, 40%)",
            ],
            &[
                "",
                "AABBCC",
                "#GGHHII",
                "#ABCDE",
                "rgb(+1,+2,+3)",
                "hsl(+120, 50%, 50%)",
                "hsl(120, +50%, 50%)",
            ],
        ),
        (
            s().currency_code(),
            V::Currency,
            &["USD", "EUR", "JPY", "CLF", "XCG", "ZWG"],
            // unassigned, withdrawn (HRK, ZWL), metals / test codes (no minor units)
            &[
                "", "usd", "US", "USDT", "ÜSD", "ABC", "HRK", "ZWL", "XAU", "XTS",
            ],
        ),
        (
            s().country_code(),
            V::CountryCode,
            &["US", "DE"],
            &["", "us", "USA", "Ü1"],
        ),
        (
            s().locale(),
            V::Locale,
            &["en", "en-US"],
            &["", "e", "english", "en-USA", "en_US", "EN-US", "en-us"],
        ),
        (
            s().cron(),
            V::Cron,
            &[
                "*/5 * * * *",
                "0 0 * * * *",
                "0 9-17 * * MON-FRI",
                "30 2 1,15 * ?",
                "0 0 1 jan *",
                "0 0 * * 7",
                "0-30/10 * * * *",
                "59 23 31 12 6",
                "@daily",
                "@hourly",
            ],
            &[
                "",
                "* * *",
                "hello",
                "* * * * * * * *",
                "60 * * * *",
                "* 24 * * *",
                "* * 0 * *",
                "* * * 13 *",
                "* * * * 8",
                "*/0 * * * *",
                "*/60 * * * *",
                "5-1 * * * *",
                "? * * * *",
                "1,,2 * * * *",
                "1/2/3 * * * *",
                "+5 * * * *",
                "0 0 L * *",
                "0 0 * * MON#2",
                "@reboot",
                "* * * FOO *",
            ],
        ),
        (
            s().country(),
            V::Country,
            &["US", "DE", "GB", "AX", "ZW"],
            &["", "us", "UK", "XX", "ZZ", "USA", "U", "ÜS"],
        ),
        (
            s().country_alpha3(),
            V::CountryAlpha3,
            &["USA", "DEU", "GBR", "ALA", "ZWE"],
            &["", "usa", "UK", "GB", "XXX", "UKR1"],
        ),
        (
            s().iban(),
            V::Iban,
            &[
                "DE89370400440532013000",
                "DE89 3704 0044 0532 0130 00",
                "GB82WEST12345698765432",
                "FR1420041010050500013M02606",
                "NL91ABNA0417164300",
                "NO9386011117947",
                "BE68539007547034",
            ],
            &[
                "",
                // checksum off by one digit
                "DE89370400440532013001",
                "GB82WEST12345698765433",
                // wrong length for the country
                "DE8937040044053201300",
                "DE893704004405320130000",
                "de89370400440532013000",
                "DE89  3704 0044 0532 0130 00",
                " DE89370400440532013000",
                "XX89370400440532013000",
                "DE8X370400440532013000",
                "DE89-3704-0044-0532-0130-00",
            ],
        ),
        (
            s().bic(),
            V::Bic,
            &["DEUTDEFF", "DEUTDEFF500", "NEDSZAJJXXX", "BKPRXKPR"],
            &[
                "",
                "DEUTDEF",
                "DEUTDEFF50",
                "DEUT1EFF",
                "DEUTZZFF",
                "deutdeff",
                "DEUTDEFF5000",
            ],
        ),
        (
            s().isbn(),
            V::Isbn,
            &[
                "978-3-16-148410-0",
                "9783161484100",
                "0-306-40615-2",
                "0306406152",
                "080442957X",
                "0 306 40615 2",
                "979-10-90636-07-1",
            ],
            &[
                "",
                "978-3-16-148410-1",
                "0-306-40615-3",
                "080442957x",
                "9773161484100",
                "-0306406152",
                "0306406152-",
                "0--306406152",
                "030640615",
                "97831614841000",
            ],
        ),
        (
            s().ean(),
            V::Ean,
            &[
                "4006381333931",
                "73513537",
                "036000291452",
                "10012345678902",
            ],
            &[
                "",
                "4006381333932",
                "73513536",
                "036000291453",
                "400638133393",
                "4006-381333931",
                "40063813339a1",
                "123456789",
            ],
        ),
        (
            s().vin(),
            V::Vin,
            &[
                "1M8GDM9AXKP042788",
                "11111111111111111",
                "1HGCM82633A004352",
            ],
            &[
                "",
                "1M8GDM9A1KP042788",
                "1M8GDM9AXKP042789",
                "1M8GDM9AXKP04278",
                "1M8GDM9AXKP0427880",
                "IM8GDM9AXKP042788",
                "1m8gdm9axkp042788",
            ],
        ),
        (
            s().mime(),
            V::Mime,
            &[
                "application/json",
                "text/html; charset=utf-8",
                "text/html;charset=UTF-8",
                "application/vnd.api+json",
                "image/svg+xml",
                "multipart/form-data; boundary=\"a b\\\"c\"",
            ],
            &[
                "",
                "json",
                "text/",
                "/html",
                "text/html;",
                "text/html; charset",
                "text/html; charset=",
                "text html",
                "text/html/x",
                "text/html; charset=\"open",
                "text/html; charset=a b",
            ],
        ),
        (
            s().object_id(),
            V::ObjectId,
            &["507f1f77bcf86cd799439011", "507F1F77BCF86CD799439011"],
            &[
                "",
                "507f1f77bcf86cd79943901",
                "507f1f77bcf86cd7994390111",
                "507f1f77bcf86cd79943901g",
            ],
        ),
        (
            s().timezone(),
            V::Timezone,
            &[
                "UTC",
                "Europe/Berlin",
                "America/Argentina/Buenos_Aires",
                "Etc/GMT+5",
            ],
            &[
                "",
                "europe/berlin",
                "Europe//Berlin",
                "Europe/Berlin/",
                "A/B/C/D",
                "Europe/Ber lin",
            ],
        ),
        (
            s().cuid2(),
            V::Cuid2,
            &[
                "clh3a8e9g000008l5fkbc1z1n",
                "abc123def456",
                "tz4a98xxat96iws9zmbrgj3a",
            ],
            &[
                "",
                "a",
                "ABC123",
                "abc-123",
                "1abc",
                "é",
                "abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ],
        ),
        (
            s().ulid(),
            V::Ulid,
            &["01ARZ3NDEKTSV4RRFFQ69G5FAV", "7ZZZZZZZZZZZZZZZZZZZZZZZZZ"],
            &[
                "",
                "01ARZ3",
                "01ARZ3NDEKTSV4RRFFQ69GILOL",
                "01ARZ3NDEKTSV4RRFFQ69G5FAVX",
                "8ZZZZZZZZZZZZZZZZZZZZZZZZZ",
                "ZZZZZZZZZZZZZZZZZZZZZZZZZZ",
            ],
        ),
        (
            s().nanoid(),
            V::Nanoid,
            &["V1StGXR8_Z5jdHi6B-myT"],
            &["", "hello world", "é"],
        ),
        (
            s().emoji(),
            V::Emoji,
            &["😀", "⭐", "😀👍", "👍🏽", "❤\u{FE0F}", "👨\u{200D}👩"],
            &[
                "",
                "hello world",
                "hello 😀",
                "hello😀",
                "\u{FE0F}",
                "\u{200D}",
                "\u{FE0F}\u{200D}",
            ],
        ),
    ];
    #[cfg(feature = "regex")]
    rows.push((
        s().regex_syntax(),
        V::RegexSyntax,
        &["^[a-z]+$", r"\d{3}-\d{4}", "(a|b)*", ""][..],
        &["(", "[a-", "a{2,1}", "*a", r"\"][..],
    ));
    #[cfg(feature = "unicode")]
    rows.push((
        s().single_script(),
        V::SingleScript,
        &["paypal", "привет", "日本語のテキスト", "abc 123!"][..],
        // Latin `p`, `y`, `l` with a Cyrillic `а`; mixed words.
        &["p\u{430}ypal", "abc абв", "Ωmega"][..],
    ));
    // With `jiff` the name must exist in the time zone database.
    #[cfg(feature = "jiff")]
    rows.push((
        s().timezone(),
        V::Timezone,
        &["Asia/Kolkata", "America/Indiana/Indianapolis"][..],
        &["Mars/Olympus_Mons", "Europe/Atlantis", "EUROPE/BERLIN"][..],
    ));
    #[cfg(feature = "string-advanced")]
    rows.extend([
        (
            s().url_strict(),
            V::UrlStrict,
            &["https://example.com/a", "http://[2606:4700::1111]/"][..],
            &[
                "",
                "ftp://example.com/",
                "http://[::1]/",
                "http://0.0.0.0/",
                "http://localhost./",
                "http://LOCALHOST/",
                "http://[::ffff:192.168.0.1]/",
                "http://[::ffff:127.0.0.1]/",
                "http://[fe80::1]/",
                "http://10.0.0.1/",
                "http://127.0.0.1/",
            ][..],
        ),
        (
            s().uri(),
            V::Uri,
            &["https://example.com/path", "urn:isbn:0451450523"],
            &["", "not a uri", "/relative/path"],
        ),
        (
            s().uuid_v1(),
            V::UuidV1,
            &["c232ab00-9414-11ec-b3c8-9f6bdeced846"],
            &[
                "",
                "550e8400-e29b-41d4-a716-446655440000",
                "01890f57-5a7b-7f8b-bfd3-63f8e7c6f4b8",
            ],
        ),
        (
            s().uuid_v4(),
            V::UuidV4,
            &["550e8400-e29b-41d4-a716-446655440000"],
            &[
                "",
                "c232ab00-9414-11ec-b3c8-9f6bdeced846",
                "01890f57-5a7b-7f8b-bfd3-63f8e7c6f4b8",
            ],
        ),
        (
            s().uuid_v7(),
            V::UuidV7,
            &["01890f57-5a7b-7f8b-bfd3-63f8e7c6f4b8"],
            &["", "550e8400-e29b-41d4-a716-446655440000"],
        ),
        (
            s().semver_full(),
            V::SemverFull,
            &["1.2.3", "1.2.3-alpha.1+build.5"],
            &["", "v1.2.3", "1.2", "01.2.3"],
        ),
        (
            s().phone_e164_strict(),
            V::PhoneE164,
            &["+14155552671", "+442071838750"],
            &["", "14155552671", "+1", "+1abc"],
        ),
    ]);
    rows
}

/// Given a string schema for each format (`string-advanced` adds the strict URL, URI, versioned
/// UUIDs, full semver and strict E.164)
/// When valid and near-valid inputs are parsed
/// Then valid ones come back unchanged, and each invalid one is exactly one root `invalid_string`
/// issue naming the format, with the input as `received`.
#[test]
fn every_string_format_accepts_and_rejects() {
    for (schema, validation, ok, bad) in rows() {
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

    // 253 characters is the total hostname limit (each label here is a valid 9).
    let long = ["abcdefghi"; 26].join(".");
    let err = vld::string()
        .hostname()
        .parse_value(&json!(long))
        .unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidString {
            validation: V::Hostname
        }
    );
}
