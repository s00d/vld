//! Stories: entities whose `before_save` validates (`impl_vld_before_save!`),
//! saved through SeaORM's `MockDatabase`: the logged statements show what
//! reached the database.

use crate::common::*;
use vld_sea::sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseConnection, DbBackend, DbErr, MockDatabase, Set,
    Unchanged, Value,
};

/// Every statement the mock database received: `(sql, bound values)`.
fn statements(db: DatabaseConnection) -> Vec<(String, Vec<Value>)> {
    db.into_transaction_log()
        .iter()
        .flat_map(|t| t.statements().to_vec())
        .map(|s| (s.sql, s.values.map(|v| v.0).unwrap_or_default()))
        .collect()
}

#[track_caller]
fn custom(err: DbErr) -> String {
    match err {
        DbErr::Custom(msg) => msg,
        other => panic!("expected DbErr::Custom, got {other:?}"),
    }
}

/// Given the `users` entity validated with `UserInput` on every save,
/// when a valid user is inserted, then an invalid one,
/// then the valid one is sent as one INSERT with its values and returned, and
/// the invalid one fails with `DbErr::Custom` naming the bad column before any
/// statement is sent.
#[tokio::test]
async fn insert_validates_before_reaching_the_database() {
    let saved = user::Model {
        id: 1,
        name: "Alice".into(),
        email: "a@b.co".into(),
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![saved.clone()]])
        .into_connection();
    let am = user::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set("Alice".to_owned()),
        email: Set("a@b.co".to_owned()),
    };
    assert_eq!(am.clone().insert(&db).await.unwrap(), saved);
    let bad = user::ActiveModel {
        email: Set("nope".to_owned()),
        ..am
    };
    let msg = custom(bad.insert(&db).await.unwrap_err());
    assert!(
        msg.starts_with("Validation error: ") && msg.contains(".email: "),
        "{msg}"
    );

    let sent = statements(db);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(
        sent[0].0.starts_with(r#"INSERT INTO "users""#),
        "{}",
        sent[0].0
    );
    assert_eq!(sent[0].1, vec![Value::from("Alice"), Value::from("a@b.co")]);
}

/// Given the `accounts` entity validated with `UserInput` on insert and
/// `UserPatch` on update,
/// when an insert misses the email, an update sets only a valid email, and an
/// update sets an invalid one,
/// then the insert is rejected as `.email` missing, the partial update passes
/// (the `NotSet` name is not required by the patch schema) and is sent, and
/// the invalid update is rejected unsent.
#[tokio::test]
async fn insert_and_update_use_their_own_schemas() {
    let updated = account::Model {
        id: 7,
        name: "Ann".into(),
        email: "new@b.co".into(),
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![updated.clone()]])
        .into_connection();

    let no_email = account::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set("Ann".to_owned()),
        email: ActiveValue::NotSet,
    };
    let msg = custom(no_email.insert(&db).await.unwrap_err());
    assert!(msg.contains(".email: "), "{msg}");

    let patch = account::ActiveModel {
        id: Unchanged(7),
        name: ActiveValue::NotSet,
        email: Set("new@b.co".to_owned()),
    };
    assert_eq!(patch.clone().update(&db).await.unwrap(), updated);
    let bad = account::ActiveModel {
        email: Set("nope".to_owned()),
        ..patch
    };
    let msg = custom(bad.update(&db).await.unwrap_err());
    assert!(msg.contains(".email: "), "{msg}");

    let sent = statements(db);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(
        sent[0].0.starts_with(r#"UPDATE "accounts""#),
        "{}",
        sent[0].0
    );
    assert_eq!(sent[0].1, vec![Value::from("new@b.co"), Value::from(7)]);
}
