//! Stories: a MongoDB document checked before `insert_one` and after
//! `find_one`, as the schema sees it (relaxed Extended JSON).

use super::bson::{self, DateTime, Document, doc, oid::ObjectId};
use super::vld_bson::prelude::*;
use crate::issues::{of, pairs};

vld::schema! {
    #[derive(Debug)]
    pub struct OidSchema {
        pub oid: String as "$oid" => vld::string().len(24),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct DateSchema {
        pub date: String as "$date" => vld::string().iso_datetime(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub id: OidSchema as "_id" => vld::nested!(OidSchema),
        pub name: String => vld::string().min(1),
        pub visits: i64 => vld::number().int().min(0),
        pub joined: DateSchema => vld::nested!(DateSchema),
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    #[serde(rename = "_id")]
    id: ObjectId,
    name: String,
    visits: i64,
    joined: DateTime,
}

const OID: &str = "65a1b2c3d4e5f60718293a4b";

fn user(name: &str, visits: i64) -> User {
    User {
        id: ObjectId::parse_str(OID).unwrap(),
        name: name.into(),
        visits,
        joined: DateTime::from_millis(1_700_000_000_000),
    }
}

#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldBsonError>) -> Vec<(String, String)> {
    match r {
        Err(VldBsonError::Validation(e)) => of(&e),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Given a user with an `ObjectId`, an `Int64` and a `DateTime`,
/// when it is turned into a document with `to_document` and read back with
/// `from_document`,
/// then the document keeps the BSON types (what `insert_one` stores), the
/// schema sees `{"$oid": ..}` / `{"$date": "<ISO>"}` / a plain number, and the
/// value comes back unchanged.
#[test]
fn user_round_trips_through_a_validated_document() {
    let doc = to_document::<UserSchema, _>(&user("Ada", 3)).unwrap();
    assert_eq!(doc.get_object_id("_id").unwrap().to_hex(), OID);
    assert!(matches!(doc.get("joined"), Some(bson::Bson::DateTime(_))));

    let parsed = validate_document::<UserSchema>(&doc).unwrap();
    assert_eq!(parsed.id.oid, OID);
    assert_eq!(parsed.joined.date, "2023-11-14T22:13:20Z");
    assert_eq!(parsed.visits, 3);

    let back: User = from_document::<UserSchema, _>(doc).unwrap();
    assert_eq!(back, user("Ada", 3));
}

/// Given invalid values and documents,
/// when they are checked on the way in and out,
/// then `to_document` and `from_document` report every bad field (nested
/// `$`-keys included) and nothing is produced; a document written by another
/// client with a raw string `_id` is rejected as `invalid_type`.
#[test]
fn invalid_documents_are_rejected_both_ways() {
    assert_eq!(
        issues(to_document::<UserSchema, _>(&user("", -1))),
        pairs(&[(".name", "too_small"), (".visits", "too_small")])
    );

    let foreign = doc! {
        "_id": "not-an-oid",
        "name": "Bob",
        "visits": 1_i32,
        "joined": DateTime::from_millis(0),
    };
    assert_eq!(
        issues(from_document::<UserSchema, User>(foreign.clone())),
        pairs(&[("._id", "invalid_type")])
    );
    assert_eq!(
        issues(validate_document::<UserSchema>(&foreign)),
        pairs(&[("._id", "invalid_type")])
    );

    // Before 1970 a date is `{"$date": {"$numberLong": ..}}`, not a string.
    let old = doc! {
        "_id": ObjectId::parse_str(OID).unwrap(),
        "name": "Old",
        "visits": 0_i64,
        "joined": DateTime::from_millis(-1),
    };
    assert_eq!(
        issues(validate_document::<UserSchema>(&old)),
        pairs(&[(".joined.$date", "invalid_type")])
    );
}

/// Given a batch of documents from a cursor,
/// when it is checked with `validate_documents`,
/// then the index and issues of the first bad document are reported and an
/// empty batch passes.
#[test]
fn a_batch_reports_its_first_bad_document() {
    let good = to_document::<UserSchema, _>(&user("Ada", 1)).unwrap();
    let mut bad = good.clone();
    bad.insert("visits", -5_i64);
    let mut worse = good.clone();
    worse.insert("name", "");

    validate_documents::<UserSchema>(&[]).unwrap();
    validate_documents::<UserSchema>(&[good.clone(), good.clone()]).unwrap();
    let (i, e) = validate_documents::<UserSchema>(&[good, bad, worse]).unwrap_err();
    assert_eq!(i, 1);
    assert_eq!(issues::<()>(Err(e)), pairs(&[(".visits", "too_small")]));
}

/// Given values bson cannot store as a document or a document that passes
/// the schema but not the target type,
/// when they are converted,
/// then the error is `Serialization` / `Deserialization`, not `Validation`.
#[test]
fn conversion_failures_are_not_validation_errors() {
    let err = to_document::<UserSchema, _>(&5_i64).unwrap_err();
    assert!(matches!(err, VldBsonError::Serialization(_)), "{err:?}");

    #[derive(Debug, serde::Deserialize)]
    struct Wrong {
        #[allow(dead_code)]
        name: i64,
    }
    let doc: Document = to_document::<UserSchema, _>(&user("Ada", 1)).unwrap();
    let err = from_document::<UserSchema, Wrong>(doc).unwrap_err();
    assert!(matches!(err, VldBsonError::Deserialization(_)), "{err:?}");
    assert!(err.to_string().starts_with("Deserialization error: "));
    assert!(std::error::Error::source(&err).is_none());
}
