//! Stories: documents checked before `create` / `insert` and after `select`,
//! and the error body sent back to an API client.

use super::common::*;
use serde_json::json;
use vld_data::surrealdb::prelude::*;

/// Given a person document for `db.create("person").content(..)`,
/// when it is checked with `validate_content` and wrapped in `Validated`,
/// then a valid one (bounds inclusive) passes and the wrapper serializes as
/// the document itself (what the SDK sends); `Deref`, `inner`, `into_inner`,
/// `Clone`, `Debug` expose it; invalid ones name every bad field.
#[test]
fn person_is_validated_before_create() {
    for ok in [
        person("Alice", "alice@example.com", 30),
        person("A", "a@b.co", 150),
    ] {
        validate_content::<PersonSchema, _>(&ok).unwrap();
    }
    let v = Validated::<PersonSchema, _>::new(person("Alice", "alice@example.com", 30)).unwrap();
    assert_eq!((v.name.as_str(), v.inner().age), ("Alice", 30));
    let content = serde_json::to_value(&v).unwrap();
    assert_eq!(
        content,
        json!({"name": "Alice", "email": "alice@example.com", "age": 30})
    );
    assert!(format!("{v:?}").starts_with("Validated { inner: Person"));
    assert_eq!(
        v.clone().into_inner(),
        person("Alice", "alice@example.com", 30)
    );

    let cases = [
        (
            person("", "alice@example.com", 30),
            vec![(".name", "too_small")],
        ),
        (
            person("Alice", "not-an-email", 30),
            vec![(".email", "invalid_string")],
        ),
        (
            person("Alice", "alice@example.com", -1),
            vec![(".age", "too_small")],
        ),
        (
            person("", "bad", -1),
            vec![
                (".name", "too_small"),
                (".email", "invalid_string"),
                (".age", "too_small"),
            ],
        ),
    ];
    for (bad, expected) in cases {
        assert_eq!(
            issues(validate_content::<PersonSchema, _>(&bad)),
            pairs(&expected)
        );
        assert_eq!(
            issues(Validated::<PersonSchema, _>::new(bad)),
            pairs(&expected)
        );
    }
    let err = validate_content::<PersonSchema, _>(&Unserializable).unwrap_err();
    assert!(
        matches!(&err, VldSurrealError::Serialization(m) if m == "nope"),
        "{err:?}"
    );
}

/// Given the JSON a `SELECT` returns (records and raw query results),
/// when it is checked with `validate_json` and `validate_records`,
/// then good results pass, missing fields are `missing_field`, out-of-range
/// ones carry their codes, and a batch reports the index of its first bad
/// record with that record's issues; an empty batch passes.
#[test]
fn select_results_are_rechecked() {
    validate_json::<PersonSchema>(&json!({"name": "Bob", "email": "bob@example.com", "age": 25}))
        .unwrap();
    assert_eq!(
        issues(validate_json::<PersonSchema>(
            &json!({"name": "", "email": "bad", "age": 200})
        )),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_big")
        ])
    );
    assert_eq!(
        issues(validate_json::<PersonSchema>(&json!({"name": "Bob"}))),
        pairs(&[(".email", "missing_field"), (".age", "missing_field")])
    );

    let rows = [
        person("Alice", "a@x.io", 1),
        person("", "b@x.io", 2),
        person("Carol", "nope", 3),
    ];
    validate_records::<PersonSchema, _>(&rows[..1]).unwrap();
    validate_records::<PersonSchema, Person>(&[]).unwrap();
    let (index, err) = validate_records::<PersonSchema, _>(&rows).unwrap_err();
    assert_eq!(index, 1);
    assert_eq!(issues_of(&err), pairs(&[(".name", "too_small")]));
    let (index, err) = validate_records::<PersonSchema, _>(&rows[2..]).unwrap_err();
    assert_eq!(index, 0);
    assert_eq!(issues_of(&err), pairs(&[(".email", "invalid_string")]));
}

/// Given a nested document rejected by the schema and a document that cannot
/// be serialized,
/// when each error is turned into a `VldSurrealResponse` for the API client,
/// then the validation body is `Validation failed` with one entry per issue
/// whose path has no leading dot (`tags[0]`, `addr.city`) and whose message
/// is the issue's; the serialization body is `Serialization error` with one
/// entry for the root; both survive a JSON round trip.
#[test]
fn errors_become_an_api_response() {
    let err =
        validate_json::<OuterDoc>(&json!({"tags": ["ok", ""], "addr": {"city": "X"}})).unwrap_err();
    let VldSurrealError::Validation(ve) = &err else {
        panic!("{err:?}")
    };
    let resp = VldSurrealResponse::from_error(&err);
    assert_eq!(resp.error, "Validation failed");
    let fields: Vec<(&str, &str)> = resp
        .fields
        .iter()
        .map(|f| (f.field.as_str(), f.message.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            ("tags[1]", ve.issues[0].message.as_str()),
            ("addr.city", ve.issues[1].message.as_str())
        ]
    );
    let body = resp.to_json();
    assert_eq!(body["error"], "Validation failed");
    assert_eq!(body["fields"][1]["field"], "addr.city");
    let back: VldSurrealResponse = serde_json::from_value(body).unwrap();
    assert_eq!(back.fields, resp.fields);
    assert_eq!(VldSurrealResponse::from_vld_error(ve).fields, resp.fields);

    let err = validate_content::<PersonSchema, _>(&Unserializable).unwrap_err();
    let resp = VldSurrealResponse::from_error(&err);
    assert_eq!(resp.error, "Serialization error");
    assert_eq!(
        resp.fields,
        [FieldError {
            field: String::new(),
            message: "nope".into()
        }]
    );
    assert_eq!(
        resp.to_json(),
        json!({"error": "Serialization error", "fields": [{"field": "", "message": "nope"}]})
    );
}

/// Given the two error kinds,
/// when they are displayed and asked for their source,
/// then each text names its kind and carries the cause, and only a
/// validation error has a source; a `VldError` converts into `Validation`.
#[test]
fn error_kinds() {
    use std::error::Error as _;
    let err = VldSurrealError::Serialization("test error".into());
    assert_eq!(err.to_string(), "Serialization error: test error");
    assert!(err.source().is_none());
    let err: VldSurrealError =
        vld::error::VldError::single(vld::error::IssueCode::MissingField, "required").into();
    assert_eq!(issues_of(&err), pairs(&[("", "missing_field")]));
    assert!(err.to_string().starts_with("Validation error: "));
    assert!(err.source().is_some());
}
