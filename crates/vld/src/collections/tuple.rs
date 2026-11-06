use serde_json::Value;

use crate::error::{IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::VldSchema;

macro_rules! impl_tuple_schema {
    ($count:expr; $(($idx:tt, $T:ident, $v:ident)),+) => {
        impl<$($T: VldSchema),+> VldSchema for ($($T,)+) {
            type Output = ($($T::Output,)+);

            fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
                let arr = value.as_array().ok_or_else(|| {
                    crate::primitives::type_error(&None, "array", value)
                })?;

                if arr.len() != $count {
                    return Err(VldError::from_code(IssueCode::InvalidLength { origin: SizeOrigin::Tuple, expected: $count }));
                }

                let mut __errors = VldError::new();

                $(
                    let $v = match self.$idx.parse_value(&arr[$idx]) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            __errors = __errors.merge(e.with_prefix(PathSegment::Index($idx)));
                            None
                        }
                    };
                )+

                match ($($v,)+) {
                    ($(Some($v),)+) => Ok(($($v,)+)),
                    _ => Err(__errors),
                }
            }
        }
    };
}

impl_tuple_schema!(1; (0, A, _a));
impl_tuple_schema!(2; (0, A, _a), (1, B, _b));
impl_tuple_schema!(3; (0, A, _a), (1, B, _b), (2, C, _c));
impl_tuple_schema!(4; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d));
impl_tuple_schema!(5; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e));
impl_tuple_schema!(6; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f));
impl_tuple_schema!(7; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f), (6, G, _g));
impl_tuple_schema!(8; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f), (6, G, _g), (7, H, _h));
impl_tuple_schema!(9; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f), (6, G, _g), (7, H, _h), (8, I, _i));
impl_tuple_schema!(10; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f), (6, G, _g), (7, H, _h), (8, I, _i), (9, J, _j));
impl_tuple_schema!(11; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f), (6, G, _g), (7, H, _h), (8, I, _i), (9, J, _j), (10, K, _k));
impl_tuple_schema!(12; (0, A, _a), (1, B, _b), (2, C, _c), (3, D, _d), (4, E, _e), (5, F, _f), (6, G, _g), (7, H, _h), (8, I, _i), (9, J, _j), (10, K, _k), (11, L, _l));

// Tuples parse a fixed-length JSON array.
#[cfg(feature = "openapi")]
macro_rules! tuple_schemas {
    ($( $n:literal; $( ($idx:tt, $T:ident) ),+ );* $(;)?) => {$(
        impl<$($T: VldSchema + crate::openapi::JsonSchema),+> crate::openapi::JsonSchema for ($($T,)+) {
            fn json_schema(&self) -> serde_json::Value {
                serde_json::json!({
                    "type": "array",
                    "prefixItems": [ $( self.$idx.json_schema() ),+ ],
                    "items": false,
                    "minItems": $n,
                    "maxItems": $n,
                })
            }
        }

        impl<$($T: VldSchema + crate::openapi::CollectNestedSchemas),+> crate::openapi::CollectNestedSchemas for ($($T,)+) {
            fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
                $( self.$idx.collect_nested_schemas(out); )+
            }
        }
    )*};
}

#[cfg(feature = "openapi")]
tuple_schemas! {
    1; (0, A);
    2; (0, A), (1, B);
    3; (0, A), (1, B), (2, C);
    4; (0, A), (1, B), (2, C), (3, D);
    5; (0, A), (1, B), (2, C), (3, D), (4, E);
    6; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F);
    7; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G);
    8; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H);
    9; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I);
    10; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J);
    11; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K);
    12; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K), (11, L);
}
