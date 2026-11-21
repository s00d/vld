//! Stories: the validated column wrappers (`VldText`, `VldInt`, `VldFloat`,
//! `VldBool`) built, bound into queries, decoded from columns and sent through
//! serde.

use crate::common::sqlx::{self, Row, SqlitePool};
use crate::common::*;
use std::collections::HashSet;
use vld_sqlx::prelude::*;

/// Given an email column wrapper,
/// when a valid and an invalid address are wrapped,
/// then the valid one exposes its text every way (`as_str`, `Deref`, `AsRef`,
/// `Display`, `Debug`, `into_inner`), compares / hashes / orders by value, and
/// the invalid one reports `.value invalid_string`; `new_unchecked` skips the
/// check.
#[test]
fn text_wrapper_holds_a_validated_string() {
    let email = VldText::<EmailField>::new("user@example.com").unwrap();
    assert_eq!(email.as_str(), "user@example.com");
    assert_eq!(&*email, "user@example.com");
    assert_eq!(AsRef::<str>::as_ref(&email), "user@example.com");
    assert_eq!(email.to_string(), "user@example.com");
    assert_eq!(format!("{email:?}"), r#"VldText("user@example.com")"#);

    let a = VldText::<EmailField>::new("a@example.com").unwrap();
    assert_eq!(a.clone(), a);
    assert!(a < email);
    let set: HashSet<_> = [a.clone(), a.clone(), email.clone()].into_iter().collect();
    assert_eq!(set.len(), 2);
    assert_eq!(email.into_inner(), "user@example.com");

    assert_eq!(
        issues(VldText::<EmailField>::new("not-an-email")),
        pairs(&[(".value", "invalid_string")])
    );
    assert_eq!(
        VldText::<EmailField>::new_unchecked("anything").as_str(),
        "anything"
    );
}

/// Given int, float and bool wrappers with bounds,
/// when values inside and outside the bounds are wrapped,
/// then accepted values are read with `get` / `Deref` / `Display` / `Debug`
/// and order by value; rejected ones report `.value` with `too_small` /
/// `too_big`; `new_unchecked` keeps an out-of-range value.
#[test]
fn scalar_wrappers_check_bounds() {
    let age = VldInt::<AgeField>::new(25).unwrap();
    assert_eq!(
        (*age, age.get(), age.to_string()),
        (25, 25, "25".to_string())
    );
    assert_eq!(format!("{age:?}"), "VldInt(25)");
    assert!(VldInt::<AgeField>::new(150).unwrap() > age);
    assert_eq!(
        issues(VldInt::<AgeField>::new(-1)),
        pairs(&[(".value", "too_small")])
    );
    assert_eq!(
        issues(VldInt::<AgeField>::new(200)),
        pairs(&[(".value", "too_big")])
    );
    assert_eq!(*VldInt::<AgeField>::new_unchecked(999), 999);

    let price = VldFloat::<PriceField>::new(9.99).unwrap();
    assert_eq!((price.get(), *price), (9.99, 9.99));
    assert_eq!(format!("{price:?}"), "VldFloat(9.99)");
    assert!(VldFloat::<PriceField>::new(0.0).unwrap() < price);
    assert_eq!(
        issues(VldFloat::<PriceField>::new(-0.01)),
        pairs(&[(".value", "too_small")])
    );

    let active = VldBool::<ActiveField>::new(true).unwrap();
    assert!(*active && active.get());
    assert!(VldBool::<ActiveField>::new(false).unwrap() < active);
}

/// Given a float column whose schema accepts a missing value,
/// when NaN or infinity is wrapped (both serialize as JSON `null`),
/// then they are rejected as `not_finite` at the root instead of passing as
/// "missing"; a finite value passes.
#[test]
fn non_finite_floats_are_rejected() {
    vld::schema! {
        #[derive(Debug)]
        pub struct OptFloat {
            pub value: Option<f64> => vld::number().optional(),
        }
    }
    assert_eq!(
        issues(VldFloat::<OptFloat>::new(f64::NAN)),
        pairs(&[("", "not_finite")])
    );
    assert_eq!(
        issues(VldFloat::<OptFloat>::new(f64::INFINITY)),
        pairs(&[("", "not_finite")])
    );
    assert_eq!(*VldFloat::<OptFloat>::new(2.5).unwrap(), 2.5);
}

/// Given wrappers in a JSON API body,
/// when they are serialized and deserialized,
/// then they are plain JSON scalars, deserializing validates, and an invalid
/// value fails with the validation error naming `.value`.
#[test]
fn wrappers_go_through_serde_as_plain_values() {
    let email = VldText::<EmailField>::new("a@b.com").unwrap();
    assert_eq!(
        serde_json::to_value(&email).unwrap(),
        serde_json::json!("a@b.com")
    );
    assert_eq!(
        serde_json::to_value(VldInt::<AgeField>::new(42).unwrap()).unwrap(),
        42
    );
    assert_eq!(
        serde_json::to_value(VldFloat::<PriceField>::new(3.25).unwrap()).unwrap(),
        3.25
    );
    assert_eq!(
        serde_json::to_value(VldBool::<ActiveField>::new(false).unwrap()).unwrap(),
        false
    );

    let back: VldText<EmailField> = serde_json::from_value(serde_json::json!("a@b.com")).unwrap();
    assert_eq!(back, email);
    let back: VldInt<AgeField> = serde_json::from_value(serde_json::json!(30)).unwrap();
    assert_eq!(back.get(), 30);

    for err in [
        serde_json::from_value::<VldText<EmailField>>(serde_json::json!("bad")).unwrap_err(),
        serde_json::from_value::<VldInt<AgeField>>(serde_json::json!(-5)).unwrap_err(),
    ] {
        assert!(
            err.to_string().starts_with("Validation error: .value: "),
            "{err}"
        );
    }
}

/// Given a table with TEXT, BIGINT, INT4, SMALLINT, DOUBLE, REAL and BOOLEAN
/// columns,
/// when validated wrappers are bound into an INSERT and decoded back,
/// then every column round-trips its value (INT4 / SMALLINT widen to `i64`,
/// `0.1` stays `0.1` in a REAL column), and the wrappers work in a WHERE.
#[tokio::test]
async fn wrappers_bind_and_decode_through_every_column_type() {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE t (email TEXT NOT NULL, big BIGINT NOT NULL, i4 INT4 NOT NULL,
         i2 SMALLINT NOT NULL, d DOUBLE NOT NULL, r REAL NOT NULL, b BOOLEAN NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let email = VldText::<EmailField>::new("alice@test.com").unwrap();
    sqlx::query("INSERT INTO t VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&email)
        .bind(VldInt::<AgeField>::new(30).unwrap())
        .bind(VldInt::<AgeField>::new(31).unwrap())
        .bind(VldInt::<AgeField>::new(7).unwrap())
        .bind(VldFloat::<PriceField>::new(9.5).unwrap())
        .bind(VldFloat::<PriceField>::new(0.1).unwrap())
        .bind(VldBool::<ActiveField>::new(true).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT * FROM t WHERE email = ?")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let got: VldText<EmailField> = row.try_get("email").unwrap();
    assert_eq!(got, email);
    let ints: Vec<i64> = ["big", "i4", "i2"]
        .iter()
        .map(|c| row.try_get::<VldInt<AgeField>, _>(*c).unwrap().get())
        .collect();
    assert_eq!(ints, [30, 31, 7]);
    let d: VldFloat<PriceField> = row.try_get("d").unwrap();
    let r: VldFloat<PriceField> = row.try_get("r").unwrap();
    assert_eq!((d.get(), r.get()), (9.5, 0.1));
    let b: VldBool<ActiveField> = row.try_get("b").unwrap();
    assert!(b.get());
}

/// Given a database holding a value the schema rejects and a TEXT value in a
/// numeric position,
/// when they are decoded into wrappers,
/// then the invalid email decodes unchecked (data from the database is
/// trusted) and `validate_row` catches it, while a TEXT value is a type
/// mismatch for the int and float wrappers.
#[tokio::test]
async fn decoding_trusts_the_database_but_checks_column_types() {
    let pool = users_db().await;
    sqlx::query("INSERT INTO users (name, email, age) VALUES ('X', 'bad', 999)")
        .execute(&pool)
        .await
        .unwrap();
    let row = sqlx::query("SELECT name, email, age FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let email: VldText<EmailField> = row.try_get("email").unwrap();
    let age: VldInt<AgeField> = row.try_get("age").unwrap();
    assert_eq!((email.as_str(), age.get()), ("bad", 999));
    assert_eq!(
        issues(validate_row::<UserSchema, _>(&user("X", &email, *age))),
        pairs(&[(".email", "invalid_string"), (".age", "too_big")])
    );

    assert!(row.try_get::<VldInt<AgeField>, _>("name").is_err());
    assert!(row.try_get::<VldFloat<PriceField>, _>("name").is_err());
    assert!(row.try_get::<VldBool<ActiveField>, _>("name").is_err());
    assert!(row.try_get::<VldText<EmailField>, _>("age").is_err());
}

/// Given Postgres, where INT2 / INT4 / INT8 and FLOAT4 / FLOAT8 are distinct
/// types,
/// when the wrappers' column compatibility is asked,
/// then `VldInt` accepts every integer width, `VldFloat` both float widths,
/// and neither accepts TEXT; both declare the wide type.
#[cfg(feature = "postgres")]
#[test]
fn postgres_columns_of_every_width_are_compatible() {
    use sqlx::Type;
    use sqlx::postgres::Postgres;
    struct Any;

    for ty in [
        <i16 as Type<Postgres>>::type_info(),
        <i32 as Type<Postgres>>::type_info(),
        <i64 as Type<Postgres>>::type_info(),
    ] {
        assert!(<VldInt<Any> as Type<Postgres>>::compatible(&ty), "{ty:?}");
    }
    for ty in [
        <f32 as Type<Postgres>>::type_info(),
        <f64 as Type<Postgres>>::type_info(),
    ] {
        assert!(<VldFloat<Any> as Type<Postgres>>::compatible(&ty), "{ty:?}");
    }
    let text = <String as Type<Postgres>>::type_info();
    assert!(!<VldInt<Any> as Type<Postgres>>::compatible(&text));
    assert!(!<VldFloat<Any> as Type<Postgres>>::compatible(&text));
    assert_eq!(
        <VldInt<Any> as Type<Postgres>>::type_info(),
        <i64 as Type<Postgres>>::type_info()
    );
    assert_eq!(
        <VldFloat<Any> as Type<Postgres>>::type_info(),
        <f64 as Type<Postgres>>::type_info()
    );
}
