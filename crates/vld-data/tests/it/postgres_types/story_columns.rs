//! Stories: JSON and scalar columns written as query parameters and read
//! back from rows.

use crate::issues::{of, pairs};
use bytes::BytesMut;
use postgres_types::{FromSql, ToSql, Type};
use vld_data::postgres_types::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct ProfileSchema {
        pub nickname: String => vld::string().min(2),
        pub age: i64 => vld::number().int().min(0),
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Profile {
    nickname: String,
    age: i64,
}

vld::schema! {
    #[derive(Debug)]
    pub struct Score {
        pub value: i64 => vld::number().int().min(0).max(100),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Ratio {
        pub value: f64 => vld::number().min(0.0).max(1.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Code {
        pub value: String => vld::string().len(3),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Accepted {
        pub value: bool => vld::literal(true),
    }
}

fn wire<T: ToSql>(value: &T, ty: &Type) -> BytesMut {
    let mut out = BytesMut::new();
    value.to_sql_checked(ty, &mut out).unwrap();
    out
}

/// The validation issues inside a boxed `FromSql` error.
#[track_caller]
fn read_issues<T: std::fmt::Debug>(
    r: Result<T, Box<dyn std::error::Error + Sync + Send>>,
) -> Vec<(String, String)> {
    let e = r.unwrap_err();
    match e.downcast_ref::<VldPgError>() {
        Some(VldPgError::Validation(v)) => of(v),
        _ => panic!("expected a validation error, got {e:?}"),
    }
}

/// Given a validated profile for a `JSONB` / `JSON` column,
/// when it is sent as a parameter and read back from a row,
/// then `JSONB` carries the version byte `1` before the JSON, `JSON` does not,
/// both decode to the same value, and other column types are refused.
#[test]
fn json_column_round_trips() {
    let profile = Profile {
        nickname: "ada".into(),
        age: 36,
    };
    let v = Validated::<ProfileSchema, _>::new(profile.clone()).unwrap();

    let jsonb = wire(&v, &Type::JSONB);
    assert_eq!(jsonb[0], 1);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&jsonb[1..]).unwrap(),
        serde_json::json!({"nickname": "ada", "age": 36})
    );
    let json = wire(&v, &Type::JSON);
    assert_eq!(&json[..], &jsonb[1..]);

    for (ty, raw) in [(Type::JSONB, &jsonb), (Type::JSON, &json)] {
        let back = Validated::<ProfileSchema, Profile>::from_sql(&ty, raw).unwrap();
        assert_eq!(back.into_inner(), profile);
    }

    assert!(<Validated<ProfileSchema, Profile> as ToSql>::accepts(
        &Type::JSONB
    ));
    assert!(<Validated<ProfileSchema, Profile> as FromSql>::accepts(
        &Type::JSON
    ));
    assert!(!<Validated<ProfileSchema, Profile> as ToSql>::accepts(
        &Type::TEXT
    ));
    assert!(v.to_sql_checked(&Type::TEXT, &mut BytesMut::new()).is_err());
}

/// Given an invalid profile, or a row whose JSON another writer stored
/// without the schema,
/// when it is wrapped or read,
/// then `Validated::new` refuses it and `from_sql` fails with the schema's
/// issues (what `Row::try_get` returns); JSON that does not fit `T` is a
/// plain deserialization error.
#[test]
fn invalid_json_is_rejected_on_both_sides() {
    let err = Validated::<ProfileSchema, _>::new(Profile {
        nickname: "a".into(),
        age: -1,
    })
    .unwrap_err();
    assert!(matches!(&err, VldPgError::Validation(e)
        if of(e) == pairs(&[(".nickname", "too_small"), (".age", "too_small")])));

    let stored = br#"{"nickname":"x","age":7}"#;
    assert_eq!(
        read_issues(Validated::<ProfileSchema, Profile>::from_sql(
            &Type::JSON,
            stored
        )),
        pairs(&[(".nickname", "too_small")])
    );

    #[derive(Debug, serde::Deserialize)]
    struct Narrow {
        #[allow(dead_code)]
        age: u8,
    }
    let big = br#"{"nickname":"ada","age":1000}"#;
    let e = Validated::<ProfileSchema, Narrow>::from_sql(&Type::JSON, big).unwrap_err();
    assert!(e.downcast_ref::<serde_json::Error>().is_some(), "{e:?}");
}

/// Given the scalar wrappers for `INT8`, `FLOAT8`, `TEXT` and `BOOL`
/// columns,
/// when valid values are written and read back,
/// then the wire bytes are the inner type's, they decode to equal values,
/// and each wrapper accepts only its inner type's columns.
#[test]
fn scalar_columns_round_trip() {
    let score = VldInt::<Score>::new(42).unwrap();
    let raw = wire(&score, &Type::INT8);
    assert_eq!(&raw[..], &42_i64.to_be_bytes());
    assert_eq!(VldInt::<Score>::from_sql(&Type::INT8, &raw).unwrap(), score);
    assert!(!<VldInt<Score> as ToSql>::accepts(&Type::INT4));

    let ratio = VldFloat::<Ratio>::new(0.25).unwrap();
    let raw = wire(&ratio, &Type::FLOAT8);
    assert_eq!(&raw[..], &0.25_f64.to_be_bytes());
    assert_eq!(
        VldFloat::<Ratio>::from_sql(&Type::FLOAT8, &raw).unwrap(),
        ratio
    );

    let code = VldText::<Code>::new("abc").unwrap();
    for ty in [Type::TEXT, Type::VARCHAR] {
        let raw = wire(&code, &ty);
        assert_eq!(&raw[..], b"abc");
        assert_eq!(VldText::<Code>::from_sql(&ty, &raw).unwrap(), code);
    }
    assert!(!<VldText<Code> as FromSql>::accepts(&Type::INT8));

    let yes = VldBool::<Accepted>::new(true).unwrap();
    let raw = wire(&yes, &Type::BOOL);
    assert_eq!(&raw[..], &[1]);
    assert_eq!(
        VldBool::<Accepted>::from_sql(&Type::BOOL, &raw).unwrap(),
        yes
    );
}

/// Given column values outside their schemas (stored before the rule
/// existed),
/// when they are read,
/// then each wrapper's `from_sql` fails with the issue at `.value`, and a
/// stored NaN is `not_finite`.
#[test]
fn out_of_range_scalars_fail_on_read() {
    assert_eq!(
        read_issues(VldInt::<Score>::from_sql(
            &Type::INT8,
            &101_i64.to_be_bytes()
        )),
        pairs(&[(".value", "too_big")])
    );
    assert_eq!(
        read_issues(VldFloat::<Ratio>::from_sql(
            &Type::FLOAT8,
            &f64::NAN.to_be_bytes()
        )),
        pairs(&[("", "not_finite")])
    );
    assert_eq!(
        read_issues(VldText::<Code>::from_sql(&Type::TEXT, b"abcd")),
        pairs(&[(".value", "invalid_length")])
    );
    assert_eq!(
        read_issues(VldBool::<Accepted>::from_sql(&Type::BOOL, &[0])),
        pairs(&[(".value", "invalid_literal")])
    );
}
