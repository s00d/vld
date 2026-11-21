//! Stories: the validated column wrappers built, sent through serde, and used
//! as Diesel column types (`Insertable`, `Queryable`, `QueryableByName`,
//! `.filter(col.eq(wrapper))`).

use crate::common::*;
use diesel::prelude::*;
use std::collections::HashSet;
use vld_diesel::prelude::*;

/// Given an email column wrapper,
/// when a valid and an invalid address are wrapped,
/// then the valid one exposes its text every way and compares / hashes /
/// orders by value, the invalid one reports `.value invalid_string`;
/// `new_unchecked` skips the check.
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
    let set: HashSet<_> = [a.clone(), a, email.clone()].into_iter().collect();
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
/// then accepted values read back with `get` / `Deref` / `Display`, rejected
/// ones report `.value` with `too_small` / `too_big`, `new_unchecked` keeps an
/// out-of-range value, and NaN / infinity are `not_finite` even for a schema
/// that accepts a missing value.
#[test]
fn scalar_wrappers_check_bounds() {
    let age = VldInt::<AgeField>::new(25).unwrap();
    assert_eq!(
        (*age, age.get(), age.to_string()),
        (25, 25, "25".to_string())
    );
    assert_eq!(
        issues(VldInt::<AgeField>::new(-1)),
        pairs(&[(".value", "too_small")])
    );
    assert_eq!(
        issues(VldInt::<AgeField>::new(200)),
        pairs(&[(".value", "too_big")])
    );
    assert_eq!(*VldInt::<AgeField>::new_unchecked(999), 999);

    assert_eq!(VldFloat::<PriceField>::new(9.5).unwrap().get(), 9.5);
    assert_eq!(
        issues(VldFloat::<PriceField>::new(-0.5)),
        pairs(&[(".value", "too_small")])
    );
    assert!(VldBool::<ActiveField>::new(true).unwrap().get());

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
    assert_eq!(VldFloat::<OptFloat>::new(1.5).unwrap().get(), 1.5);
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

diesel::table! {
    products (id) {
        id -> Integer,
        email -> Text,
        qty -> BigInt,
        price -> Double,
        active -> Bool,
    }
}

#[derive(Insertable)]
#[diesel(table_name = products)]
struct NewProduct {
    email: VldText<EmailField>,
    qty: VldInt<AgeField>,
    price: VldFloat<PriceField>,
    active: VldBool<ActiveField>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = products)]
struct Product {
    #[allow(dead_code)]
    id: i32,
    email: VldText<EmailField>,
    qty: VldInt<AgeField>,
    price: VldFloat<PriceField>,
    active: VldBool<ActiveField>,
}

#[derive(Debug, QueryableByName)]
#[diesel(table_name = products)]
struct RawProduct {
    #[diesel(sql_type = diesel::sql_types::Text)]
    email: VldText<EmailField>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    qty: VldInt<AgeField>,
}

/// Given a products table of TEXT, BIGINT, DOUBLE and BOOLEAN columns,
/// when rows of validated wrappers are inserted with `Insertable`, filtered by
/// wrapper values, loaded with `Queryable` and with `sql_query` +
/// `QueryableByName`,
/// then every column round-trips, the filter matches only the right row, and a
/// value the schema rejects (written with raw SQL) still loads: decoding trusts
/// the database.
#[test]
fn wrappers_as_diesel_column_types() {
    let mut conn = db(&[
        "CREATE TABLE products (id INTEGER PRIMARY KEY AUTOINCREMENT,
        email TEXT NOT NULL, qty BIGINT NOT NULL, price DOUBLE NOT NULL, active BOOLEAN NOT NULL)",
    ]);
    let rows = [
        ("a@example.com", 3, 9.5, true),
        ("b@example.com", 4, 0.25, false),
    ]
    .map(|(e, q, p, a)| NewProduct {
        email: VldText::new(e).unwrap(),
        qty: VldInt::new(q).unwrap(),
        price: VldFloat::new(p).unwrap(),
        active: VldBool::new(a).unwrap(),
    });
    diesel::insert_into(products::table)
        .values(&rows[..])
        .execute(&mut conn)
        .unwrap();

    let email = VldText::<EmailField>::new("b@example.com").unwrap();
    let found: Vec<Product> = products::table
        .filter(products::email.eq(&email))
        .filter(products::qty.eq(VldInt::<AgeField>::new(4).unwrap()))
        .filter(products::price.eq(VldFloat::<PriceField>::new(0.25).unwrap()))
        .filter(products::active.eq(VldBool::<ActiveField>::new(false).unwrap()))
        .select(Product::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(found.len(), 1);
    let p = &found[0];
    assert_eq!(
        (p.email.as_str(), p.qty.get(), p.price.get(), p.active.get()),
        ("b@example.com", 4, 0.25, false)
    );

    diesel::sql_query(
        "INSERT INTO products (email, qty, price, active) VALUES ('bad', 999, -1, 1)",
    )
    .execute(&mut conn)
    .unwrap();
    let raw: Vec<RawProduct> = diesel::sql_query("SELECT email, qty FROM products ORDER BY id")
        .load(&mut conn)
        .unwrap();
    let got: Vec<(&str, i64)> = raw
        .iter()
        .map(|r| (r.email.as_str(), r.qty.get()))
        .collect();
    assert_eq!(
        got,
        [("a@example.com", 3), ("b@example.com", 4), ("bad", 999)]
    );
}
