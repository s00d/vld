//! Model: every common column type through `active_model_to_json`.
//!
//! proptest draws a row of every always-mapped column type (signed and
//! unsigned integers of each width, `f32` / `f64` including NaN and ±inf,
//! bool, text, bytes, a nullable int, a custom `char` value type) and, per column, whether the
//! ActiveModel holds it `Set`, `Unchanged` or `NotSet`. The model: the JSON
//! is what `serde_json::to_value(&model)` gives for the present columns
//! (`NotSet` omitted, non-finite floats `null`), except an `f32`, which maps
//! through its decimal form. `validate_active` on the full ActiveModel and
//! `validate_model` on the model give the same issues.

use crate::common::*;
use proptest::prelude::*;
use serde_json::{Value as J, json};
use vld_sea::prelude::*;
use vld_sea::sea_orm::ActiveValue;

pub mod row {
    use sea_orm::entity::prelude::*;
    use sea_orm::sea_query::{ArrayType, ColumnType, Nullable, ValueType, ValueTypeErr};
    use sea_orm::{ColIdx, QueryResult, TryGetError, TryGetable};
    use vld_sea::sea_orm;

    /// A user-defined column type holding a `char` (sent as `Value::Char`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
    pub struct Initial(pub char);

    impl From<Initial> for Value {
        fn from(i: Initial) -> Self {
            Value::Char(Some(i.0))
        }
    }
    impl ValueType for Initial {
        fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
            match v {
                Value::Char(Some(c)) => Ok(Initial(c)),
                _ => Err(ValueTypeErr),
            }
        }
        fn type_name() -> String {
            "Initial".into()
        }
        fn array_type() -> ArrayType {
            ArrayType::Char
        }
        fn column_type() -> ColumnType {
            ColumnType::Char(None)
        }
    }
    impl Nullable for Initial {
        fn null() -> Value {
            Value::Char(None)
        }
    }
    impl TryGetable for Initial {
        fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
            String::try_get_by(res, index).map(|s| Initial(s.chars().next().unwrap_or(' ')))
        }
    }

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize)]
    #[sea_orm(table_name = "rows")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub tiny: i8,
        pub small: i16,
        pub big: i64,
        pub utiny: u8,
        pub usmall: u16,
        pub uint: u32,
        pub ubig: u64,
        #[sea_orm(column_type = "Float")]
        pub real: f32,
        #[sea_orm(column_type = "Double")]
        pub double: f64,
        pub flag: bool,
        pub text: String,
        pub bytes: Vec<u8>,
        pub maybe: Option<i32>,
        #[sea_orm(column_type = "Char(None)")]
        pub initial: Initial,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

vld::schema! {
    #[derive(Debug)]
    pub struct RowSchema {
        pub tiny: i64 => vld::number().int().min(0),
        pub ubig: i64 => vld::number().int().max(1_000_000),
        pub double: f64 => vld::number().min(0.0),
        pub text: String => vld::string().min(1).max(5),
        pub bytes: Vec<i64> => vld::array(vld::number().int()).max_len(3),
        pub maybe: Option<i64> => vld::number().int().optional(),
    }
}

fn float64() -> impl Strategy<Value = f64> {
    prop_oneof![
        -1e6f64..1e6,
        any::<f64>(),
        Just(f64::NAN),
        Just(f64::INFINITY)
    ]
}

fn float32() -> impl Strategy<Value = f32> {
    prop_oneof![
        -1e3f32..1e3,
        any::<f32>(),
        Just(0.1f32),
        Just(f32::NAN),
        Just(f32::NEG_INFINITY)
    ]
}

fn model() -> impl Strategy<Value = row::Model> {
    (
        (any::<i32>(), any::<i8>(), any::<i16>(), any::<i64>()),
        (any::<u8>(), any::<u16>(), any::<u32>(), any::<u64>()),
        (float32(), float64(), any::<bool>()),
        (
            "\\PC{0,7}",
            prop::collection::vec(any::<u8>(), 0..5),
            any::<Option<i32>>(),
            any::<char>(),
        ),
    )
        .prop_map(
            |(
                (id, tiny, small, big),
                (utiny, usmall, uint, ubig),
                (real, double, flag),
                (text, bytes, maybe, c),
            )| {
                let initial = row::Initial(c);
                row::Model {
                    id,
                    tiny,
                    small,
                    big,
                    utiny,
                    usmall,
                    uint,
                    ubig,
                    real,
                    double,
                    flag,
                    text,
                    bytes,
                    maybe,
                    initial,
                }
            },
        )
}

/// 0 = `NotSet`, 1 = `Set`, 2 = `Unchanged`.
fn hold<V: Into<vld_sea::sea_orm::Value>>(mode: u8, v: V) -> ActiveValue<V> {
    match mode {
        0 => ActiveValue::NotSet,
        1 => ActiveValue::Set(v),
        _ => ActiveValue::Unchanged(v),
    }
}

fn active(m: &row::Model, modes: &[u8; 15]) -> row::ActiveModel {
    let m = m.clone();
    row::ActiveModel {
        id: hold(modes[0], m.id),
        tiny: hold(modes[1], m.tiny),
        small: hold(modes[2], m.small),
        big: hold(modes[3], m.big),
        utiny: hold(modes[4], m.utiny),
        usmall: hold(modes[5], m.usmall),
        uint: hold(modes[6], m.uint),
        ubig: hold(modes[7], m.ubig),
        real: hold(modes[8], m.real),
        double: hold(modes[9], m.double),
        flag: hold(modes[10], m.flag),
        text: hold(modes[11], m.text),
        bytes: hold(modes[12], m.bytes),
        maybe: hold(modes[13], m.maybe),
        initial: hold(modes[14], m.initial),
    }
}

const COLUMNS: [&str; 15] = [
    "id", "tiny", "small", "big", "utiny", "usmall", "uint", "ubig", "real", "double", "flag",
    "text", "bytes", "maybe", "initial",
];

fn predicted(m: &row::Model, modes: &[u8; 15]) -> J {
    let J::Object(mut all) = serde_json::to_value(m).unwrap() else {
        unreachable!()
    };
    let real = if m.real.is_finite() {
        json!(m.real.to_string().parse::<f64>().unwrap())
    } else {
        J::Null
    };
    all.insert("real".into(), real);
    for (col, mode) in COLUMNS.iter().zip(modes) {
        if *mode == 0 {
            all.remove(*col);
        }
    }
    J::Object(all)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn model_active_model_json_matches_serde(m in model(), modes in any::<[u8; 15]>()) {
        let modes = modes.map(|x| x % 3);
        prop_assert_eq!(active_model_to_json(&active(&m, &modes)), predicted(&m, &modes));

        let full = active(&m, &[1; 15]);
        let from_active = validate_active::<RowSchema, _>(&full).err();
        let from_model = validate_model::<RowSchema, _>(&m).err();
        prop_assert_eq!(
            from_active.map(|e| issues::<()>(Err(e))),
            from_model.map(|e| issues::<()>(Err(e)))
        );
    }
}
