//! Model: `VldInt` / `VldFloat` written to and read from INTEGER (i32) and
//! REAL (f32) columns.
//!
//! proptest draws any `i64` (biased to the i32 edges) and any finite `f64`
//! (biased to the f32 range edges and tiny values). The model: the INSERT
//! fails exactly when the integer does not fit an i32 or the float overflows
//! an f32 (never a silent truncation); otherwise the row is found by
//! `filter(n.eq(wrapper))` and reads back the same integer and the float as
//! the decimal the f32 prints as (`0.1` stays `0.1`). With the `postgres` /
//! `mysql` features the same values are serialized as Diesel binds them for
//! those backends (no server needed): big-endian / native-endian `i32` and
//! `f32` bytes, or an error under the same condition.

use crate::common::*;
use diesel::prelude::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use vld_diesel::prelude::*;

diesel::table! {
    narrow (id) {
        id -> Integer,
        n -> Integer,
        r -> Float,
    }
}

#[derive(Insertable)]
#[diesel(table_name = narrow)]
struct NewNarrow {
    n: VldInt<AnyInt>,
    r: VldFloat<AnyFloat>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = narrow)]
struct Narrow {
    #[allow(dead_code)]
    id: i32,
    n: VldInt<AnyInt>,
    r: VldFloat<AnyFloat>,
}

fn int() -> impl Strategy<Value = i64> {
    let max = i64::from(i32::MAX);
    let min = i64::from(i32::MIN);
    prop_oneof![
        -1000i64..1000,
        (max - 2)..=(max + 2),
        (min - 2)..=(min + 2),
        any::<i64>(),
    ]
}

fn float() -> impl Strategy<Value = f64> {
    let max = f64::from(f32::MAX);
    prop_oneof![
        -1e6f64..1e6,
        Just(0.1),
        Just(max),
        Just(-max),
        Just(max * 1.000_001),
        Just(-max * 2.0),
        Just(1e300),
        Just(1e-50),
        any::<f64>().prop_filter("finite", |f| f.is_finite()),
    ]
}

/// `Debug` of a bind collector holding `$v` bound as SQL type `$st` for backend
/// `$db` (the collector's bytes are only reachable through `Debug`), or `None`
/// when serializing fails.
#[cfg(any(feature = "postgres", feature = "mysql"))]
macro_rules! bound {
    ($db:ty, $st:ty, $v:expr, $lookup:expr) => {{
        use diesel::query_builder::BindCollector;
        let mut c = diesel::query_builder::bind_collector::RawBytesBindCollector::<$db>::new();
        c.push_bound_value::<$st, _>($v, $lookup)
            .ok()
            .map(|()| format!("{c:?}"))
    }};
}

#[cfg(feature = "postgres")]
struct NoLookup;
#[cfg(feature = "postgres")]
impl diesel::pg::PgMetadataLookup for NoLookup {
    fn lookup_type(&mut self, _: &str, _: Option<&str>) -> diesel::pg::PgTypeMetadata {
        unreachable!("built-in types only")
    }
}

/// `(int, float)` binds of one backend; `None` = serializing failed.
type Binds<T> = (Option<T>, Option<T>);

/// What each server backend would be sent for `(n, r)`, as `Debug` text.
#[allow(unused_variables, unused_imports)]
fn server_binds(n: &VldInt<AnyInt>, r: &VldFloat<AnyFloat>) -> Vec<Binds<String>> {
    use diesel::sql_types::{Float, Integer};
    vec![
        #[cfg(feature = "postgres")]
        (
            bound!(diesel::pg::Pg, Integer, n, &mut NoLookup),
            bound!(diesel::pg::Pg, Float, r, &mut NoLookup),
        ),
        #[cfg(feature = "mysql")]
        (
            bound!(diesel::mysql::Mysql, Integer, n, &mut ()),
            bound!(diesel::mysql::Mysql, Float, r, &mut ()),
        ),
    ]
}

/// The model of [`server_binds`]: the bytes each backend gets.
#[allow(unused_variables)]
fn expected_binds(n: i64, r: f64) -> Vec<Binds<Vec<u8>>> {
    let n32 = i32::try_from(n).ok();
    let f = Some(r as f32).filter(|f| f.is_finite());
    vec![
        #[cfg(feature = "postgres")]
        (
            n32.map(|n| n.to_be_bytes().to_vec()),
            f.map(|f| f.to_be_bytes().to_vec()),
        ),
        #[cfg(feature = "mysql")]
        (
            n32.map(|n| n.to_ne_bytes().to_vec()),
            f.map(|f| f.to_ne_bytes().to_vec()),
        ),
    ]
}

/// The f32 a REAL column keeps, widened through its decimal form.
fn stored(r: f64) -> Option<f64> {
    let f = r as f32;
    f.is_finite().then(|| f.to_string().parse().unwrap())
}

#[test]
fn model_integer_and_real_columns_never_truncate() {
    let conn = std::cell::RefCell::new(db(&[
        "CREATE TABLE narrow (id INTEGER PRIMARY KEY AUTOINCREMENT,
        n INTEGER NOT NULL, r REAL NOT NULL)",
    ]));
    TestRunner::new(Config::with_cases(512))
        .run(&(int(), float()), |(n, r)| {
            let conn = &mut *conn.borrow_mut();
            diesel::delete(narrow::table).execute(conn).unwrap();
            let row = NewNarrow {
                n: VldInt::new(n).unwrap(),
                r: VldFloat::new(r).unwrap(),
            };
            let sent = server_binds(&row.n, &row.r);
            for (got, want) in sent.iter().zip(expected_binds(n, r)) {
                for (got, want) in [(&got.0, want.0), (&got.1, want.1)] {
                    match (got, want) {
                        (None, None) => {}
                        (Some(text), Some(bytes)) => {
                            let needle = format!("binds: [Some({bytes:?})]");
                            prop_assert!(text.contains(&needle), "{text} lacks {needle}");
                        }
                        (got, want) => {
                            prop_assert!(false, "{n} / {r}: sent {got:?}, model {want:?}")
                        }
                    }
                }
            }
            let inserted = diesel::insert_into(narrow::table)
                .values(&row)
                .execute(conn);
            let expected = i32::try_from(n).ok().zip(stored(r));
            let Some((n32, r_back)) = expected else {
                prop_assert!(inserted.is_err(), "{n} / {r} should not fit");
                let count: i64 = narrow::table.count().get_result(conn).unwrap();
                prop_assert_eq!(count, 0);
                return Ok(());
            };
            prop_assert_eq!(inserted.ok(), Some(1));
            let found: Vec<Narrow> = narrow::table
                .filter(narrow::n.eq(VldInt::<AnyInt>::new(n).unwrap()))
                .select(Narrow::as_select())
                .load(conn)
                .unwrap();
            prop_assert_eq!(found.len(), 1);
            prop_assert_eq!(found[0].n.get(), i64::from(n32));
            prop_assert_eq!(found[0].r.get(), r_back);
            Ok(())
        })
        .unwrap();
}
