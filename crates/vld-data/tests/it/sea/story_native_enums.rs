//! SeaORM 2 only: native database enums (`Value::Enum`, which SeaORM 1 does
//! not have).

use super::vld_sea;
use serde_json::json;
use vld_sea::prelude::*;
use vld_sea::sea_orm::Set;

/// Given an entity with a native database enum column and a nullable one
/// (SeaORM 2 sends them as `Value::Enum`),
/// when its ActiveModel is turned into JSON,
/// then the variant is its string value and the missing one `null`.
#[test]
fn native_enums_map_to_their_string_value() {
    pub mod ticket {
        use super::vld_sea::sea_orm;
        use sea_orm::entity::prelude::*;

        #[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
        #[sea_orm(db_type = "Enum", enum_name = "status")]
        pub enum Status {
            #[sea_orm(string_value = "active")]
            Active,
            #[sea_orm(string_value = "closed")]
            Closed,
        }

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "tickets")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub status: Status,
            pub previous: Option<Status>,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }
    let am = ticket::ActiveModel {
        id: Set(1),
        status: Set(ticket::Status::Active),
        previous: Set(None),
    };
    assert_eq!(
        active_model_to_json(&am),
        json!({"id": 1, "status": "active", "previous": null})
    );
    let am = ticket::ActiveModel {
        previous: Set(Some(ticket::Status::Closed)),
        ..am
    };
    assert_eq!(active_model_to_json(&am)["previous"], "closed");
}
