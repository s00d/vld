//! Regression tests for the live end-to-end findings (0.5.0).

use tonic::Request;

#[test]
fn numeric_looking_metadata_still_fits_string_fields() {
    vld::schema! {
        #[derive(Debug, Clone)]
        struct Meta {
            x_user: String => vld::string(),
            x_page: i64 => vld::number().int(),
        }
    }

    let mut req = Request::new(());
    req.metadata_mut().insert("x-user", "42".parse().unwrap());
    req.metadata_mut().insert("x-page", "3".parse().unwrap());

    let meta: Meta = vld_tonic::validate_metadata(&req).unwrap();
    assert_eq!(meta.x_user, "42");
    assert_eq!(meta.x_page, 3);
}
