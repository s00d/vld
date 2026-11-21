//! Story of a database integration crate expanding the hidden `db` macros
//! (as `vld-sqlx`, `vld-diesel`, … do) and its users storing columns.

use serde_json::json;
use vld::schema::VldParse;

vld_integration_common::__vld_db_error!(
    /// The crate's error type.
    DbError
);
vld_integration_common::__vld_db_validated!(DbError);
vld_integration_common::__vld_db_wrappers!(
    DbError;
    text: []
    int: []
    float: []
    bool: []
);

vld::schema! {
    #[derive(Debug)]
    pub struct Email {
        pub value: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Age {
        pub value: i64 => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Score {
        pub value: f64 => vld::number().min(0.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct MustBeTrue {
        pub value: bool => vld::literal(true),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct NewUser {
        pub name: String => vld::string().min(2),
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
struct UserRow {
    name: String,
}

struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

fn issue_codes(e: &DbError) -> Vec<(String, String)> {
    match e {
        DbError::Validation(v) => v
            .issues
            .iter()
            .map(|i| (vld::error::format_path(&i.path), i.code.key().to_string()))
            .collect(),
        DbError::Serialization(m) => panic!("serialization error: {m}"),
    }
}

fn pair(p: &str, c: &str) -> Vec<(String, String)> {
    vec![(p.into(), c.into())]
}

/// Given column wrappers and `Validated` generated in a database crate,
/// when a user builds, loads, compares and serializes values,
/// then only values that pass the schema get in (also through
/// deserialization), non-finite floats are `not_finite`, and the wrappers
/// behave like their inner values.
#[test]
fn columns_only_hold_values_that_passed_the_schema() {
    // Text.
    let email = VldText::<Email>::new("a@b.co").unwrap();
    assert_eq!(
        (email.as_str(), &*email, email.as_ref()),
        ("a@b.co", &"a@b.co".to_string(), "a@b.co")
    );
    assert_eq!(format!("{email:?} {email}"), r#"VldText("a@b.co") a@b.co"#);
    assert_eq!(serde_json::to_value(&email).unwrap(), json!("a@b.co"));
    let err = VldText::<Email>::new("bad").unwrap_err();
    assert_eq!(issue_codes(&err), pair("value", "invalid_string"));
    assert!(err.to_string().starts_with("Validation error: "));
    assert!(std::error::Error::source(&err).is_some());
    assert!(serde_json::from_value::<VldText<Email>>(json!("bad")).is_err());
    let loaded: VldText<Email> = serde_json::from_value(json!("z@b.co")).unwrap();
    assert!(loaded > email && loaded.clone() == loaded);
    let trusted = VldText::<Email>::new_unchecked("not checked");
    assert_eq!(trusted.into_inner(), "not checked");
    let set: std::collections::HashSet<_> = [email.clone(), email.clone()].into_iter().collect();
    assert_eq!(set.len(), 1);

    // Integers.
    let age = VldInt::<Age>::new(30).unwrap();
    assert_eq!(
        (age.get(), *age, format!("{age:?} {age}")),
        (30, 30, "VldInt(30) 30".into())
    );
    assert_eq!(
        issue_codes(&VldInt::<Age>::new(-1).unwrap_err()),
        pair("value", "too_small")
    );
    assert_eq!(
        issue_codes(&VldInt::<Age>::new(151).unwrap_err()),
        pair("value", "too_big")
    );
    assert!(serde_json::from_value::<VldInt<Age>>(json!(200)).is_err());
    assert!(VldInt::<Age>::new(1).unwrap() < age);
    let copy = age;
    assert_eq!(copy, age);

    // Floats: NaN / ±inf are `not_finite`, not "missing".
    let score = VldFloat::<Score>::new(1.5).unwrap();
    assert_eq!(
        (score.get(), format!("{score:?}")),
        (1.5, "VldFloat(1.5)".into())
    );
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let e = VldFloat::<Score>::new(x).unwrap_err();
        assert_eq!(issue_codes(&e), pair("", "not_finite"), "{x}");
    }
    assert_eq!(
        issue_codes(&VldFloat::<Score>::new(-0.5).unwrap_err()),
        pair("value", "too_small")
    );
    assert!(VldFloat::<Score>::new(0.5).unwrap() < score);

    // Booleans.
    let yes = VldBool::<MustBeTrue>::new(true).unwrap();
    assert!(yes.get());
    assert!(VldBool::<MustBeTrue>::new(false).is_err());
    assert!(VldBool::<MustBeTrue>::new_unchecked(false) < yes);

    // Whole rows.
    let row = Validated::<NewUser, _>::new(UserRow {
        name: "Alice".into(),
    })
    .unwrap();
    assert_eq!(row.inner().name, "Alice");
    assert_eq!(row.name, "Alice");
    assert_eq!(
        serde_json::to_value(&row).unwrap(),
        json!({"name": "Alice"})
    );
    assert_eq!(
        format!("{:?}", row.clone()),
        r#"Validated { inner: UserRow { name: "Alice" } }"#
    );
    assert_eq!(
        row.into_inner(),
        UserRow {
            name: "Alice".into()
        }
    );
    let err = Validated::<NewUser, _>::new(UserRow { name: "A".into() }).unwrap_err();
    assert_eq!(issue_codes(&err), pair("name", "too_small"));

    // A row that cannot be serialized.
    let Err(err) = Validated::<NewUser, _>::new(Unserializable) else {
        panic!("accepted an unserializable row")
    };
    assert!(matches!(err, DbError::Serialization(_)));
    assert!(err.to_string().starts_with("Serialization error: "));
    assert!(std::error::Error::source(&err).is_none());

    // `From<VldError>`.
    let raw = <NewUser as VldParse>::vld_parse_value(&json!({})).unwrap_err();
    assert_eq!(
        issue_codes(&DbError::from(raw)),
        pair("name", "missing_field")
    );

    // The shared check the wrappers use.
    assert!(vld_integration_common::db::check_value::<Age>(json!(3)).is_ok());
    let e = vld_integration_common::db::check_value::<Age>(json!(null)).unwrap_err();
    assert_eq!(e.issues[0].code.key(), "not_finite");
}
