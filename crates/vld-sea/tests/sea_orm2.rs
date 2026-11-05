//! sea-orm 2 integration: `validate_active`, `before_save` and
//! `impl_vld_before_save!` against a real `DeriveEntityModel` entity.
#![cfg(feature = "sea-orm-2")]

// The entity derive macros emit `sea_orm::` paths.
use sea_orm2 as sea_orm;
use vld_sea::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

mod user_entity {
    use super::sea_orm;
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub score: f32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    vld_sea::impl_vld_before_save!(ActiveModel, super::UserInput);
}

fn active(email: &str) -> user_entity::ActiveModel {
    use sea_orm::Set;
    user_entity::ActiveModel {
        id: Set(1),
        name: Set("Alice".to_owned()),
        email: Set(email.to_owned()),
        score: Set(0.1),
    }
}

#[test]
fn validate_active_and_json() {
    assert!(validate_active::<UserInput, _>(&active("a@b.co")).is_ok());
    assert!(validate_active::<UserInput, _>(&active("nope")).is_err());
    let json = active_model_to_json(&active("a@b.co"));
    // f32 maps through its decimal form.
    assert_eq!(json["score"], serde_json::json!(0.1));
}

#[test]
fn before_save_helper() {
    let ok: Result<(), sea_orm::DbErr> = before_save::<UserInput, _>(&active("a@b.co"));
    assert!(ok.is_ok());
    assert!(before_save::<UserInput, _>(&active("nope")).is_err());
}

#[tokio::test]
async fn macro_runs_validation_in_before_save() {
    use sea_orm::ActiveModelBehavior;
    let db = sea_orm::DatabaseConnection::default();
    assert!(active("a@b.co").before_save(&db, true).await.is_ok());
    let err = active("nope").before_save(&db, false).await.unwrap_err();
    assert!(err.to_string().contains("email"), "{err}");
}
