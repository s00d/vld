//! Regression tests for the 0.5.0 audit fixes.

use tonic::Request;
use vld::error::{IssueCode, PathSegment, VldError};

#[test]
fn status_details_use_clean_paths_and_code_keys() {
    let mut error = VldError::single(IssueCode::MissingField, "Required");
    error.issues[0].path = vec![
        PathSegment::Field("user".into()),
        PathSegment::Field("items".into()),
        PathSegment::Index(0),
        PathSegment::Field("name".into()),
    ];
    let status = vld_tonic::vld_status(&error);
    let body: serde_json::Value = serde_json::from_str(status.message()).unwrap();
    let detail = &body["details"][0];
    assert_eq!(detail["path"], ".user.items[0].name");
    assert_eq!(detail["code"], IssueCode::MissingField.key());
    assert_eq!(detail["message"], "Required");
}

#[test]
fn metadata_uses_lossless_shared_coercion() {
    vld::schema! {
        #[derive(Debug, Clone)]
        struct Meta {
            x_account: String => vld::string(),
            x_user: String => vld::string(),
            x_page: i64 => vld::number().int(),
        }
    }

    // Non-canonical ("01234") and canonical ("42") numbers both stay strings.
    let mut req = Request::new(());
    req.metadata_mut()
        .insert("x-account", "01234".parse().unwrap());
    req.metadata_mut().insert("x-user", "42".parse().unwrap());
    req.metadata_mut().insert("x-page", "3".parse().unwrap());

    let meta: Meta = vld_tonic::validate_metadata(&req).unwrap();
    assert_eq!(meta.x_account, "01234");
    assert_eq!(meta.x_user, "42");
    assert_eq!(meta.x_page, 3);
}

/// The interceptor stores `Validated<T>`, not a bare `T` that could collide
/// with another extension of the same type.
#[test]
fn interceptor_stores_validated_wrapper() {
    vld::schema! {
        #[derive(Debug, Clone)]
        struct Auth {
            authorization: String => vld::string().min(1),
        }
    }

    let mut req = Request::new(());
    req.metadata_mut()
        .insert("authorization", "Bearer x".parse().unwrap());
    let req = vld_tonic::metadata_interceptor::<Auth>(req).unwrap();
    assert!(req.extensions().get::<Auth>().is_none());
    assert_eq!(
        req.extensions()
            .get::<vld_tonic::Validated<Auth>>()
            .unwrap()
            .authorization,
        "Bearer x"
    );
    let auth: Auth = vld_tonic::validated_metadata(&req).unwrap();
    assert_eq!(auth.authorization, "Bearer x");
}
