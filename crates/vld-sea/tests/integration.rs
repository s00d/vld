use vld_sea::prelude::*;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct ItemInput {
        pub title: String => vld::string().min(1),
        pub price: f64    => vld::number().min(0.0),
        pub qty: i64      => vld::number().int().min(0),
    }
}

// ---------------------------------------------------------------------------
// SeaORM entity (manual definition without DeriveEntityModel to avoid
// pulling in heavy macros in tests)
// ---------------------------------------------------------------------------

mod user_entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

mod item_entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "items")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        #[sea_orm(column_type = "Double")]
        pub price: f64,
        pub qty: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ---------------------------------------------------------------------------
// Tests: validate_active (ActiveModel → JSON → vld)
// ---------------------------------------------------------------------------

#[test]
fn active_model_valid() {
    use sea_orm::Set;

    let am = user_entity::ActiveModel {
        id: Set(1),
        name: Set("Alice".to_owned()),
        email: Set("alice@example.com".to_owned()),
    };

    let result = validate_active::<UserInput, _>(&am);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
}

#[test]
fn active_model_invalid_email() {
    use sea_orm::Set;

    let am = user_entity::ActiveModel {
        id: Set(1),
        name: Set("Bob".to_owned()),
        email: Set("not-an-email".to_owned()),
    };

    let result = validate_active::<UserInput, _>(&am);
    assert!(result.is_err());
}

#[test]
fn active_model_invalid_name_too_short() {
    use sea_orm::Set;

    let am = user_entity::ActiveModel {
        id: Set(1),
        name: Set("".to_owned()),
        email: Set("bob@example.com".to_owned()),
    };

    let result = validate_active::<UserInput, _>(&am);
    assert!(result.is_err());
}

#[test]
fn active_model_item_valid() {
    use sea_orm::Set;

    let am = item_entity::ActiveModel {
        id: Set(1),
        title: Set("Widget".to_owned()),
        price: Set(9.99),
        qty: Set(10),
    };

    let result = validate_active::<ItemInput, _>(&am);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
}

#[test]
fn active_model_item_invalid_price() {
    use sea_orm::Set;

    let am = item_entity::ActiveModel {
        id: Set(1),
        title: Set("Widget".to_owned()),
        price: Set(-5.0),
        qty: Set(10),
    };

    let result = validate_active::<ItemInput, _>(&am);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Tests: active_model_to_json
// ---------------------------------------------------------------------------

#[test]
fn active_model_to_json_includes_set_fields() {
    use sea_orm::Set;

    let am = user_entity::ActiveModel {
        id: Set(42),
        name: Set("Charlie".to_owned()),
        email: Set("charlie@test.com".to_owned()),
    };

    let json = active_model_to_json(&am);
    assert_eq!(json["id"], 42);
    assert_eq!(json["name"], "Charlie");
    assert_eq!(json["email"], "charlie@test.com");
}

#[test]
fn active_model_to_json_skips_not_set() {
    use sea_orm::{ActiveValue, Set};

    let am = user_entity::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set("Dave".to_owned()),
        email: Set("dave@test.com".to_owned()),
    };

    let json = active_model_to_json(&am);
    let obj = json.as_object().unwrap();
    assert!(!obj.contains_key("id"), "NotSet field should be omitted");
    assert_eq!(json["name"], "Dave");
}

// ---------------------------------------------------------------------------
// Tests: validate_model (Serialize-based)
// ---------------------------------------------------------------------------

#[test]
fn validate_model_valid() {
    #[derive(serde::Serialize)]
    struct Input {
        name: String,
        email: String,
    }

    let input = Input {
        name: "Eve".into(),
        email: "eve@example.com".into(),
    };

    let result = validate_model::<UserInput, _>(&input);
    assert!(result.is_ok());
}

#[test]
fn validate_model_invalid() {
    #[derive(serde::Serialize)]
    struct Input {
        name: String,
        email: String,
    }

    let input = Input {
        name: "".into(),
        email: "bad".into(),
    };

    let result = validate_model::<UserInput, _>(&input);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Tests: validate_json
// ---------------------------------------------------------------------------

#[test]
fn validate_json_valid() {
    let json = serde_json::json!({
        "name": "Frank",
        "email": "frank@example.com",
    });
    let result = validate_json::<UserInput>(&json);
    assert!(result.is_ok());
}

#[test]
fn validate_json_invalid() {
    let json = serde_json::json!({
        "name": "",
        "email": "not-email",
    });
    let result = validate_json::<UserInput>(&json);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Tests: Validated wrapper
// ---------------------------------------------------------------------------

#[test]
fn validated_wrapper_valid() {
    #[derive(Debug, serde::Serialize)]
    struct Row {
        name: String,
        email: String,
    }

    let row = Row {
        name: "Grace".into(),
        email: "grace@example.com".into(),
    };
    let v = Validated::<UserInput, _>::new(row).unwrap();
    assert_eq!(v.inner().name, "Grace");
}

#[test]
fn validated_wrapper_invalid() {
    #[derive(Debug, serde::Serialize)]
    struct Row {
        name: String,
        email: String,
    }

    let row = Row {
        name: "".into(),
        email: "bad".into(),
    };
    assert!(Validated::<UserInput, _>::new(row).is_err());
}

// ---------------------------------------------------------------------------
// Tests: before_save helper
// ---------------------------------------------------------------------------

#[test]
fn before_save_ok() {
    use sea_orm::Set;

    let am = user_entity::ActiveModel {
        id: Set(1),
        name: Set("Heidi".to_owned()),
        email: Set("heidi@example.com".to_owned()),
    };

    let result: Result<(), sea_orm::DbErr> = before_save::<UserInput, _>(&am);
    assert!(result.is_ok());
}

#[test]
fn before_save_err() {
    use sea_orm::Set;

    let am = user_entity::ActiveModel {
        id: Set(1),
        name: Set("".to_owned()),
        email: Set("bad".to_owned()),
    };

    let result: Result<(), sea_orm::DbErr> = before_save::<UserInput, _>(&am);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        sea_orm::DbErr::Custom(msg) => {
            assert!(msg.contains("Validation error"), "msg: {}", msg);
        }
        other => panic!("Expected DbErr::Custom, got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Tests: error conversion
// ---------------------------------------------------------------------------

#[test]
fn vld_sea_error_display() {
    let err = VldSeaError::Serialization("oops".to_string());
    assert!(err.to_string().contains("oops"));
}

#[test]
fn vld_sea_error_into_dberr() {
    let err = VldSeaError::Serialization("test".to_string());
    let db_err: sea_orm::DbErr = err.into();
    match db_err {
        sea_orm::DbErr::Custom(msg) => assert!(msg.contains("test")),
        other => panic!("Expected Custom, got: {:?}", other),
    }
}
