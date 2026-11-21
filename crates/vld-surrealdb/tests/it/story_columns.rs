//! Stories: the validated scalar wrappers in documents (built, displayed,
//! serialized, deserialized with validation).

use crate::common::*;
use serde_json::json;
use vld_surrealdb::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AgeField {
        pub value: i64 => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct PriceField {
        pub value: f64 => vld::number().min(0.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct ActiveField {
        pub value: bool => vld::boolean(),
    }
}

/// A record with validated fields, as an SDK `select` deserializes it.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Account {
    email: VldText<EmailField>,
    age: VldInt<AgeField>,
    price: VldFloat<PriceField>,
    active: VldBool<ActiveField>,
}

/// Given a record type built from validated wrappers,
/// when a good record is built, serialized and deserialized,
/// then each field is a plain JSON scalar and reads back with `as_str` /
/// `get` / `Deref` / `Display`; equal values compare equal.
#[test]
fn record_of_wrappers_round_trips() {
    let acc = Account {
        email: VldText::new("test@test.com").unwrap(),
        age: VldInt::new(30).unwrap(),
        price: VldFloat::new(19.99).unwrap(),
        active: VldBool::new(false).unwrap(),
    };
    let doc = serde_json::to_value(&acc).unwrap();
    assert_eq!(
        doc,
        json!({"email": "test@test.com", "age": 30, "price": 19.99, "active": false})
    );
    let back: Account = serde_json::from_value(doc).unwrap();
    assert_eq!(back.email, acc.email);
    assert_eq!(back.email.to_string(), "test@test.com");
    assert_eq!(
        (
            back.email.as_str(),
            back.age.get(),
            *back.price,
            *back.active
        ),
        ("test@test.com", 30, 19.99, false)
    );
    let from_db: Account = serde_json::from_value(
        json!({"email": "a@b.com", "age": 42, "price": 42.5, "active": true}),
    )
    .unwrap();
    assert_eq!(
        (from_db.age.get(), from_db.price.get(), from_db.active.get()),
        (42, 42.5, true)
    );
}

/// Given invalid values,
/// when they are wrapped directly or deserialized inside a record,
/// then direct construction reports `.value` with the schema code and
/// deserialization fails with the validation error.
#[test]
fn invalid_values_are_rejected() {
    assert_eq!(
        issues(VldText::<EmailField>::new("not-an-email")),
        pairs(&[(".value", "invalid_string")])
    );
    assert_eq!(
        issues(VldInt::<AgeField>::new(-1)),
        pairs(&[(".value", "too_small")])
    );
    assert_eq!(
        issues(VldInt::<AgeField>::new(200)),
        pairs(&[(".value", "too_big")])
    );
    assert_eq!(
        issues(VldFloat::<PriceField>::new(-1.0)),
        pairs(&[(".value", "too_small")])
    );
    for bad in [
        json!({"email": "bad", "age": 1, "price": 1.0, "active": true}),
        json!({"email": "a@b.com", "age": -5, "price": 1.0, "active": true}),
    ] {
        let err = serde_json::from_value::<Account>(bad).unwrap_err();
        assert!(
            err.to_string().starts_with("Validation error: .value: "),
            "{err}"
        );
    }
}
