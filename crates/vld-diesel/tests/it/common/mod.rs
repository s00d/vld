//! Helpers shared by every module of the `it` test binary.
#![allow(dead_code)]

use diesel::prelude::*;
use vld_diesel::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AgeField {
        pub value: i64 => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AnyInt {
        pub value: i64 => vld::number().int(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct PriceField {
        pub value: f64 => vld::number().min(0.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AnyFloat {
        pub value: f64 => vld::number(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct ActiveField {
        pub value: bool => vld::boolean(),
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        name -> Text,
        email -> Text,
        age -> BigInt,
    }
}

#[derive(Debug, Clone, PartialEq, Insertable, serde::Serialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub name: String,
    pub email: String,
    pub age: i64,
}

#[derive(Debug, Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: i64,
}

pub fn user(name: &str, email: &str, age: i64) -> NewUser {
    NewUser {
        name: name.into(),
        email: email.into(),
        age,
    }
}

/// An in-memory database with the given tables (`CREATE TABLE` statements).
pub fn db(tables: &[&str]) -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    for t in tables {
        diesel::sql_query(*t).execute(&mut conn).unwrap();
    }
    conn
}

pub const USERS: &str = "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL, email TEXT NOT NULL, age BIGINT NOT NULL)";

/// `(path, code)` of every issue of a `Validation` error (panics otherwise).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldDieselError>) -> Vec<(String, String)> {
    match r {
        Err(e) => issues_of(&e),
        Ok(v) => panic!("expected a validation error, got Ok({v:?})"),
    }
}

#[track_caller]
pub fn issues_of(e: &VldDieselError) -> Vec<(String, String)> {
    match e {
        VldDieselError::Validation(e) => e
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
