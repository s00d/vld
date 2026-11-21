//! Model: random scalars through the column wrappers and an SQLite table.
//!
//! proptest draws integers, floats (finite, negative, NaN, ±inf), strings of
//! any length and booleans. The model of the field schemas predicts whether
//! `new` accepts each value and the exact `(path, code)` when it does not;
//! every accepted value is bound into columns of each declared type (BIGINT /
//! INT4 / SMALLINT, DOUBLE / REAL, TEXT, BOOLEAN) and must decode back equal.

use super::common::*;
use super::sqlx::{self, Row, SqlitePool};
use super::vld_sqlx::prelude::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

vld::schema! {
    #[derive(Debug)]
    pub struct ShortName {
        pub value: String => vld::string().min(1).max(10),
    }
}

/// What the field schemas say about one value: `None` = accepted.
fn int_verdict(v: i64) -> Option<(&'static str, &'static str)> {
    match v {
        ..0 => Some((".value", "too_small")),
        151.. => Some((".value", "too_big")),
        _ => None,
    }
}

fn float_verdict(v: f64) -> Option<(&'static str, &'static str)> {
    if !v.is_finite() {
        Some(("", "not_finite"))
    } else if v < 0.0 {
        Some((".value", "too_small"))
    } else {
        None
    }
}

fn text_verdict(s: &str) -> Option<(&'static str, &'static str)> {
    match s.chars().count() {
        0 => Some((".value", "too_small")),
        11.. => Some((".value", "too_big")),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct Case {
    int: i64,
    small: i16,
    float: f64,
    text: String,
    flag: bool,
}

fn case() -> impl Strategy<Value = Case> {
    let int = prop_oneof![-5i64..160, any::<i64>()];
    let float = prop_oneof![
        -10.0f64..1e6,
        any::<f64>(),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
    ];
    (int, 0i16..=150, float, "\\PC{0,14}", any::<bool>()).prop_map(
        |(int, small, float, text, flag)| Case {
            int,
            small,
            float,
            text,
            flag,
        },
    )
}

async fn check(pool: &SqlitePool, c: &Case) -> Result<(), TestCaseError> {
    let int = VldInt::<AgeField>::new(c.int);
    let float = VldFloat::<PriceField>::new(c.float);
    let text = VldText::<ShortName>::new(c.text.clone());
    let flag = VldBool::<ActiveField>::new(c.flag).unwrap();

    for (got, want) in [
        (int.as_ref().err().map(issues_of), int_verdict(c.int)),
        (float.as_ref().err().map(issues_of), float_verdict(c.float)),
        (text.as_ref().err().map(issues_of), text_verdict(&c.text)),
    ] {
        prop_assert_eq!(got, want.map(|w| pairs(&[w])));
    }

    let (Ok(int), Ok(float), Ok(text)) = (int, float, text) else {
        return Ok(());
    };
    let small = VldInt::<AgeField>::new(i64::from(c.small)).unwrap();
    sqlx::query("DELETE FROM m").execute(pool).await.unwrap();
    sqlx::query("INSERT INTO m VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(int)
        .bind(int)
        .bind(small)
        .bind(float)
        .bind(float)
        .bind(&text)
        .bind(flag)
        .execute(pool)
        .await
        .unwrap();
    let row = sqlx::query("SELECT * FROM m")
        .fetch_one(pool)
        .await
        .unwrap();
    for (col, want) in [("big", int), ("i4", int), ("i2", small)] {
        prop_assert_eq!(row.try_get::<VldInt<AgeField>, _>(col).unwrap(), want);
    }
    for col in ["d", "r"] {
        prop_assert_eq!(row.try_get::<VldFloat<PriceField>, _>(col).unwrap(), float);
    }
    prop_assert_eq!(row.try_get::<VldText<ShortName>, _>("t").unwrap(), text);
    prop_assert_eq!(row.try_get::<VldBool<ActiveField>, _>("b").unwrap(), flag);
    Ok(())
}

#[test]
fn model_wrappers_accept_by_schema_and_round_trip() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let pool = rt.block_on(async {
        let pool = memory_pool().await;
        sqlx::query(
            "CREATE TABLE m (big BIGINT NOT NULL, i4 INT4 NOT NULL, i2 SMALLINT NOT NULL,
             d DOUBLE NOT NULL, r REAL NOT NULL, t TEXT NOT NULL, b BOOLEAN NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    });
    TestRunner::new(Config::with_cases(256))
        .run(&case(), |c| rt.block_on(check(&pool, &c)))
        .unwrap();
}
