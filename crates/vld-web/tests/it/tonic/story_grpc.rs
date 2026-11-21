//! Stories of a gRPC service validating its requests with `vld-tonic`: the
//! message rules, a mirror schema, the metadata interceptor and the
//! `Status` body the client reads.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tonic::metadata::MetadataValue;
use tonic::{Code, Request, Status};
use vld::error::{IssueCode, PathSegment, VldError};
use vld_web::tonic::prelude::*;

/// `(path, code)` of every issue in a status body (checking its shape).
#[track_caller]
fn details(status: &Status, code: Code) -> Vec<(String, String)> {
    assert_eq!(status.code(), code, "{status:?}");
    let body: Value = serde_json::from_str(status.message()).unwrap();
    assert_eq!(body["error"], "VALIDATION_ERROR");
    assert_eq!(body["message"], "Request validation failed");
    body["details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            assert!(d["message"].is_string(), "{d}");
            (
                d["path"].as_str().unwrap().to_string(),
                d["code"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
    age: i32,
}

vld_web::tonic::impl_validate!(CreateUserRequest {
    name  => vld::string().min(2).max(50),
    email => vld::string().email(),
    age   => vld::number().int().min(0).max(150),
});

fn user(name: &str, email: &str, age: i32) -> CreateUserRequest {
    CreateUserRequest {
        name: name.into(),
        email: email.into(),
        age,
    }
}

/// Given a `CreateUser` RPC whose message type has rules attached with
/// `impl_validate!`,
/// when clients send valid, edge and invalid messages,
/// then `validate` hands the handler the message, and an invalid one is an
/// `INVALID_ARGUMENT` status whose JSON body lists every `(path, code)`;
/// the inherent `validate` / `is_valid` agree.
#[test]
fn a_create_user_rpc_validates_its_message() {
    let msg =
        vld_web::tonic::validate(Request::new(user("Alice", "alice@example.com", 30))).unwrap();
    assert_eq!(msg, user("Alice", "alice@example.com", 30));
    validate_ref(&user("Bo", "b@t.co", 150)).unwrap();
    validate_ref(&user("Bo", "b@t.co", 0)).unwrap();

    let status = vld_web::tonic::validate(Request::new(user("A", "bad", -5))).unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_small")
        ])
    );
    let status = validate_ref(&user("", "nope", 151)).unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_big")
        ])
    );

    let good = user("Charlie", "c@d.com", 40);
    assert!(good.validate().is_ok() && good.is_valid() && good.vld_validate().is_ok());
    let bad = user("", "bad", 200);
    assert!(bad.validate().is_err() && !bad.is_valid() && bad.vld_validate().is_err());
}

vld::schema! {
    #[derive(Debug)]
    struct UserSchema {
        name: String  => vld::string().min(2),
        email: String => vld::string().email(),
    }
}

#[derive(Debug, PartialEq, Serialize)]
struct SimpleMsg {
    name: String,
    email: String,
}

#[derive(Debug)]
struct Unserializable;
impl Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

/// Given a prost message without rules and a `vld::schema!` mirroring its
/// fields,
/// when the handler validates requests with `validate_with`,
/// then valid messages come back unchanged, invalid ones are
/// `INVALID_ARGUMENT` with `(path, code)` details, and a message that cannot
/// be serialized is `INTERNAL`.
#[test]
fn a_message_is_checked_against_a_mirror_schema() {
    let msg = SimpleMsg {
        name: "Alice".into(),
        email: "a@b.com".into(),
    };
    let back = validate_with::<UserSchema, _>(Request::new(SimpleMsg {
        name: "Alice".into(),
        email: "a@b.com".into(),
    }))
    .unwrap();
    assert_eq!(back, msg);
    validate_with_ref::<UserSchema, _>(&msg).unwrap();

    let bad = SimpleMsg {
        name: "A".into(),
        email: "x".into(),
    };
    let status = validate_with::<UserSchema, _>(Request::new(bad)).unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let status = validate_with_ref::<UserSchema, _>(&SimpleMsg {
        name: "".into(),
        email: "a@b.co".into(),
    })
    .unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[(".name", "too_small")])
    );

    let status = validate_with_ref::<UserSchema, _>(&Unserializable).unwrap_err();
    assert_eq!(status.code(), Code::Internal);
    assert!(status.message().starts_with("Serialization error: "));
    assert_eq!(
        validate_with::<UserSchema, _>(Request::new(Unserializable))
            .unwrap_err()
            .code(),
        Code::Internal
    );
}

