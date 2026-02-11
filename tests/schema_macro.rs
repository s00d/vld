use serde_json::json;
use vld::prelude::*;

#[test]
fn schema_macro_basic() {
    vld::schema! {
        #[derive(Debug)]
        struct TestUser {
            name: String => vld::string().min(2),
            age: Option<i64> => vld::number().int().min(0).optional(),
        }
    }

    let user = TestUser::parse(r#"{"name": "Alex", "age": 25}"#).unwrap();
    assert_eq!(user.name, "Alex");
    assert_eq!(user.age, Some(25));

    let user2 = TestUser::parse(r#"{"name": "Bob"}"#).unwrap();
    assert_eq!(user2.age, None);

    assert!(TestUser::parse(r#"{"name": "A"}"#).is_err());
}

#[test]
fn schema_macro_error_accumulation() {
    vld::schema! {
        #[derive(Debug)]
        struct TestData {
            name: String => vld::string().min(3),
            email: String => vld::string().email(),
        }
    }

    let result = TestData::parse(r#"{"name": "ab", "email": "bad"}"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().issues.len() >= 2);
}

#[test]
fn nested_schemas() {
    vld::schema! {
        #[derive(Debug)]
        struct Address {
            city: String => vld::string().min(1),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        struct Person {
            name: String => vld::string(),
            address: Address => vld::nested(Address::parse_value),
        }
    }

    let p = Person::parse(r#"{"name": "Alex", "address": {"city": "Moscow"}}"#).unwrap();
    assert_eq!(p.address.city, "Moscow");
}

#[test]
fn parse_from_value() {
    vld::schema! {
        #[derive(Debug)]
        struct Simple {
            name: String => vld::string(),
        }
    }

    let val = json!({"name": "test"});
    assert!(Simple::parse(&val).is_ok());
}

#[test]
fn enum_in_schema_macro() {
    vld::schema! {
        #[derive(Debug)]
        struct Config {
            mode: String => vld::enumeration(&["dev", "prod", "test"]),
            port: i64 => vld::number().int().min(1).max(65535),
        }
    }

    let c = Config::parse(r#"{"mode": "dev", "port": 8080}"#).unwrap();
    assert_eq!(c.mode, "dev");
    assert!(Config::parse(r#"{"mode": "invalid", "port": 8080}"#).is_err());
}
