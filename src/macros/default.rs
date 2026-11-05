/// Generate `impl Default` for a struct created by [`schema!`](crate::schema!).
///
/// Every listed field is set to **its type's** `Default::default()` — the same
/// as `#[derive(Default)]`. Schema defaults (`.with_default(...)`) are **not**
/// used; to get them, parse an empty object instead:
/// `Config::parse_value(&serde_json::json!({}))`.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct Config {
///         pub host: String => vld::string().with_default("localhost".into()),
///         pub port: Option<i64> => vld::number().int().optional(),
///         pub tags: Vec<String> => vld::array(vld::string()).with_default(vec![]),
///     }
/// }
///
/// vld::impl_default!(Config { host, port, tags });
///
/// let cfg = Config::default();
/// assert_eq!(cfg.host, "");       // String::default(), NOT the schema default
/// assert_eq!(cfg.port, None);     // Option::default()
/// assert!(cfg.tags.is_empty());   // Vec::default()
///
/// // Schema defaults come from parsing:
/// let parsed = Config::parse_value(&serde_json::json!({})).unwrap();
/// assert_eq!(parsed.host, "localhost");
/// ```
#[macro_export]
macro_rules! impl_default {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl ::std::default::Default for $name {
            fn default() -> Self {
                Self {
                    $( $field: ::std::default::Default::default(), )*
                }
            }
        }
    };
}
