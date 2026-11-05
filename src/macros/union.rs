/// Create a union schema from 2 or more schemas.
///
/// Dispatches to `vld::union()` for 2 schemas and `vld::union3()` for 3.
/// For 4+ schemas, unions are nested automatically (any number of schemas;
/// order and first-match semantics are preserved).
///
/// # Examples
///
/// ```rust
/// use vld::prelude::*;
///
/// // 2 schemas
/// let s = vld::union!(vld::string(), vld::number().int());
/// assert!(s.parse(r#""hello""#).is_ok());
/// assert!(s.parse("42").is_ok());
///
/// // 3 schemas
/// let s = vld::union!(vld::string(), vld::number().int(), vld::boolean());
/// assert!(s.parse("true").is_ok());
/// ```
#[macro_export]
macro_rules! union {
    // 2 schemas
    ($a:expr, $b:expr $(,)?) => {
        $crate::union($a, $b)
    };
    // 3 schemas
    ($a:expr, $b:expr, $c:expr $(,)?) => {
        $crate::union3($a, $b, $c)
    };
    // 4 schemas — nest as union(union(a, b), union(c, d))
    ($a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {
        $crate::union($crate::union($a, $b), $crate::union($c, $d))
    };
    // 5 schemas
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr $(,)?) => {
        $crate::union($crate::union3($a, $b, $c), $crate::union($d, $e))
    };
    // 6 schemas
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr $(,)?) => {
        $crate::union($crate::union3($a, $b, $c), $crate::union3($d, $e, $f))
    };
    // 7+ schemas — first six, then the rest recursively
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $($rest:expr),+ $(,)?) => {
        $crate::union(
            $crate::union!($a, $b, $c, $d, $e, $f),
            $crate::union!($($rest),+),
        )
    };
}
