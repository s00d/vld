//! Helpers shared by the SQLx test modules of one major.
#![allow(dead_code)] // each feature combination uses a different subset

use super::sqlx::{self, SqlitePool};
use super::vld_sqlx::VldSqlxError;

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
    pub struct PriceField {
        pub value: f64 => vld::number().min(0.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct ActiveField {
        pub value: bool => vld::boolean(),
    }
}

/// A user row as the application writes and reads it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NewUser {
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

/// `(path, code)` of every issue of a `Validation` error (panics otherwise).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldSqlxError>) -> Vec<(String, String)> {
    match r {
        Err(e) => issues_of(&e),
        Ok(v) => panic!("expected a validation error, got Ok({v:?})"),
    }
}

#[track_caller]
pub fn issues_of(e: &VldSqlxError) -> Vec<(String, String)> {
    match e {
        VldSqlxError::Validation(e) => e
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

/// A fresh in-memory database of its own.
///
/// Not `":memory:"`: SQLx names each such database `sqlx-in-memory-<n>` in
/// SQLite's process-wide shared cache, with one counter per SQLx crate, so
/// with both majors in one test binary a 0.8 pool and a 0.9 pool would open
/// the same database. The name here carries the major.
pub async fn memory_pool() -> SqlitePool {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let url = format!("sqlite:file:vld-{}-{n}?mode=memory", super::MAJOR);
    SqlitePool::connect(&url).await.unwrap()
}

/// A fresh in-memory database with a `users` table.
pub async fn users_db() -> SqlitePool {
    let pool = memory_pool().await;
    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

/// Run a future on a current-thread runtime (for proptest bodies).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}
