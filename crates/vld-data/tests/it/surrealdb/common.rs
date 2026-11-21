//! Helpers shared by every module of the `it` test binary.
#![allow(dead_code)]

use vld_data::surrealdb::VldSurrealError;

vld::schema! {
    #[derive(Debug)]
    pub struct PersonSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct InnerAddr {
        pub city: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct OuterDoc {
        pub tags: Vec<String> => vld::array(vld::string().min(1)),
        pub addr: InnerAddr => vld::nested(InnerAddr::parse_value),
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Person {
    pub name: String,
    pub email: String,
    pub age: i64,
}

pub fn person(name: &str, email: &str, age: i64) -> Person {
    Person {
        name: name.into(),
        email: email.into(),
        age,
    }
}

/// `(path, code)` of every issue of a `Validation` error (panics otherwise).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldSurrealError>) -> Vec<(String, String)> {
    match r {
        Err(e) => issues_of(&e),
        Ok(v) => panic!("expected a validation error, got Ok({v:?})"),
    }
}

#[track_caller]
pub fn issues_of(e: &VldSurrealError) -> Vec<(String, String)> {
    match e {
        VldSurrealError::Validation(e) => e
            .issues
            .iter()
            .map(|i| {
                (
                    i.path.iter().map(ToString::to_string).collect(),
                    i.code.key().to_string(),
                )
            })
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(path, code)` pairs for comparing with [`issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// A value whose `Serialize` impl fails.
pub struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}
