//! Stories: ActiveModels, models, DTOs and raw JSON checked against a schema
//! (`validate_active`, `active_model_to_json`, `validate_model`,
//! `validate_json`, `Validated`).

use crate::common::*;
use serde_json::json;
use vld_sea::prelude::*;
use vld_sea::sea_orm::{ActiveValue, Set, Unchanged};

/// Given user ActiveModels with `Set`, `Unchanged` and `NotSet` fields,
/// when they are turned into JSON and validated with `validate_active`,
/// then `Set` and `Unchanged` values appear under their column names, `NotSet`
/// ones are omitted (so a required field reads as missing), and the parsed
/// schema value carries the fields; bad values report their column.
#[test]
fn active_models_are_validated_from_their_set_fields() {
    let am = user::ActiveModel {
        id: Unchanged(42),
        name: Set("Charlie".to_owned()),
        email: Set("charlie@test.com".to_owned()),
    };
    assert_eq!(
        active_model_to_json(&am),
        json!({"id": 42, "name": "Charlie", "email": "charlie@test.com"})
    );
    let parsed = validate_active::<UserInput, _>(&am).unwrap();
    assert_eq!(
        (parsed.name.as_str(), parsed.email.as_str()),
        ("Charlie", "charlie@test.com")
    );

    let partial = user::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set("Dave".to_owned()),
        email: ActiveValue::NotSet,
    };
    assert_eq!(active_model_to_json(&partial), json!({"name": "Dave"}));
    assert_eq!(
        issues(validate_active::<UserInput, _>(&partial)),
        pairs(&[(".email", "missing_field")])
    );
    validate_active::<UserPatch, _>(&partial).unwrap();

    for (name, email, expected) in [
        ("Bob", "not-an-email", vec![(".email", "invalid_string")]),
        ("", "bob@example.com", vec![(".name", "too_small")]),
        (
            "",
            "bad",
            vec![(".name", "too_small"), (".email", "invalid_string")],
        ),
    ] {
        let am = user::ActiveModel {
            id: Set(1),
            name: Set(name.to_owned()),
            email: Set(email.to_owned()),
        };
        assert_eq!(
            issues(validate_active::<UserInput, _>(&am)),
            pairs(&expected)
        );
    }
}

/// Given a loaded `Model`, an input DTO and raw JSON,
/// when they are validated with `validate_model`, `validate_json` and
/// `Validated::new`,
/// then valid ones give the parsed schema value / the wrapped original, and
/// invalid ones name every bad field.
#[test]
fn models_dtos_and_json_are_validated() {
    #[derive(Debug, Clone, serde::Serialize)]
    struct Input {
        name: String,
        email: String,
    }

    let model = user::Model {
        id: 1,
        name: "Eve".into(),
        email: "eve@example.com".into(),
    };
    let parsed = validate_model::<UserInput, _>(&model).unwrap();
    assert_eq!(
        (parsed.name.as_str(), parsed.email.as_str()),
        ("Eve", "eve@example.com")
    );
    let bad = Input {
        name: "".into(),
        email: "bad".into(),
    };
    let both = pairs(&[(".name", "too_small"), (".email", "invalid_string")]);
    assert_eq!(issues(validate_model::<UserInput, _>(&bad)), both);

    let parsed =
        validate_json::<UserInput>(&json!({"name": "Frank", "email": "frank@example.com"}))
            .unwrap();
    assert_eq!(parsed.name, "Frank");
    assert_eq!(
        issues(validate_json::<UserInput>(
            &json!({"name": "", "email": "not-email"})
        )),
        both
    );

    let good = Input {
        name: "Grace".into(),
        email: "grace@example.com".into(),
    };
    let v = Validated::<UserInput, _>::new(good).unwrap();
    assert_eq!(
        (v.inner().name.as_str(), v.email.as_str()),
        ("Grace", "grace@example.com")
    );
    assert_eq!(serde_json::to_value(&v).unwrap()["name"], "Grace");
    assert_eq!(v.clone().into_inner().name, "Grace");
    assert_eq!(issues(Validated::<UserInput, _>::new(bad)), both);
}

/// A value whose `Serialize` impl fails.
struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

/// Given a value that cannot be serialized and an invalid ActiveModel,
/// when their errors are handed to SeaORM (`?` in `before_save`),
/// then the first is `Serialization`, and both become `DbErr::Custom` whose
/// text starts with the error kind and names the cause / the bad column.
#[test]
fn errors_become_db_errors() {
    use vld_sea::sea_orm::DbErr;
    let err = validate_model::<UserInput, _>(&Unserializable).unwrap_err();
    assert!(
        matches!(&err, VldSeaError::Serialization(m) if m == "nope"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Serialization error: nope");
    match DbErr::from(err) {
        DbErr::Custom(msg) => assert_eq!(msg, "Serialization error: nope"),
        other => panic!("expected DbErr::Custom, got {other:?}"),
    }

    let am = user::ActiveModel {
        id: Set(1),
        name: Set("Heidi".into()),
        email: Set("nope".into()),
    };
    match before_save::<UserInput, _>(&am) {
        Err(DbErr::Custom(msg)) => {
            assert!(msg.starts_with("Validation error: "), "{msg}");
            assert!(msg.contains(".email: "), "{msg}");
        }
        other => panic!("expected DbErr::Custom, got {other:?}"),
    }
    let ok = user::ActiveModel {
        email: Set("heidi@example.com".into()),
        ..am
    };
    before_save::<UserInput, _>(&ok).unwrap();
}
