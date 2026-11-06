//! `from_serde`, `from_str`, `enum_of` adapters.

use crate::common::{code, Role};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

#[derive(Debug, PartialEq, serde::Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[test]
fn from_serde_adapter() {
    let role = vld::from_serde::<Role>();
    assert_eq!(role.parse(r#""admin""#).unwrap(), Role::Admin);
    let err = role.parse(r#""root""#).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::ParseError);
    assert!(err.issues[0].message.contains("unknown variant"));
    assert_eq!(err.issues[0].received, Some(json!("root")));

    let p = vld::from_serde::<Point>()
        .parse(r#"{"x": 1, "y": -2}"#)
        .unwrap();
    assert_eq!(p, Point { x: 1, y: -2 });
    let err = vld::from_serde::<Point>().parse(r#"{"x": 1}"#).unwrap_err();
    assert!(err.issues[0].message.contains("missing field"));

    let nested = vld::array(vld::from_serde::<Role>());
    let err = nested.parse(r#"["user", "x"]"#).unwrap_err();
    assert_eq!(err.issues[0].path, vec![PathSegment::Index(1)]);
}

#[test]
fn from_str_adapter() {
    use std::net::IpAddr;
    let ip = vld::from_str::<IpAddr>();
    assert!(ip.parse(r#""192.168.0.1""#).unwrap().is_ipv4());
    assert!(ip.parse(r#""::1""#).unwrap().is_ipv6());
    let err = ip.parse(r#""300.0.0.1""#).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_format");
    assert_eq!(
        err.issues[0].message,
        "300.0.0.1".parse::<IpAddr>().unwrap_err().to_string()
    );
    assert_eq!(code(ip.parse("1")).key(), "invalid_type");
    let err = ip.clone().type_error("ip string").parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "ip string");

    assert_eq!(vld::from_str::<u32>().parse(r#""42""#).unwrap(), 42);
    assert!(vld::from_str::<u32>().parse(r#""-1""#).is_err());
}

#[cfg(feature = "string-advanced")]
#[test]
fn from_str_third_party_types() {
    let id = vld::from_str::<uuid::Uuid>()
        .parse(r#""67e55044-10b1-426f-9247-bb680e5fe0c8""#)
        .unwrap();
    assert_eq!(id.get_version_num(), 4);
    assert!(vld::from_str::<uuid::Uuid>().parse(r#""nope""#).is_err());
    let url = vld::from_str::<url::Url>()
        .parse(r#""https://example.com/a?b=1""#)
        .unwrap();
    assert_eq!(url.host_str(), Some("example.com"));
    let v = vld::from_str::<semver::Version>()
        .parse(r#""1.2.3-beta.1""#)
        .unwrap();
    assert_eq!(v.minor, 2);
    assert!(vld::from_str::<semver::Version>()
        .parse(r#""1.2""#)
        .is_err());
}

#[test]
fn enum_of_adapter() {
    #[derive(Debug, Clone, PartialEq)]
    enum Level {
        Low,
        High,
    }
    let level = vld::enum_of(&[("low", Level::Low), ("high", Level::High)]);
    assert_eq!(level.parse(r#""low""#).unwrap(), Level::Low);
    assert_eq!(
        code(level.parse(r#""mid""#)),
        IssueCode::InvalidEnumValue {
            options: vec!["low".into(), "high".into()]
        }
    );
    assert_eq!(code(level.parse("1")).key(), "invalid_type");
    let err = level.type_error("level!").parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "level!");
    // Empty enum rejects everything.
    let none: ZEnumOf<u8> = vld::enum_of(&[]);
    assert!(none.parse(r#""x""#).is_err());
}
