use serde_json::Value;

use crate::error::{value_type_name, IssueCode, PathSegment, VldError};
use crate::schema::VldSchema;

macro_rules! impl_tuple_schema {
    ($count:expr; $(($idx:tt, $T:ident, $v:ident)),+) => {
        impl<$($T: VldSchema),+> VldSchema for ($($T,)+) {
            type Output = ($($T::Output,)+);

            fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
                let arr = value.as_array().ok_or_else(|| {
                    VldError::single(
                        IssueCode::InvalidType {
                            expected: "array".to_string(),
                            received: value_type_name(value),
                        },
                        format!("Expected array (tuple), received {}", value_type_name(value)),
                    )
                })?;

                if arr.len() != $count {
                    return Err(VldError::single(
                        IssueCode::Custom { code: "invalid_tuple_length".to_string() },
                        format!("Expected tuple of {} elements, received {}", $count, arr.len()),
                    ));
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

                if !__errors.is_empty() {
                    return Err(__errors);
                }

                Ok(($($v.unwrap(),)+))
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
