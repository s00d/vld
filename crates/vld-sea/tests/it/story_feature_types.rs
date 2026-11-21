//! Stories and a model for the column types behind vld-sea's type features
//! (`with-json`, `with-chrono`, `with-time`, `with-uuid`, `with-rust_decimal`,
//! `with-bigdecimal`, `postgres-array`; tested together) and, on SeaORM 2,
//! native enums.
#![cfg(all(
    feature = "with-json",
    feature = "with-chrono",
    feature = "with-time",
    feature = "with-uuid",
    feature = "with-rust_decimal",
    feature = "with-bigdecimal",
    feature = "postgres-array"
))]

use proptest::prelude::*;
use serde_json::json;
use vld_sea::prelude::*;
use vld_sea::sea_orm::prelude::*;
use vld_sea::sea_orm::{ActiveModelTrait, Set};

pub mod typed {
    use sea_orm::entity::prelude::*;
    use vld_sea::sea_orm;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "typed")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub c_date: ChronoDate,
        pub c_time: ChronoTime,
        pub c_dt: ChronoDateTime,
        pub c_utc: ChronoDateTimeUtc,
        pub c_local: ChronoDateTimeLocal,
        pub c_tz: ChronoDateTimeWithTimeZone,
        pub t_date: TimeDate,
        pub t_time: TimeTime,
        pub t_dt: TimeDateTime,
        pub t_tz: TimeDateTimeWithTimeZone,
        pub id_u: Uuid,
        pub id_none: Option<Uuid>,
        pub dec: Decimal,
        pub big: BigDecimal,
        pub doc: Json,
        pub nums: Vec<i32>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

vld::schema! {
    #[derive(Debug)]
    pub struct TypedSchema {
        pub c_date: String => vld::string().min(10),
        pub id_u: String => vld::string().uuid(),
        pub dec: String => vld::string().min(1),
        pub nums: Vec<i64> => vld::array(vld::number().int().min(0)),
    }
}

fn empty() -> typed::ActiveModel {
    <typed::ActiveModel as ActiveModelTrait>::default()
}

/// Given a row with every feature-gated column type,
/// when its ActiveModel is turned into JSON and validated,
/// then dates / times are ISO 8601 strings (naive datetimes without offset,
/// zoned ones RFC 3339), UUIDs hyphenated, decimals as their exact strings,
/// JSON as-is, arrays as JSON arrays and a `None` as `null`; a schema over
/// those shapes accepts the row and rejects a negative array item at its
/// index.
#[test]
fn feature_types_map_to_json_like_serde() {
    use std::str::FromStr;
    let date = ChronoDate::from_ymd_opt(2024, 2, 29).unwrap();
    let c_dt = date.and_hms_opt(12, 30, 0).unwrap();
    let c_utc = c_dt.and_utc();
    let c_local: ChronoDateTimeLocal = c_utc.into();
    let c_tz = c_utc.fixed_offset();
    let t_date = TimeDate::from_calendar_date(2024, time::Month::February, 29).unwrap();
    let t_time = TimeTime::from_hms(12, 30, 0).unwrap();
    let am = typed::ActiveModel {
        c_date: Set(date),
        c_time: Set(c_dt.time()),
        c_dt: Set(c_dt),
        c_utc: Set(c_utc),
        c_local: Set(c_local),
        c_tz: Set(c_tz),
        t_date: Set(t_date),
        t_time: Set(t_time),
        t_dt: Set(t_date.with_time(t_time)),
        t_tz: Set(t_date.with_time(t_time).assume_utc()),
        id_u: Set(Uuid::nil()),
        id_none: Set(None),
        dec: Set("12.50".parse().unwrap()),
        big: Set(BigDecimal::from_str("1234567890.000000001").unwrap()),
        doc: Set(json!({"a": [1, 2]})),
        nums: Set(vec![1, 2, 3]),
        ..empty()
    };
    assert_eq!(
        active_model_to_json(&am),
        json!({
            "c_date": "2024-02-29",
            "c_time": "12:30:00",
            "c_dt": "2024-02-29T12:30:00",
            "c_utc": "2024-02-29T12:30:00+00:00",
            "c_local": c_local.to_rfc3339(),
            "c_tz": "2024-02-29T12:30:00+00:00",
            "t_date": "2024-02-29",
            "t_time": "12:30:00",
            "t_dt": "2024-02-29T12:30:00",
            "t_tz": "2024-02-29T12:30:00+00:00",
            "id_u": "00000000-0000-0000-0000-000000000000",
            "id_none": null,
            "dec": "12.50",
            "big": "1234567890.000000001",
            "doc": {"a": [1, 2]},
            "nums": [1, 2, 3],
        })
    );
    validate_active::<TypedSchema, _>(&am).unwrap();
    let bad = typed::ActiveModel {
        nums: Set(vec![1, -2]),
        ..am
    };
    assert_eq!(
        crate::common::issues(validate_active::<TypedSchema, _>(&bad)),
        crate::common::pairs(&[(".nums[1]", "too_small")])
    );
}