vld::schema! {
    #[derive(Debug, Clone)]
    struct AuthMeta {
        authorization: String => vld::string().min(1),
        x_request_id: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    struct Meta {
        x_account: String => vld::string(),
        x_user: String => vld::string(),
        x_page_size: i64 => vld::number().int().min(1).max(100),
        x_trace: Option<String> => vld::string().optional(),
    }
}

fn with_metadata(pairs: &[(&'static str, &'static str)]) -> Request<()> {
    let mut req = Request::new(());
    for (k, v) in pairs {
        req.metadata_mut().append(*k, v.parse().unwrap());
    }
    req
}

/// Given a service wrapped with `metadata_interceptor::<AuthMeta>` and a
/// handler reading more metadata with `validate_metadata`,
/// when calls carry valid, missing, numeric-looking, repeated, binary and
/// out-of-range metadata,
/// then the interceptor stores `Validated<AuthMeta>` (never a bare
/// `AuthMeta`) for `validated_metadata`, keys become snake_case, values are
/// coerced losslessly, `-bin` keys are skipped, and a bad value is
/// `INVALID_ARGUMENT` at its key.
#[test]
fn metadata_carries_auth_through_an_interceptor() {
    let req = metadata_interceptor::<AuthMeta>(with_metadata(&[
        ("authorization", "Bearer abc"),
        ("x-request-id", "abc-123"),
    ]))
    .unwrap();
    assert!(req.extensions().get::<AuthMeta>().is_none());
    let stored = req.extensions().get::<Validated<AuthMeta>>().unwrap();
    assert_eq!(
        (
            stored.authorization.as_str(),
            stored.x_request_id.as_deref()
        ),
        ("Bearer abc", Some("abc-123"))
    );
    let auth: AuthMeta = validated_metadata(&req).unwrap();
    assert_eq!(auth.authorization, "Bearer abc");
    assert!(validated_metadata::<AuthMeta, _>(&Request::new(())).is_none());

    let status = metadata_interceptor::<AuthMeta>(Request::new(())).unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[(".authorization", "missing_field")])
    );
    let status =
        validate_metadata::<AuthMeta, _>(&with_metadata(&[("authorization", "")])).unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[(".authorization", "too_small")])
    );

    let mut req = with_metadata(&[
        ("x-account", "01234"),
        ("x-user", "42"),
        ("x-page-size", "25"),
        ("x-trace", "a"),
        ("x-trace", "b"),
    ]);
    req.metadata_mut()
        .insert_bin("x-trace-bin", MetadataValue::from_bytes(b"\xFF"));
    req.metadata_mut()
        .insert_bin("x-user-bin", MetadataValue::from_bytes(b"evil"));
    let meta: Meta = validate_metadata(&req).unwrap();
    assert_eq!(
        (
            meta.x_account.as_str(),
            meta.x_user.as_str(),
            meta.x_page_size,
            meta.x_trace.as_deref()
        ),
        ("01234", "42", 25, Some("a, b"))
    );
    let status = validate_metadata::<Meta, _>(&with_metadata(&[
        ("x-account", "a"),
        ("x-user", "u"),
        ("x-page-size", "500"),
    ]))
    .unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[(".x_page_size", "too_big")])
    );
    let status = validate_metadata::<Meta, _>(&with_metadata(&[("x-page-size", "x")])).unwrap_err();
    assert_eq!(
        details(&status, Code::InvalidArgument),
        pairs(&[
            (".x_account", "missing_field"),
            (".x_user", "missing_field"),
            (".x_page_size", "invalid_type")
        ])
    );
}

/// Given errors built outside the helpers (nested paths, custom codes),
/// when they become statuses with `vld_status` / `vld_status_with_code`,
/// then the body is the documented JSON with `.a.b[0].c` paths and code
/// keys, under the requested gRPC code.
#[test]
fn errors_become_statuses_the_client_can_read() {
    let mut error = VldError::single(IssueCode::MissingField, "Required");
    error.issues[0].path = vec![
        PathSegment::Field("user".into()),
        PathSegment::Field("items".into()),
        PathSegment::Index(0),
        PathSegment::Field("name".into()),
    ];
    let status = vld_status(&error);
    assert_eq!(
        serde_json::from_str::<Value>(status.message()).unwrap(),
        json!({
            "error": "VALIDATION_ERROR",
            "message": "Request validation failed",
            "details": [{"path": ".user.items[0].name", "message": "Required", "code": "missing_field"}],
        })
    );

    let custom = VldError::single(
        IssueCode::Custom {
            code: "auth".into(),
        },
        "unauthorized",
    );
    let status = vld_status_with_code(&custom, Code::PermissionDenied);
    assert_eq!(
        details(&status, Code::PermissionDenied),
        pairs(&[("", "auth")])
    );
    assert_eq!(
        details(&vld_status(&custom), Code::InvalidArgument),
        pairs(&[("", "auth")])
    );
}
