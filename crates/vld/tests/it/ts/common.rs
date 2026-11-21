//! Helpers shared by the `ts` modules (the type-erased schema is
//! `crate::common::Erased`).
#![allow(dead_code)] // each module uses a different subset

use serde_json::Value;

/// A generator target, for corpus rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Zod,
    Zod4,
    Valibot,
    Ts,
}

/// Generated code for `schema` (a JSON Schema value) in `target`.
pub fn generate(target: Target, schema: &Value) -> String {
    match target {
        Target::Zod => vld::ts::__to_zod_from_value(schema),
        Target::Zod4 => vld::ts::__to_zod4_from_value(schema),
        Target::Valibot => vld::ts::__to_valibot_from_value(schema),
        Target::Ts => vld::ts::__to_ts_type_from_value(schema),
    }
}
