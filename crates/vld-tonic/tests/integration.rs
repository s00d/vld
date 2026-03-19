use serde::{Deserialize, Serialize};
use tonic::Request;

// ---- impl_validate! + validate/validate_ref ----

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestMessage {
    name: String,
    email: String,
    age: i32,
}

vld_tonic::impl_validate!(TestMessage {
    name  => vld::string().min(2).max(50),
    email => vld::string().email(),
    age   => vld::number().int().min(0).max(150),
});

#[test]
fn validate_valid_message() {
    let req = Request::new(TestMessage {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    });
    let result = vld_tonic::validate(req);
    assert!(result.is_ok());
    let msg = result.unwrap();
    assert_eq!(msg.name, "Alice");
}

#[test]
fn validate_invalid_message() {
    let req = Request::new(TestMessage {
        name: "A".into(),
        email: "bad".into(),
        age: -5,
    });
    let result = vld_tonic::validate(req);
    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("VALIDATION_ERROR"));
}

#[test]
fn validate_ref_valid() {
    let msg = TestMessage {
        name: "Bob".into(),
        email: "bob@test.com".into(),
        age: 25,
    };
    assert!(vld_tonic::validate_ref(&msg).is_ok());
}

#[test]
fn validate_ref_invalid() {
    let msg = TestMessage {
        name: "".into(),
        email: "nope".into(),
        age: -1,
    };
    let result = vld_tonic::validate_ref(&msg);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[test]
fn inherent_validate_method() {
    let good = TestMessage {
        name: "Charlie".into(),
        email: "c@d.com".into(),
        age: 40,
    };
    assert!(good.validate().is_ok());
    assert!(good.is_valid());

    let bad = TestMessage {
        name: "".into(),
        email: "bad".into(),
        age: 200,
    };
    assert!(bad.validate().is_err());
    assert!(!bad.is_valid());
}

// ---- validate_with (schema-based) ----

vld::schema! {
    #[derive(Debug)]
    struct UserSchema {
        name: String  => vld::string().min(2),
        email: String => vld::string().email(),
    }
}

#[derive(Debug, Serialize)]
struct SimpleMsg {
    name: String,
    email: String,
}

#[test]
fn validate_with_valid() {
    let req = Request::new(SimpleMsg {
        name: "Alice".into(),
        email: "a@b.com".into(),
    });
    assert!(vld_tonic::validate_with::<UserSchema, _>(req).is_ok());
}

#[test]
fn validate_with_invalid() {
    let req = Request::new(SimpleMsg {
        name: "A".into(),
        email: "bad".into(),
    });
    let result = vld_tonic::validate_with::<UserSchema, _>(req);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[test]
fn validate_with_ref_works() {
    let msg = SimpleMsg {
        name: "Bob".into(),
        email: "b@c.com".into(),
    };
    assert!(vld_tonic::validate_with_ref::<UserSchema, _>(&msg).is_ok());

    let bad = SimpleMsg {
        name: "".into(),
        email: "x".into(),
    };
    assert!(vld_tonic::validate_with_ref::<UserSchema, _>(&bad).is_err());
}

// ---- Metadata validation ----

vld::schema! {
    #[derive(Debug, Clone)]
    struct AuthMeta {
        authorization: String => vld::string().min(1),
    }
}

#[test]
fn validate_metadata_valid() {
    let mut req = Request::new(());
    req.metadata_mut()
        .insert("authorization", "Bearer token123".parse().unwrap());

    let result = vld_tonic::validate_metadata::<AuthMeta, _>(&req);
    assert!(result.is_ok());
    let meta = result.unwrap();
    assert_eq!(meta.authorization, "Bearer token123");
}

#[test]
fn validate_metadata_missing() {
    let req = Request::new(());
    let result = vld_tonic::validate_metadata::<AuthMeta, _>(&req);
    assert!(result.is_err());
}

#[test]
fn validate_metadata_kebab_to_snake() {
    vld::schema! {
        #[derive(Debug, Clone)]
        struct CustomHeaders {
            x_request_id: String => vld::string().min(1),
        }
    }

    let mut req = Request::new(());
    req.metadata_mut()
        .insert("x-request-id", "abc-123".parse().unwrap());

    let result = vld_tonic::validate_metadata::<CustomHeaders, _>(&req);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().x_request_id, "abc-123");
}

// ---- Metadata interceptor ----

#[test]
fn metadata_interceptor_valid() {
    let mut req: Request<()> = Request::new(());
    req.metadata_mut()
        .insert("authorization", "Bearer abc".parse().unwrap());

    let result = vld_tonic::metadata_interceptor::<AuthMeta>(req);
    assert!(result.is_ok());
    let req = result.unwrap();
    let auth = req.extensions().get::<AuthMeta>().unwrap();
    assert_eq!(auth.authorization, "Bearer abc");
}

#[test]
fn metadata_interceptor_invalid() {
    let req: Request<()> = Request::new(());
    let result = vld_tonic::metadata_interceptor::<AuthMeta>(req);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[test]
fn validated_metadata_helper() {
    let mut req: Request<()> = Request::new(());
    req.metadata_mut()
        .insert("authorization", "Bearer xyz".parse().unwrap());

    let req = vld_tonic::metadata_interceptor::<AuthMeta>(req).unwrap();
    let auth = vld_tonic::validated_metadata::<AuthMeta, _>(&req);
    assert!(auth.is_some());
    assert_eq!(auth.unwrap().authorization, "Bearer xyz");
}

// ---- vld_status conversion ----

#[test]
fn vld_status_invalid_argument() {
    let error = vld::error::VldError::single(
        vld::error::IssueCode::Custom {
            code: "test".into(),
        },
        "test error",
    );
    let status = vld_tonic::vld_status(&error);
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("VALIDATION_ERROR"));
    assert!(status.message().contains("test error"));
}

#[test]
fn vld_status_custom_code() {
    let error = vld::error::VldError::single(
        vld::error::IssueCode::Custom {
            code: "auth".into(),
        },
        "unauthorized",
    );
    let status = vld_tonic::vld_status_with_code(&error, tonic::Code::PermissionDenied);
    assert_eq!(status.code(), tonic::Code::PermissionDenied);
}

// ---- Coercion in metadata ----

#[test]
fn metadata_coerces_numbers() {
    vld::schema! {
        #[derive(Debug, Clone)]
        struct PageMeta {
            x_page_size: i64 => vld::number().int().min(1).max(100),
        }
    }

    let mut req = Request::new(());
    req.metadata_mut()
        .insert("x-page-size", "25".parse().unwrap());

    let result = vld_tonic::validate_metadata::<PageMeta, _>(&req);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().x_page_size, 25);
}
