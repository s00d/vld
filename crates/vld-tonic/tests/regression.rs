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
            x_page: i64 => vld::number().int(),
        }
    }

    let mut req = Request::new(());
    req.metadata_mut()
        .insert("x-account", "01234".parse().unwrap());
    req.metadata_mut().insert("x-page", "3".parse().unwrap());

    let meta: Meta = vld_tonic::validate_metadata(&req).unwrap();
    assert_eq!(meta.x_account, "01234");
    assert_eq!(meta.x_page, 3);
}
