//! Helpers and entities shared by every module of the `it` test binary.
#![allow(dead_code)]

use vld_sea::VldSeaError;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserPatch {
        pub name: Option<String>  => vld::string().min(1).max(100).optional(),
        pub email: Option<String> => vld::string().email().optional(),
    }
}

/// `users`: validated on every save with `UserInput`.
pub mod user {
    use sea_orm::entity::prelude::*;
    use vld_sea::sea_orm;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    vld_sea::impl_vld_before_save!(ActiveModel, super::UserInput);
}

/// `accounts`: `UserInput` on insert, `UserPatch` on update.
pub mod account {
    use sea_orm::entity::prelude::*;
    use vld_sea::sea_orm;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "accounts")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    vld_sea::impl_vld_before_save!(ActiveModel, insert: super::UserInput, update: super::UserPatch);
}

/// `(path, code)` of every issue of a `Validation` error (panics otherwise).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldSeaError>) -> Vec<(String, String)> {
    match r {
        Err(VldSeaError::Validation(e)) => e
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
