use serde_json::Value;

use crate::error::{value_type_name, IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::VldSchema;

macro_rules! impl_tuple_schema {
    ($count:expr; $(($idx:tt, $T:ident, $v:ident)),+) => {
        impl<$($T: VldSchema),+> VldSchema for ($($T,)+) {
            type Output = ($($T::Output,)+);

            fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
                let arr = value.as_array().ok_or_else(|| {
                    VldError::from_code(IssueCode::InvalidType {
                            expected: "array".to_string(),
                            received: value_type_name(value),
                        })
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
}