/// Given an entity with a native database enum column and a nullable one
/// (SeaORM 2 sends them as `Value::Enum`),
/// when its ActiveModel is turned into JSON,
/// then the variant is its string value and the missing one `null`.
#[cfg(feature = "sea-orm-2")]
#[test]
fn native_enums_map_to_their_string_value() {
    pub mod ticket {
        use sea_orm::entity::prelude::*;
        use vld_sea::sea_orm;

        #[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
        #[sea_orm(db_type = "Enum", enum_name = "status")]
        pub enum Status {
            #[sea_orm(string_value = "active")]
            Active,
            #[sea_orm(string_value = "closed")]
            Closed,
        }

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "tickets")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub status: Status,
            pub previous: Option<Status>,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }
    let am = ticket::ActiveModel {
        id: Set(1),
        status: Set(ticket::Status::Active),
        previous: Set(None),
    };
    assert_eq!(
        active_model_to_json(&am),
        json!({"id": 1, "status": "active", "previous": null})
    );
    let am = ticket::ActiveModel {
        previous: Set(Some(ticket::Status::Closed)),
        ..am
    };
    assert_eq!(active_model_to_json(&am)["previous"], "closed");
}

/// A wall-clock instant with an offset, drawn by proptest.
#[derive(Debug, Clone)]
struct Instant {
    y: i32,
    mo: u8,
    d: u8,
    h: u8,
    mi: u8,
    s: u8,
    nanos: u32,
    offset_min: i32,
}

fn instant() -> impl Strategy<Value = Instant> {
    let nanos = prop_oneof![
        Just(0u32),
        Just(500_000_000),
        Just(123_456_789),
        Just(1_000),
        0u32..1_000_000_000
    ];
    (
        0i32..=9999,
        1u8..=12,
        1u8..=28,
        0u8..24,
        0u8..60,
        0u8..60,
        nanos,
        -1439i32..=1439,
    )
        .prop_map(|(y, mo, d, h, mi, s, nanos, offset_min)| Instant {
            y,
            mo,
            d,
            h,
            mi,
            s,
            nanos,
            offset_min,
        })
}

proptest! {
    /// Model: `time` values map to `YYYY-MM-DD`, `HH:MM:SS[.fff[fff[fff]]]`
    /// (3, 6 or 9 fraction digits) and RFC 3339 with a `±HH:MM` offset:
    /// exactly what the chrono columns give for the same instant.
    #[test]
    fn model_time_values_map_to_iso_strings(i in instant()) {
        let t_date = TimeDate::from_calendar_date(i.y, time::Month::try_from(i.mo).unwrap(), i.d).unwrap();
        let t_time = TimeTime::from_hms_nano(i.h, i.mi, i.s, i.nanos).unwrap();
        let t_dt = t_date.with_time(t_time);
        let offset = time::UtcOffset::from_whole_seconds(i.offset_min * 60).unwrap();
        let am = typed::ActiveModel {
            t_date: Set(t_date),
            t_time: Set(t_time),
            t_dt: Set(t_dt),
            t_tz: Set(t_dt.assume_offset(offset)),
            ..empty()
        };
        let got = active_model_to_json(&am);

        let date = format!("{:04}-{:02}-{:02}", i.y, i.mo, i.d);
        let clock = format!("{:02}:{:02}:{:02}", i.h, i.mi, i.s);
        let clock = match i.nanos {
            0 => clock,
            n if n % 1_000_000 == 0 => format!("{clock}.{:03}", n / 1_000_000),
            n if n % 1_000 == 0 => format!("{clock}.{:06}", n / 1_000),
            n => format!("{clock}.{n:09}"),
        };
        let sign = if i.offset_min < 0 { '-' } else { '+' };
        let off = i.offset_min.abs();
        let zoned = format!("{date}T{clock}{sign}{:02}:{:02}", off / 60, off % 60);
        prop_assert_eq!(
            &got,
            &json!({"t_date": date, "t_time": clock, "t_dt": format!("{date}T{clock}"), "t_tz": zoned})
        );

        {
            let c_date = ChronoDate::from_ymd_opt(i.y, i.mo.into(), i.d.into()).unwrap();
            let c_time =
                ChronoTime::from_hms_nano_opt(i.h.into(), i.mi.into(), i.s.into(), i.nanos).unwrap();
            let c_dt = c_date.and_time(c_time);
            let c_tz = ChronoDateTimeWithTimeZone::parse_from_rfc3339(&zoned).unwrap();
            let chrono_am = typed::ActiveModel {
                c_date: Set(c_date),
                c_time: Set(c_time),
                c_dt: Set(c_dt),
                c_tz: Set(c_tz),
                ..empty()
            };
            let c = active_model_to_json(&chrono_am);
            prop_assert_eq!(
                [&c["c_date"], &c["c_time"], &c["c_dt"], &c["c_tz"]],
                [&got["t_date"], &got["t_time"], &got["t_dt"], &got["t_tz"]]
            );
        }
    }
}
