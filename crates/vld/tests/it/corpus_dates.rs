//! Corpus and stories: `date()`, `datetime()` and `time()` of every enabled backend
//! (`vld::chrono`, `vld::jiff`, `vld::time`; all three can be on at once). The same suite runs
//! per backend, with backend values obtained by parsing; a differential checks that
//! `string().iso_datetime()` and every backend's `datetime()` share one grammar.

macro_rules! backend_suite {
    ($backend:ident, $date_ty:ty, $time_backend:ty) => {
        mod $backend {
            use crate::common::*;
            use serde_json::json;
            use vld::error::{DateKind, IssueCode, SizeOrigin};
            use vld::prelude::*;
            use vld::$backend::{date, datetime};

            fn d(s: &str) -> <vld::$backend::ZDate as VldSchema>::Output {
                date().parse_value(&json!(s)).unwrap()
            }

            fn dt(s: &str) -> <vld::$backend::ZDateTime as VldSchema>::Output {
                datetime().parse_value(&json!(s)).unwrap()
            }

            fn root(code: &str) -> Vec<(String, String)> {
                vec![(String::new(), code.to_string())]
            }

            #[track_caller]
            fn first<T: std::fmt::Debug>(r: Result<T, VldError>) -> IssueCode {
                r.unwrap_err().issues[0].code.clone()
            }

            /// Given `date()`, bare, bounded by literals and by backend values, and `past` /
            /// `future`
            /// When strict ISO dates, near-misses (wrong widths, impossible days, times, signs,
            /// spaces, non-ASCII digits) and non-strings are parsed
            /// Then only strict `YYYY-MM-DD` calendar dates are accepted (leap days included,
            /// years 0001–9999) and print back unchanged; every near-miss is `invalid_date`
            /// (date); non-strings are `invalid_type` expecting `date`; bounds are inclusive and
            /// report the bound (also in the default and translated message); past / future reject
            /// the other side; custom messages replace the defaults.
            #[test]
            fn date_corpus() {
                for ok in ["2024-06-15", "2024-02-29", "2000-02-29"] {
                    assert_eq!(d(ok).to_string(), ok);
                }
                for ok in ["0001-01-01", "9999-12-31"] {
                    assert!(date().parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                for bad in [
                    "", "06/15/2024", "2024-6-5", "2024-06-5", "2023-02-29", "1900-02-29",
                    "2024-02-31", "2024-04-31", "2024-13-01", "2024-00-10", "2024-01-00",
                    "2024-06-15T00:00:00Z", "2024-01-01T", " 2024-06-15", "+2024-06-15",
                    "2024-06-15é", "２０２４-06-15", "é", "2024x06-15", "2024-06x15",
                ] {
                    assert_eq!(
                        first(date().parse_value(&json!(bad))),
                        IssueCode::InvalidDate { kind: DateKind::Date },
                        "{bad:?}"
                    );
                }
                for (bad, received) in [(json!(42), "number"), (json!(null), "null"), (json!(true), "boolean")] {
                    let err = date().parse_value(&bad).unwrap_err();
                    assert_eq!(issues(&err), root("invalid_type"));
                    assert_eq!(
                        err.issues[0].code,
                        IssueCode::InvalidType { expected: "date".into(), received: received.into() }
                    );
                }
                let err = date().type_error("Date required!").parse_value(&json!(true)).unwrap_err();
                assert_eq!(err.issues[0].message, "Date required!");

                let year = date().min("2024-01-01").max("2024-12-31");
                for ok in ["2024-01-01", "2024-07-04", "2024-12-31"] {
                    assert!(year.parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                let err = year.parse_value(&json!("2023-12-31")).unwrap_err();
                assert_eq!(
                    err.issues[0].code,
                    IssueCode::TooSmall {
                        origin: SizeOrigin::Date,
                        minimum: json!("2024-01-01"),
                        inclusive: true,
                    }
                );
                assert!(err.issues[0].message.contains("2024-01-01"), "{err}");
                let tr = vld::i18n::translate_error(&err, &vld::i18n::english());
                assert!(tr.issues[0].message.contains("2024-01-01"));
                assert!(matches!(
                    first(year.parse_value(&json!("2025-01-01"))),
                    IssueCode::TooBig { origin: SizeOrigin::Date, maximum, .. } if maximum == json!("2024-12-31")
                ));

                let jan = date().min_date(d("2024-01-01")).max_date(d("2024-01-31"));
                assert!(jan.parse_value(&json!("2024-01-15")).is_ok());
                assert_eq!(issues(&jan.parse_value(&json!("2023-12-31")).unwrap_err()), root("too_small"));
                assert_eq!(issues(&jan.parse_value(&json!("2024-02-01")).unwrap_err()), root("too_big"));

                assert!(date().past().parse_value(&json!("1900-01-01")).is_ok());
                assert_eq!(
                    first(date().past().parse_value(&json!("2999-01-01"))),
                    IssueCode::NotInPast { kind: DateKind::Date }
                );
                assert!(date().future().parse_value(&json!("2999-01-01")).is_ok());
                for before_today in ["1900-01-01", "2000-01-01"] {
                    assert_eq!(
                        first(date().future().parse_value(&json!(before_today))),
                        IssueCode::NotInFuture { kind: DateKind::Date }
                    );
                }
                let msg = |s: vld::$backend::ZDate, v: &str| s.parse_value(&json!(v)).unwrap_err().issues[0].message.clone();
                assert_eq!(msg(date().past_msg("past only"), "2999-01-01"), "past only");
                assert_eq!(msg(date().future_msg("future only"), "1900-01-01"), "future only");
            }

            /// Given `datetime()`, bare and with every timezone policy, bounds and relative checks
            /// When RFC 3339 timestamps (offsets, fractions, lower-case `t`/`z`, naive), near-misses
            /// and timestamps near the edges of the representable range are parsed
            /// Then only RFC 3339 is accepted (naive input is UTC unless a naive offset is set, an
            /// explicit offset wins) and values normalize to UTC; near-misses and instants outside
            /// 0000-01-01Z..9999-12-30T22:00Z (after applying offsets, also the naive one) are
            /// `invalid_date` (datetime); `with_timezone_only` / `naive_allowed(false)` reject naive
            /// input unless re-allowed; `timezone_offset_only` demands one offset
            /// (`invalid_timezone_offset` naming it, custom message kept); bounds compare instants
            /// and report the bound; past / future reject the other side.
            #[test]
            fn datetime_corpus() {
                for ok in [
                    "2024-06-15T12:00:00Z",
                    "2024-06-15T12:00:00+03:00",
                    "2024-06-15T12:00:00.123456789-05:30",
                    "2024-06-15t12:00:00z",
                    "2024-06-15T12:00:00",
                    "2024-06-15T12:00:00.5",
                    "9999-12-30T22:00:00.999999999Z",
                    "9999-12-31T00:00:00+02:00",
                    "0000-01-01T00:00:00Z",
                    "0000-01-01T00:30:00-01:00",
                    "0000-01-01T00:30:00+00:30",
                ] {
                    if let Err(e) = datetime().parse_value(&json!(ok)) {
                        panic!("{ok} rejected: {e}");
                    }
                }
                for bad in [
                    "", "not-a-date", "2024-06-15", "2024-01-01T", "2024-06-15T12:00",
                    "2024-06-15T12:00Z", "2024-06-15 12:00:00Z", "2024-6-15T12:00:00Z",
                    "2024-06-15T12:00:00+0300", "2024-06-15T12:00:00Zjunk",
                    "2024-06-15T12:00:00.Z", "2024-06-15T12:00:00.1234567890Z",
                    "2024-06-15T24:00:00Z", "2024-02-31T12:00:00Z", "2024-06-15T12:00:00[UTC]",
                    "2024-06-15T1é:00:00Z", "2024-06-15T12:30x00Z", "2024-06-15T12:30:5Z",
                    "2024-06-15T12:30:Z",
                    "9999-12-31T23:00:00-05:00", "9999-12-31T00:00:00Z", "9999-12-30T22:00:01Z",
                    "0000-01-01T00:30:00+01:00",
                ] {
                    assert_eq!(
                        first(datetime().parse_value(&json!(bad))),
                        IssueCode::InvalidDate { kind: DateKind::DateTime },
                        "{bad:?}"
                    );
                }
                let shifted = datetime().naive_timezone_offset(-5 * 3600);
                assert_eq!(issues(&shifted.parse_value(&json!("9999-12-31T23:00:00")).unwrap_err()), root("invalid_date"));
                let shifted = datetime().naive_timezone_offset(3600);
                assert_eq!(issues(&shifted.parse_value(&json!("0000-01-01T00:00:00")).unwrap_err()), root("invalid_date"));
                assert!(matches!(
                    first(datetime().parse_value(&json!(123))),
                    IssueCode::InvalidType { expected, .. } if expected == "datetime"
                ));
                let err = datetime().type_error("Datetime required!").parse_value(&json!(null)).unwrap_err();
                assert_eq!(err.issues[0].message, "Datetime required!");

                // Offsets normalize to UTC; naive input is UTC unless told otherwise.
                assert!(dt("2024-06-15T12:30:00+03:00") == dt("2024-06-15T09:30:00Z"));
                assert!(dt("2024-06-15T12:30:00+05:45") == dt("2024-06-15T06:45:00Z"));
                assert!(dt("2024-06-15T12:30:00-01:15") == dt("2024-06-15T13:45:00Z"));
                assert!(dt("2024-06-15T12:30:00.123Z") != dt("2024-06-15T12:30:00Z"));
                assert!(dt("2024-06-15T12:00:00") == dt("2024-06-15T12:00:00Z"));
                let msk = datetime().naive_timezone_offset(3 * 3600);
                assert!(msk.parse_value(&json!("2024-06-15T12:00:00")).unwrap() == dt("2024-06-15T09:00:00Z"));
                assert!(msk.parse_value(&json!("2024-06-15T12:00:00Z")).unwrap() == dt("2024-06-15T12:00:00Z"));

                for strict in [datetime().with_timezone_only(), datetime().naive_allowed(false)] {
                    assert!(strict.parse_value(&json!("2024-06-15T12:30:00Z")).is_ok());
                    assert_eq!(
                        first(strict.parse_value(&json!("2024-06-15T12:30:00"))),
                        IssueCode::InvalidDate { kind: DateKind::DateTime }
                    );
                }
                let relaxed = datetime().with_timezone_only().naive_allowed(true);
                assert!(relaxed.parse_value(&json!("2024-06-15T12:30:00")).is_ok());

                let moscow = datetime().timezone_offset_only(3 * 3600);
                assert!(moscow.parse_value(&json!("2024-06-15T12:30:00+03:00")).is_ok());
                for bad in ["2024-06-15T12:30:00+02:00", "2024-06-15T12:30:00Z", "2024-06-15T12:30:00"] {
                    let err = moscow.parse_value(&json!(bad)).unwrap_err();
                    assert_eq!(
                        err.issues[0].code,
                        IssueCode::InvalidTimezoneOffset { expected: "+03:00".into() },
                        "{bad}"
                    );
                    assert!(err.issues[0].message.contains("+03:00"), "{err}");
                }
                let est = datetime().timezone_offset_only_msg(-5 * 3600, "EST only");
                let err = est.parse_value(&json!("2024-06-15T12:30:00Z")).unwrap_err();
                assert_eq!(err.issues[0].message, "EST only");
                assert_eq!(err.issues[0].code, IssueCode::InvalidTimezoneOffset { expected: "-05:00".into() });

                let year = datetime().min("2024-01-01T00:00:00Z").max("2024-12-31T23:59:59Z");
                assert!(year.parse_value(&json!("2024-06-15T12:00:00Z")).is_ok());
                assert!(year.parse_value(&json!("2024-01-01T03:00:00+03:00")).is_ok());
                assert!(matches!(
                    first(year.parse_value(&json!("2023-12-31T23:59:59Z"))),
                    IssueCode::TooSmall { origin: SizeOrigin::DateTime, .. }
                ));
                // 2024-01-01T02:59:59+03:00 is 2023-12-31T23:59:59Z.
                assert_eq!(issues(&year.parse_value(&json!("2024-01-01T02:59:59+03:00")).unwrap_err()), root("too_small"));
                let err = year.parse_value(&json!("2025-01-01T00:00:00Z")).unwrap_err();
                assert!(matches!(&err.issues[0].code, IssueCode::TooBig { origin: SizeOrigin::DateTime, .. }));
                assert!(err.issues[0].message.contains("2024-12-31"), "{err}");

                let day = datetime().min_datetime(dt("2024-01-01T00:00:00Z")).max_datetime(dt("2024-01-02T00:00:00Z"));
                assert!(day.parse_value(&json!("2024-01-01T12:00:00Z")).is_ok());
                assert_eq!(issues(&day.parse_value(&json!("2024-01-03T00:00:00Z")).unwrap_err()), root("too_big"));

                assert_eq!(
                    first(datetime().past().parse_value(&json!("2999-01-01T00:00:00Z"))),
                    IssueCode::NotInPast { kind: DateKind::DateTime }
                );
                let past = datetime().past_msg("in the past");
                assert!(past.parse_value(&json!("2000-01-01T00:00:00Z")).is_ok());
                let err = past.parse_value(&json!("2999-01-01T00:00:00Z")).unwrap_err();
                assert_eq!(err.issues[0].code, IssueCode::NotInPast { kind: DateKind::DateTime });
                assert_eq!(err.issues[0].message, "in the past");
                assert!(datetime().future().parse_value(&json!("2999-01-01T00:00:00Z")).is_ok());
                assert_eq!(
                    first(datetime().future().parse_value(&json!("2000-01-01T00:00:00Z"))),
                    IssueCode::NotInFuture { kind: DateKind::DateTime }
                );
                let err = datetime().future_msg("later").parse_value(&json!("2000-01-01T00:00:00Z")).unwrap_err();
                assert_eq!(err.issues[0].message, "later");
            }

            /// Given `time()` of this backend, bare and bounded by literals and by a parsed value
            /// When times of day (with up to 9 fraction digits), near-misses and a non-string are
            /// parsed
            /// Then `HH:MM:SS[.fff]` is accepted, near-misses (24h, 60 min/sec, no seconds, zone
            /// suffixes, one-digit hour, empty / too long fraction, spaces, non-ASCII) are
            /// `invalid_date` (time), a number is `invalid_type` (`type_error` replaces its
            /// message), and bounds are inclusive with the bound zero-padded in the code and the
            /// default message; a bound from a backend value and a JSON Schema default use the same
            /// text, which parses back.
            #[test]
            fn time_of_day_corpus() {
                let t = vld::primitives::date::ZTime::<$time_backend>::new;
                for ok in ["00:00:00", "23:59:59", "12:30:45.123456789"] {
                    assert!(t().parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                for bad in [
                    "24:00:00", "12:60:00", "12:00:60", "12:00", "12:00:00Z", "12:00:00+01:00",
                    "1:00:00", "12:00:00.", "12:00:00.1234567890", "12:00:00 ", "", "é",
                    "12:30x00", "12:30:", "12:30:5",
                ] {
                    assert_eq!(
                        first(t().parse_value(&json!(bad))),
                        IssueCode::InvalidDate { kind: DateKind::Time },
                        "{bad}"
                    );
                }
                assert_eq!(issues(&t().parse("1200").unwrap_err()), root("invalid_type"));
                let err = t().type_error("time!").parse("1200").unwrap_err();
                assert_eq!(err.issues[0].message, "time!");

                let office = t().min("08:00:00").max("17:30:00");
                assert!(office.parse(r#""08:00:00""#).is_ok());
                assert!(office.parse(r#""17:30:00""#).is_ok());
                let err = office.parse(r#""07:59:59.999""#).unwrap_err();
                assert_eq!(
                    err.issues[0].code,
                    IssueCode::TooSmall { origin: SizeOrigin::Time, minimum: json!("08:00:00"), inclusive: true }
                );
                assert_eq!(err.issues[0].message, "Time must be at or after 08:00:00");
                let err = office.parse(r#""17:30:00.5""#).unwrap_err();
                assert_eq!(err.issues[0].message, "Time must be at or before 17:30:00");

                let at_again = || t().parse(r#""10:15:00""#).unwrap();
                let at = at_again();
                let exact = t().min_time(at.clone()).max_time(at);
                assert!(exact.parse(r#""10:15:00""#).is_ok());
                // A bound from a backend value is reported in a form `time()` parses back.
                let err = exact.parse(r#""10:15:01""#).unwrap_err();
                let IssueCode::TooBig { maximum, .. } = &err.issues[0].code else { panic!("{err}") };
                assert!(t().parse_value(maximum).unwrap() == at_again(), "{maximum}");

                // The JSON Schema default is the same text, and parses back.
                #[cfg(feature = "openapi")]
                {
                    use vld::openapi::JsonSchema;
                    let js = t().with_default(at_again()).json_schema();
                    assert_eq!(&js["default"], maximum);
                }
            }

            /// Given `datetime().unix_seconds()` / `.unix_millis()`, bare, bounded, past / future and
            /// coercing
            /// When integers at and beyond the common range, whole floats, fractions, strings and
            /// other JSON arrive
            /// Then integers in `0000-01-01T00:00:00Z ..= 9999-12-30T22:00:00(.999)Z` give the
            /// instant (milliseconds keep their fraction), anything beyond is `invalid_date`, a
            /// fraction `not_int`, a string `invalid_type` (`integer`) unless `coerce()` accepts
            /// digits with an optional `-`; bounds and past / future apply as for strings; the JSON
            /// Schema is an integer with the range (bounds rounded inward) and a default prints as
            /// the integer.
            #[test]
            fn unix_timestamps() {
                let secs = datetime().unix_seconds();
                for (n, want) in [
                    (json!(0), "1970-01-01T00:00:00Z"),
                    (json!(1_700_000_000), "2023-11-14T22:13:20Z"),
                    (json!(-1), "1969-12-31T23:59:59Z"),
                    (json!(1_000.0), "1970-01-01T00:16:40Z"),
                    (json!(-62_167_219_200i64), "0000-01-01T00:00:00Z"),
                    (json!(253_402_207_200i64), "9999-12-30T22:00:00Z"),
                ] {
                    assert_eq!(secs.parse_value(&n).unwrap(), dt(want), "{n}");
                }
                for bad in [
                    json!(-62_167_219_201i64),
                    json!(253_402_207_201i64),
                    json!(i64::MAX),
                    json!(i64::MIN),
                    json!(u64::MAX),
                    json!(1e300),
                ] {
                    assert_eq!(first(secs.parse_value(&bad)), IssueCode::InvalidDate { kind: DateKind::DateTime }, "{bad}");
                }
                assert_eq!(issues(&secs.parse_value(&json!(1.5)).unwrap_err()), root("not_int"));
                for bad in [json!("1700000000"), json!(null), json!(true), json!([1])] {
                    let err = secs.parse_value(&bad).unwrap_err();
                    assert!(matches!(&err.issues[0].code, IssueCode::InvalidType { expected, .. } if expected == "integer"), "{bad}");
                }
                let coerced = datetime().unix_seconds().coerce();
                assert_eq!(coerced.parse_value(&json!("1700000000")).unwrap(), dt("2023-11-14T22:13:20Z"));
                assert_eq!(coerced.parse_value(&json!("-1")).unwrap(), dt("1969-12-31T23:59:59Z"));
                assert_eq!(first(coerced.parse_value(&json!("99999999999999999999999999"))), IssueCode::InvalidDate { kind: DateKind::DateTime });
                for bad in ["", "-", "+1", "1e3", " 1", "1.0", "１"] {
                    assert_eq!(issues(&coerced.parse_value(&json!(bad)).unwrap_err()), root("invalid_type"), "{bad:?}");
                }

                let ms = datetime().unix_millis();
                assert_eq!(ms.parse_value(&json!(1_700_000_000_123i64)).unwrap(), dt("2023-11-14T22:13:20.123Z"));
                assert_eq!(ms.parse_value(&json!(-1)).unwrap(), dt("1969-12-31T23:59:59.999Z"));
                assert!(ms.parse_value(&json!(-62_167_219_200_000i64)).is_ok());
                assert_eq!(ms.parse_value(&json!(253_402_207_200_999i64)).unwrap(), dt("9999-12-30T22:00:00.999Z"));
                for bad in [-62_167_219_200_001i64, 253_402_207_201_000] {
                    assert_eq!(first(ms.parse_value(&json!(bad))), IssueCode::InvalidDate { kind: DateKind::DateTime }, "{bad}");
                }

                let y2k = datetime().unix_seconds().min("2000-01-01T00:00:00Z").max("2000-12-31T23:59:59Z");
                assert!(y2k.parse_value(&json!(946_684_800)).is_ok());
                assert!(y2k.parse_value(&json!(978_307_199)).is_ok());
                assert_eq!(issues(&y2k.parse_value(&json!(946_684_799)).unwrap_err()), root("too_small"));
                assert_eq!(issues(&y2k.parse_value(&json!(978_307_200)).unwrap_err()), root("too_big"));
                assert!(datetime().unix_seconds().past().parse_value(&json!(0)).is_ok());
                assert_eq!(
                    first(datetime().unix_seconds().past().parse_value(&json!(253_402_207_200i64))),
                    IssueCode::NotInPast { kind: DateKind::DateTime }
                );
                assert_eq!(
                    first(datetime().unix_millis().future().parse_value(&json!(0))),
                    IssueCode::NotInFuture { kind: DateKind::DateTime }
                );

                #[cfg(feature = "openapi")]
                {
                    use vld::openapi::JsonSchema;
                    assert_eq!(
                        secs.json_schema(),
                        json!({
                            "type": "integer",
                            "format": "unix-time",
                            "description": "Unix timestamp in seconds",
                            "minimum": -62_167_219_200i64,
                            "maximum": 253_402_207_200i64,
                        })
                    );
                    let narrow = datetime()
                        .unix_millis()
                        .min("2000-01-01T00:00:00.0005Z")
                        .max("2000-01-01T00:00:01.9999Z")
                        .past();
                    assert_eq!(
                        narrow.json_schema(),
                        json!({
                            "type": "integer",
                            "format": "unix-time-ms",
                            "description": "Unix timestamp in milliseconds",
                            "minimum": 946_684_800_001i64,
                            "maximum": 946_684_801_999i64,
                            "x-past": true,
                        })
                    );
                    let js = datetime().unix_seconds().with_default(dt("2023-11-14T22:13:20Z")).json_schema();
                    assert_eq!(js["default"], json!(1_700_000_000));
                    let validator = jsonschema::draft202012::new(&y2k.json_schema()).unwrap();
                    for n in [946_684_799i64, 946_684_800, 978_307_199, 978_307_200] {
                        assert_eq!(validator.is_valid(&json!(n)), y2k.parse_value(&json!(n)).is_ok(), "{n}");
                    }
                }
            }

            /// Given `vld::range()` of this backend's `date()`, `datetime()` and unix datetimes
            /// When ordered, equal and reversed pairs (also across offsets) arrive
            /// Then ends compare as instants, not text: equal ends pass, a reversed pair is one
            /// `range_order` at the end field; item issues are reported per field.
            #[test]
            fn date_ranges() {
                let days = vld::range(date());
                let r = days.parse_value(&json!({"start": "2024-01-01", "end": "2024-01-31"})).unwrap();
                assert_eq!((r.start().to_string(), r.end().to_string()), ("2024-01-01".into(), "2024-01-31".into()));
                assert!(days.parse_value(&json!({"start": "2024-01-01", "end": "2024-01-01"})).is_ok());
                assert_eq!(
                    issues(&days.parse_value(&json!({"start": "2024-02-01", "end": "2024-01-31"})).unwrap_err()),
                    [(".end".into(), "range_order".into())]
                );
                assert_eq!(
                    issues(&days.parse_value(&json!({"start": "2024-13-01", "end": "x"})).unwrap_err()),
                    [(".start".into(), "invalid_date".into()), (".end".into(), "invalid_date".into())]
                );
                let stay = vld::range(datetime().with_timezone_only()).keys("from", "to");
                // 08:00Z <= 09:00Z although the text sorts the other way.
                assert!(stay.parse_value(&json!({"from": "2024-01-01T10:00:00+02:00", "to": "2024-01-01T09:00:00Z"})).is_ok());
                assert_eq!(
                    issues(&stay.parse_value(&json!({"from": "2024-01-01T09:00:00Z", "to": "2024-01-01T10:00:00+02:00"})).unwrap_err()),
                    [(".to".into(), "range_order".into())]
                );
                let unix = vld::range(datetime().unix_seconds());
                assert!(unix.parse_value(&json!({"start": 0, "end": 1})).is_ok());
                assert_eq!(
                    issues(&unix.parse_value(&json!({"start": 1, "end": 0})).unwrap_err()),
                    [(".end".into(), "range_order".into())]
                );
                #[cfg(feature = "openapi")]
                {
                    use vld::openapi::JsonSchema;
                    assert_eq!(
                        days.json_schema(),
                        json!({
                            "type": "object",
                            "properties": {
                                "start": {"type": "string", "format": "date"},
                                "end": {"type": "string", "format": "date"},
                            },
                            "required": ["start", "end"],
                            "x-rangeOrder": ["start", "end"],
                        })
                    );
                }
            }

            vld::schema! {
                #[derive(Debug)]
                pub struct Event {
                    pub title: String => vld::string().min(1),
                    pub date: $date_ty => vld::$backend::date().min("2020-01-01"),
                }
            }

            /// Given an `Event` declared with `vld::schema!` whose `date` field is this backend's
            /// date, and date schemas wrapped in `optional` / `with_default`
            /// When events and `null` are parsed, and (feature `openapi`) their JSON Schemas read
            /// Then fields parse to the backend type and both bad fields are reported in order;
            /// `null` is `None` / the default; the JSON Schemas are `string` with `date` /
            /// `date-time` format, and a date-time default is RFC 3339 that `datetime()` parses
            /// back to the same instant.
            #[test]
            fn event_with_dates() {
                let e = Event::parse(r#"{"title": "Conf", "date": "2024-09-15"}"#).unwrap();
                assert_eq!(e.date.to_string(), "2024-09-15");
                let err = Event::parse(r#"{"title": "", "date": "2019-01-01"}"#).unwrap_err();
                assert_eq!(
                    issues(&err),
                    [(".title".into(), "too_small".into()), (".date".into(), "too_small".into())]
                );

                assert_eq!(date().optional().parse("null").unwrap(), None);
                assert!(date().optional().parse(r#""2024-01-01""#).unwrap().is_some());
                assert_eq!(
                    date().with_default(d("2000-01-01")).parse("null").unwrap().to_string(),
                    "2000-01-01"
                );

                #[cfg(feature = "openapi")]
                {
                    use vld::openapi::JsonSchema;
                    assert_eq!(date().json_schema(), json!({"type": "string", "format": "date"}));
                    assert_eq!(
                        datetime().with_timezone_only().json_schema(),
                        json!({"type": "string", "format": "date-time"})
                    );
                    assert_eq!(date().with_default(d("2024-01-02")).json_schema()["default"], "2024-01-02");
                    let instant = dt("2024-01-02T03:04:05Z");
                    for (s, zoned) in [(datetime(), false), (datetime().with_timezone_only(), true)] {
                        let js = s.with_default(instant.clone()).json_schema();
                        if zoned {
                            assert_eq!(js["format"], "date-time");
                        }
                        let default = js["default"].clone();
                        assert!(
                            datetime().parse_value(&default).is_ok_and(|v| v == instant),
                            "default {default} does not round-trip"
                        );
                    }
                }
            }
        }
    };
}

#[cfg(feature = "chrono")]
backend_suite!(chrono, ::chrono::NaiveDate, vld::chrono::Chrono);
#[cfg(feature = "jiff")]
backend_suite!(jiff, ::jiff::civil::Date, vld::jiff::Jiff);
#[cfg(feature = "time")]
backend_suite!(time, ::time::Date, vld::time::Time);

use serde_json::json;
use vld::prelude::*;

/// Given `string().iso_datetime()` and every enabled backend's `datetime()`
/// When the same timestamps are parsed by each
/// Then they agree on every verdict: the string schema returns the input unchanged or one
/// `invalid_string`, each backend parses or reports one `invalid_date`; a separator out of place
/// fails `string().iso_date()` / `iso_time()` too.
#[test]
fn iso_datetime_string_and_backends_share_one_grammar() {
    let cases = [
        ("2024-01-15T10:30:00Z", true),
        ("2024-01-15t10:30:00z", true),
        ("2024-01-15T10:30:00.5+02:00", true),
        ("2024-01-15T10:30:00", true),
        ("2024-01-15T10:30Z", false),
        ("2024-01-15T10:30", false),
        ("2024-01-15 10:30:00Z", false),
        ("2024x01-15T10:30:00Z", false),
        ("2024-01x15T10:30:00Z", false),
        ("2024-01-15T10:30x00Z", false),
        ("2024-01-15T10:30:0Z", false),
    ];
    for (input, ok) in cases {
        let v = json!(input);
        match vld::string().iso_datetime().parse_value(&v) {
            Ok(out) => assert!(ok && out == input, "{input}"),
            Err(e) => assert!(
                !ok && crate::common::issues(&e)[0].1 == "invalid_string",
                "{input}"
            ),
        }
        let backends: Vec<Result<(), VldError>> = vec![
            #[cfg(feature = "chrono")]
            vld::chrono::datetime().parse_value(&v).map(drop),
            #[cfg(feature = "jiff")]
            vld::jiff::datetime().parse_value(&v).map(drop),
            #[cfg(feature = "time")]
            vld::time::datetime().parse_value(&v).map(drop),
        ];
        for r in backends {
            match r {
                Ok(()) => assert!(ok, "{input} accepted by a backend"),
                Err(e) => assert!(
                    !ok && crate::common::issues(&e)
                        == [(String::new(), "invalid_date".to_string())],
                    "{input}: {e}"
                ),
            }
        }
    }
    for bad in ["2024x06-15", "2024-06x15"] {
        assert!(
            vld::string().iso_date().parse_value(&json!(bad)).is_err(),
            "{bad}"
        );
    }
    for bad in ["12:30x00", "12:30:", "12:30:5"] {
        assert!(
            vld::string().iso_time().parse_value(&json!(bad)).is_err(),
            "{bad}"
        );
    }
    assert!(
        vld::string()
            .iso_date()
            .parse_value(&json!("2024-06-15"))
            .is_ok()
    );
    assert!(
        vld::string()
            .iso_time()
            .parse_value(&json!("12:30:05"))
            .is_ok()
    );
}

/// Given more than one date backend enabled, or exactly one
/// When dates and times are parsed through each backend module and through the `vld::date()` /
/// `vld::time_of_day()` shortcuts, and `chrono::time()` is built with an impossible bound
/// Then each module keeps its own output type (all print the same date); the shortcuts exist only
/// with exactly one backend and behave like it; backend times convert to the backend's own value
/// (the `time` crate's bound prints `08:00:00.0`); an invalid time literal panics when the schema
/// is built.
#[test]
fn backends_coexist_and_shortcuts() {
    #[cfg(all(feature = "chrono", feature = "jiff", feature = "time"))]
    {
        let c: ::chrono::NaiveDate = vld::chrono::date().parse(r#""2024-06-15""#).unwrap();
        let j: ::jiff::civil::Date = vld::jiff::date().parse(r#""2024-06-15""#).unwrap();
        let t: ::time::Date = vld::time::date().parse(r#""2024-06-15""#).unwrap();
        assert_eq!(c.to_string(), j.to_string());
        assert_eq!(j.to_string(), t.to_string());
    }
    #[cfg(all(feature = "chrono", not(feature = "jiff"), not(feature = "time")))]
    {
        let d: ::chrono::NaiveDate = vld::date().parse(r#""2024-06-15""#).unwrap();
        let _: ZDate = vld::date();
        assert_eq!(d.to_string(), "2024-06-15");
    }
    #[cfg(any(
        all(feature = "chrono", not(feature = "jiff"), not(feature = "time")),
        all(feature = "jiff", not(feature = "chrono"), not(feature = "time")),
        all(feature = "time", not(feature = "chrono"), not(feature = "jiff"))
    ))]
    {
        let s: ZTime = vld::time_of_day().max("12:00:00");
        assert_eq!(
            s.parse(r#""11:59:59""#).unwrap(),
            vld::time_of_day().parse(r#""11:59:59""#).unwrap()
        );
        assert_eq!(
            crate::common::issues(&s.parse(r#""12:00:01""#).unwrap_err())[0].1,
            "too_big"
        );
    }
    #[cfg(feature = "chrono")]
    {
        assert_eq!(
            vld::chrono::time().parse(r#""09:05:01.5""#).unwrap(),
            ::chrono::NaiveTime::from_hms_milli_opt(9, 5, 1, 500).unwrap()
        );
        let panic =
            std::panic::catch_unwind(|| drop(vld::chrono::time().min("25:00:00"))).unwrap_err();
        let text = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(text.contains("Invalid time literal"), "{text}");
    }
    #[cfg(feature = "jiff")]
    assert_eq!(
        vld::jiff::time().parse(r#""09:05:01""#).unwrap(),
        ::jiff::civil::time(9, 5, 1, 0)
    );
    #[cfg(feature = "time")]
    {
        assert_eq!(
            vld::time::time().parse(r#""09:05:01.25""#).unwrap(),
            ::time::Time::from_hms_milli(9, 5, 1, 250).unwrap()
        );
        let s = vld::time::time().min_time(::time::Time::from_hms(8, 0, 0).unwrap());
        let err = s.parse(r#""07:00:00""#).unwrap_err();
        assert!(err.issues[0].message.ends_with("08:00:00.0"), "{err}");
    }
}
