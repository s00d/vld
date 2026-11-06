//! # vld-ts — TypeScript Zod / Valibot schemas from `vld`
//!
//! Generates [Zod](https://zod.dev/) 3 or [Valibot](https://valibot.dev/) 1
//! code that accepts the same values as a vld schema:
//!
//! - [`to_zod`] / [`to_valibot`] — an expression for any schema value;
//! - [`impl_to_zod!`] / [`impl_to_valibot!`] — `Type::to_zod()` /
//!   `Type::to_valibot()` and named exports ([`ToZod::to_zod_export`]) for a
//!   `vld::schema!` or `#[derive(Validate)]` type;
//! - [`to_openapi`] / [`impl_to_openapi!`] — the plain JSON Schema and its `$ref`s.
//!
//! ```
//! use vld_ts::{impl_to_valibot, impl_to_zod, to_zod, ToValibot, ToZod};
//!
//! vld::schema! {
//!     pub struct Address {
//!         pub city: String => vld::string().min(1),
//!     }
//! }
//!
//! vld::schema! {
//!     pub struct Order {
//!         pub shipping: Address => vld::nested!(Address),
//!     }
//! }
//!
//! impl_to_zod!(Address);
//! impl_to_zod!(Order);
//! impl_to_valibot!(Order);
//!
//! assert_eq!(to_zod(&vld::string().email()), "z.string().email()");
//! assert!(Order::to_zod().contains("shipping: z.lazy(() => AddressSchema)"));
//! assert!(Order::to_valibot().starts_with("v.object"));
//!
//! // A TypeScript module: exports for every type, the import is yours.
//! let module = format!(
//!     "import {{ z }} from \"zod\";\n\n{}\n{}",
//!     Address::to_zod_export(),
//!     Order::to_zod_export(),
//! );
//! assert!(module.contains("export type Order = z.infer<typeof OrderSchema>;"));
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/codegen/ts>

use serde_json::Value;

/// Convert a vld schema instance directly to Zod.
///
/// This is the high-level API for users who work with `vld` schemas and do not
/// want to manually call `.json_schema()` and pass raw JSON values around.
///
/// # Example
///
/// ```
/// use vld::openapi::JsonSchema as _;
/// use vld_ts::to_zod;
///
/// let schema = vld::string().min(2).email();
/// let ts = to_zod(&schema);
/// assert!(ts.contains("z.string()"));
/// assert!(ts.contains(".email()"));
/// ```
pub fn to_zod<S>(schema: &S) -> String
where
    S: vld::openapi::JsonSchema,
{
    convert(Zod, &schema.json_schema())
}

/// Convert a vld schema instance directly to Valibot.
///
/// # Example
///
/// ```
/// use vld_ts::to_valibot;
///
/// let schema = vld::string().min(2).email();
/// let ts = to_valibot(&schema);
/// assert!(ts.contains("v.string()"));
/// assert!(ts.contains("v.email()"));
/// ```
pub fn to_valibot<S>(schema: &S) -> String
where
    S: vld::openapi::JsonSchema,
{
    convert(Valibot, &schema.json_schema())
}

/// Convert a vld schema instance to plain JSON Schema (OpenAPI-compatible).
///
/// # Example
///
/// ```
/// use vld_ts::to_openapi;
///
/// let schema = vld::string().min(2).email();
/// let schema_json = to_openapi(&schema);
/// assert_eq!(schema_json["type"], "string");
/// ```
pub fn to_openapi<S>(schema: &S) -> Value
where
    S: vld::openapi::JsonSchema,
{
    schema.json_schema()
}

/// Collect all `$ref` paths from a schema object.
///
/// Designed to be used with [`to_openapi`].
///
/// # Example
///
/// ```
/// use vld_ts::openapi_refs;
///
/// let schema = serde_json::json!({
///     "type": "object",
///     "properties": {
///         "address": { "$ref": "#/components/schemas/Address" }
///     }
/// });
///
/// let refs = openapi_refs(&schema);
/// assert_eq!(refs, vec!["#/components/schemas/Address".to_string()]);
/// ```
pub fn openapi_refs(schema: &Value) -> Vec<String> {
    collect_ref_paths(schema)
}

/// Trait for types that can generate their Zod representation.
///
/// Implement with [`impl_to_zod!`], then call `Type::to_zod()`.
pub trait ToZod {
    /// Generate Zod schema expression for this type (no imports/exports).
    fn to_zod() -> String;

    /// Name used for exports (`export const <Name>Schema`).
    ///
    /// Defaults to the type name; `impl_to_zod!(Type, "Name")` overrides it.
    fn zod_name() -> String {
        short_type_name::<Self>()
    }

    /// `export const <Name>Schema = …;` plus `export type <Name> = z.infer<…>;`.
    ///
    /// `$ref`s to nested schemas become `z.lazy(() => <Nested>Schema)`, so export
    /// nested types as well (under their type name).
    fn to_zod_export() -> String {
        zod_export(&Self::zod_name(), &Self::to_zod())
    }
}

/// Trait for types that can generate their Valibot representation.
pub trait ToValibot {
    /// Generate Valibot schema expression for this type (no imports/exports).
    fn to_valibot() -> String;

    /// Name used for exports (`export const <Name>Schema`).
    ///
    /// Defaults to the type name; `impl_to_valibot!(Type, "Name")` overrides it.
    fn valibot_name() -> String {
        short_type_name::<Self>()
    }

    /// `export const <Name>Schema = …;` plus `export type <Name> = v.InferOutput<…>;`.
    fn to_valibot_export() -> String {
        valibot_export(&Self::valibot_name(), &Self::to_valibot())
    }
}

/// Render a named Zod export:
/// `export const <name>Schema = <expr>;\nexport type <name> = z.infer<typeof <name>Schema>;\n`.
///
/// ```
/// let ts = vld_ts::zod_export("Email", &vld_ts::to_zod(&vld::string().email()));
/// assert!(ts.starts_with("export const EmailSchema = z.string().email();"));
/// assert!(ts.contains("export type Email = z.infer<typeof EmailSchema>;"));
/// ```
pub fn zod_export(name: &str, expr: &str) -> String {
    format!(
        "export const {0}Schema = {1};\nexport type {0} = z.infer<typeof {0}Schema>;\n",
        name, expr
    )
}

/// Render a named Valibot export (see [`zod_export`]).
pub fn valibot_export(name: &str, expr: &str) -> String {
    format!(
        "export const {0}Schema = {1};\nexport type {0} = v.InferOutput<typeof {0}Schema>;\n",
        name, expr
    )
}

/// `my::mod::User<T>` → `User`.
fn short_type_name<T: ?Sized>() -> String {
    let full = std::any::type_name::<T>();
    let base = full.split('<').next().unwrap_or(full);
    base.rsplit("::").next().unwrap_or(base).to_string()
}

/// Trait for types that can generate plain JSON Schema.
pub trait ToOpenApi {
    /// Generate plain JSON Schema for this type.
    fn to_openapi() -> Value;
}

/// Trait for types that can return discovered `$ref` paths.
pub trait ToRefs {
    fn to_refs() -> Vec<String>;
}

/// Implement [`ToZod`] for a `vld::schema!` type.
///
/// `impl_to_zod!(Type, "Name")` sets the export name used by
/// [`ToZod::to_zod_export`] (default: the type name). Nested `$ref`s always
/// point at `<TypeName>Schema`, so rename only top-level (non-nested) types.
///
/// # Example
///
/// ```
/// use vld_ts::{impl_to_zod, ToZod};
///
/// vld::schema! {
///     pub struct Address {
///         pub city: String => vld::string().min(1),
///     }
/// }
/// vld::schema! {
///     pub struct Order {
///         pub shipping: Address => vld::nested!(Address),
///     }
/// }
///
/// impl_to_zod!(Address);
/// impl_to_zod!(Order, "OrderBody");
///
/// let ts = Order::to_zod();
/// assert!(ts.starts_with("z.object("));
///
/// let module = format!(
///     "import {{ z }} from \"zod\";\n\n{}\n{}",
///     Address::to_zod_export(),
///     Order::to_zod_export(),
/// );
/// assert!(module.contains("export const AddressSchema = z.object("));
/// assert!(module.contains("export const OrderBodySchema = z.object("));
/// assert!(module.contains("export type OrderBody = z.infer<typeof OrderBodySchema>;"));
/// ```
#[macro_export]
macro_rules! impl_to_zod {
    ($ty:ty $(, $name:expr)?) => {
        impl $crate::ToZod for $ty {
            fn to_zod() -> ::std::string::String {
                $crate::__to_zod_from_value(&<$ty>::json_schema())
            }
            $(
                fn zod_name() -> ::std::string::String {
                    ::std::string::String::from($name)
                }
            )?
        }
    };
}

/// Implement [`ToOpenApi`] for a `vld::schema!` type.
///
/// # Example
///
/// ```
/// use vld_ts::{impl_to_openapi, ToOpenApi};
///
/// vld::schema! {
///     pub struct User {
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// impl_to_openapi!(User);
/// let schema_json = User::to_openapi();
/// assert_eq!(schema_json["type"], "object");
/// ```
#[macro_export]
macro_rules! impl_to_openapi {
    // The optional name is accepted for symmetry with `impl_to_zod!`; the JSON
    // Schema itself carries no name.
    ($ty:ty $(, $name:expr)?) => {
        impl $crate::ToOpenApi for $ty {
            fn to_openapi() -> ::serde_json::Value {
                <$ty>::json_schema()
            }
        }
        impl $crate::ToRefs for $ty {
            fn to_refs() -> ::std::vec::Vec<::std::string::String> {
                $crate::openapi_refs(&<$ty>::json_schema())
            }
        }
    };
}

/// Implement [`ToValibot`] for a `vld::schema!` type.
///
/// # Example
///
/// ```
/// use vld_ts::{impl_to_valibot, ToValibot};
///
/// vld::schema! {
///     pub struct User {
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// impl_to_valibot!(User);
/// let ts = User::to_valibot();
/// assert!(ts.starts_with("v.object"));
/// ```
#[macro_export]
macro_rules! impl_to_valibot {
    ($ty:ty $(, $name:expr)?) => {
        impl $crate::ToValibot for $ty {
            fn to_valibot() -> ::std::string::String {
                $crate::__to_valibot_from_value(&<$ty>::json_schema())
            }
            $(
                fn valibot_name() -> ::std::string::String {
                    ::std::string::String::from($name)
                }
            )?
        }
    };
}

/// Output dialect of the shared JSON Schema walker.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Zod,
    Valibot,
}

use Target::{Valibot, Zod};

/// One generated validation step.
///
/// `Check`s become `.refine(...)` (Zod) / `v.check(...)` (Valibot) and are
/// always attached last: a Zod refinement returns `ZodEffects`, which has no
/// `.min()` / `.regex()` etc.
enum Act {
    Plain(String),
    /// `(fn, message)`.
    Check(String, String),
    /// A `/* … */` note on the base expression (something not expressible in JS).
    Note(String),
}

fn plain(s: impl Into<String>) -> Act {
    Act::Plain(s.into())
}

fn check(f: &str, msg: &str) -> Act {
    Act::Check(f.to_string(), msg.to_string())
}

/// `^…$` shape of an ISO time `HH:MM[:SS[.frac]]` (vld's `iso_time`).
const TIME_RE: &str = r"([01]\d|2[0-3]):[0-5]\d(:[0-5]\d(\.\d{1,9})?)?";
/// Time part of `iso_datetime()` / `datetime()`: seconds are required.
const DATETIME_TIME_RE: &str = r"([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?";

/// The date part (first 10 chars) is a real calendar day.
const CALENDAR_FN: &str = "(s) => { const d = new Date(s.slice(0, 10) + \"T00:00:00Z\"); return !isNaN(d) && d.toISOString().slice(0, 10) === s.slice(0, 10); }";

const LUHN_FN: &str = "(s) => { if (!/^[\\d -]*$/.test(s)) return false; const d = s.replace(/\\D/g, \"\"); if (d.length < 12 || d.length > 19) return false; let sum = 0; for (let i = 0; i < d.length; i++) { let n = +d[d.length - 1 - i]; if (i % 2) { n *= 2; if (n > 9) n -= 9; } sum += n; } return sum % 10 === 0; }";

const JSON_FN: &str = "(s) => { try { JSON.parse(s); return true; } catch { return false; } }";

/// Hostname labels: 1-63 alphanumerics/`-`, no leading/trailing `-`.
const LABEL_RE: &str = "[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?";

/// Actions for a `format` keyword, mirroring vld's own predicate as closely as
/// Zod 3 / Valibot 1 allow. Unknown formats (`decimal`, `int32`, `path`, …)
/// add nothing.
fn format_acts(t: Target, fmt: &str) -> Vec<Act> {
    let both = |zod: &str, valibot: &str| match t {
        Zod => plain(zod),
        Valibot => plain(valibot),
    };
    let re = |r: String| plain(format!("regex({})", r));
    let host = format!("(?=.{{1,253}}$){0}(\\.{0})*", LABEL_RE);
    let http = || re("/^https?:\\/\\/\\S+$/".to_string());
    let uuid_v = |n: u8| {
        vec![
            both("uuid()", "uuid()"),
            re(format!("/^[0-9a-fA-F]{{8}}-[0-9a-fA-F]{{4}}-{}/", n)),
        ]
    };
    let is_ip = |n: u8| match t {
        Zod => format!(
            "z.string().ip({{ version: \"v{}\" }}).safeParse(a).success",
            n
        ),
        Valibot => format!("v.safeParse(v.pipe(v.string(), v.ipv{}()), a).success", n),
    };
    match fmt {
        // Zod/Valibot `email()` are stricter than vld (they need a dot in the domain).
        "email" => vec![both("email()", "email()")],
        // vld `url()` accepts http/https only; `uri()` (string-advanced) shares
        // the `uri` format and is generated as an http(s) URL too.
        "uri" | "url" | "url-strict" => vec![both("url()", "url()"), http()],
        "uuid" => vec![both("uuid()", "uuid()")],
        "uuid-v1" => uuid_v(1),
        "uuid-v4" => uuid_v(4),
        "uuid-v7" => uuid_v(7),
        "ipv4" => vec![both("ip({ version: \"v4\" })", "ipv4()")],
        "ipv6" => vec![both("ip({ version: \"v6\" })", "ipv6()")],
        "ip" => vec![both("ip()", "ip()")],
        "cidr" => vec![check(
            &format!(
                "(s) => {{ const i = s.indexOf(\"/\"); if (i < 0) return false; const a = s.slice(0, i), p = s.slice(i + 1); if (!/^(0|[1-9]\\d{{0,2}})$/.test(p)) return false; return {} ? +p <= 32 : {} && +p <= 128; }}",
                is_ip(4),
                is_ip(6)
            ),
            "Invalid CIDR",
        )],
        "date" => vec![
            re("/^\\d{4}-\\d{2}-\\d{2}$/".to_string()),
            check(CALENDAR_FN, "Invalid date"),
        ],
        "date-time" => vec![
            re(format!(
                "/^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]{}([Zz]|[+-]([01]\\d|2[0-3]):[0-5]\\d)?$/",
                DATETIME_TIME_RE
            )),
            check(CALENDAR_FN, "Invalid datetime"),
        ],
        "time" => vec![re(format!("/^{}$/", TIME_RE))],
        "hostname" => vec![re(format!("/^{}$/", host))],
        "mac" => vec![re("/^[0-9A-Fa-f]{2}(:[0-9A-Fa-f]{2}){5}$/".to_string())],
        "hex" => vec![re("/^[0-9A-Fa-f]+$/".to_string())],
        "slug" => vec![re("/^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/".to_string())],
        "semver" => vec![re(
            "/^v?\\d+\\.\\d+\\.\\d+(?:[-+][\\s\\S]*)?$/".to_string(),
        )],
        "jwt" => vec![re(
            "/^[A-Za-z0-9_-]+\\.[A-Za-z0-9_-]+\\.[A-Za-z0-9_-]+$/".to_string(),
        )],
        "ascii" => vec![re("/^[\\x00-\\x7F]*$/".to_string())],
        "alpha" => vec![re("/^[A-Za-z]*$/".to_string())],
        "alphanumeric" => vec![re("/^[A-Za-z0-9]*$/".to_string())],
        "lowercase" => vec![check("(s) => s === s.toLowerCase()", "Must be lowercase")],
        "uppercase" => vec![check("(s) => s === s.toUpperCase()", "Must be uppercase")],
        "locale" => vec![re("/^[a-z]{2}(-[A-Z]{2})?$/".to_string())],
        "country-code" => vec![re("/^[A-Z]{2}$/".to_string())],
        "currency-code" => vec![re("/^[A-Z]{3}$/".to_string())],
        "cron" => vec![re(
            "/^\\s*[\\d*\\/,?-]+(\\s+[\\d*\\/,?-]+){4,5}\\s*$/".to_string(),
        )],
        "credit-card" => vec![check(LUHN_FN, "Invalid credit card")],
        "phone" => vec![re(
            "/^\\+(?=(?:\\D*\\d){8,15}\\D*$)[\\d ()-]*$/".to_string(),
        )],
        // Shape only: vld's strict E.164 check uses libphonenumber metadata.
        "phone-e164" => vec![re("/^\\+[1-9]\\d{1,14}$/".to_string())],
        "color" => {
            let n8 = "\\s*\\+?0*(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)\\s*";
            let pct = "\\s*\\+?0*(?:100|[1-9]?\\d)%\\s*";
            let hue = "\\s*\\+?0*(?:360|3[0-5]\\d|[12]?\\d?\\d)\\s*";
            vec![re(format!(
                "/^(?:#(?:[0-9a-f]{{6}}|[0-9a-f]{{8}})|rgb\\({0},{0},{0}\\)|hsl\\({1},{2},{2}\\))$/i",
                n8, hue, pct
            ))]
        }
        "byte" => vec![re(
            "/^(?:[A-Za-z0-9+\\/]{4})*(?:[A-Za-z0-9+\\/]{4}|[A-Za-z0-9+\\/]{2}==|[A-Za-z0-9+\\/]{3}=)$/"
                .to_string(),
        )],
        "base64url" => vec![re(
            "/^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-]{4}|[A-Za-z0-9_-]{2}(?:==?)?|[A-Za-z0-9_-]{3}=?)$/"
                .to_string(),
        )],
        "json" => vec![check(JSON_FN, "Invalid JSON")],
        "cuid2" => vec![re("/^[a-z][a-z0-9]{1,31}$/".to_string())],
        "ulid" => vec![re("/^[0-7][0-9A-HJKMNP-TV-Z]{25}$/i".to_string())],
        "nanoid" => vec![re("/^[A-Za-z0-9_-]+$/".to_string())],
        "emoji" => vec![re(
            "/^[\\u{1F600}-\\u{1F64F}\\u{1F300}-\\u{1F5FF}\\u{1F680}-\\u{1F6FF}\\u{1F1E0}-\\u{1F1FF}\\u{2702}-\\u{27B0}\\u{2600}-\\u{26FF}\\u{FE00}-\\u{FE0F}\\u{1F900}-\\u{1F9FF}\\u{1FA00}-\\u{1FAFF}\\u{231A}-\\u{231B}\\u{23E9}-\\u{23F3}\\u{23F8}-\\u{23FA}\\u{E0020}-\\u{E007F}\\u{20E3}\\u{200D}\\u{2B50}]+$/u"
                .to_string(),
        )],
        _ => vec![],
    }
}

/// Translate a Rust (`regex`/`regex-lite`) pattern into `new RegExp(source, flags)`
/// arguments.
///
/// Leading inline flags `(?i)`, `(?m)`, `(?s)` become RegExp flags, `\A` / `\z`
/// become `^` / `$` and `(?P<name>…)` becomes `(?<name>…)`. Constructs JS has no
/// equivalent for (mid-pattern / scoped flags, nested classes and class set
/// operations, `\p{…}`, `\<`, `\b{start}`, `\x{…}`, a leading `]` in a class, …)
/// return `None`.
fn js_regex(p: &str) -> Option<(String, String)> {
    let mut flags = String::new();
    let mut rest = p;
    while let Some(r) = rest.strip_prefix("(?") {
        let Some(end) = r.find(')') else { break };
        let f = &r[..end];
        if f.is_empty() || !f.chars().all(|c| matches!(c, 'i' | 'm' | 's')) {
            break;
        }
        for c in f.chars() {
            if !flags.contains(c) {
                flags.push(c);
            }
        }
        rest = &r[end + 1..];
    }

    let mut out = String::with_capacity(rest.len());
    let mut chars = rest.chars().peekable();
    let mut in_class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let n = chars.next()?;
                match n {
                    'A' if !in_class => out.push('^'),
                    'z' if !in_class => out.push('$'),
                    'b' | 'x' | 'u' if chars.peek() == Some(&'{') => return None,
                    'd' | 'D' | 'w' | 'W' | 's' | 'S' | 'b' | 'B' | 'n' | 't' | 'r' | 'f' | 'v'
                    | 'x' | 'u' => {
                        out.push('\\');
                        out.push(n);
                    }
                    n if n.is_ascii_alphanumeric() || n == '<' || n == '>' => return None,
                    n => {
                        out.push('\\');
                        out.push(n);
                    }
                }
            }
            '[' if in_class => return None,
            '[' => {
                in_class = true;
                out.push('[');
                if chars.peek() == Some(&'^') {
                    out.push(chars.next()?);
                }
                // Rust reads a leading `]` as a literal; JS `[]` is an empty class.
                if chars.peek() == Some(&']') {
                    return None;
                }
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            '&' | '-' | '~' if in_class && chars.peek() == Some(&c) => return None,
            '(' if !in_class && chars.peek() == Some(&'?') => {
                chars.next();
                match chars.next()? {
                    ':' => out.push_str("(?:"),
                    'P' if chars.peek() == Some(&'<') => {
                        chars.next();
                        out.push_str("(?<");
                    }
                    '<' => out.push_str("(?<"),
                    _ => return None,
                }
            }
            c => out.push(c),
        }
    }
    (!in_class).then_some((out, flags))
}

/// `regex(new RegExp(…))` for a portable pattern, a note otherwise.
fn pattern_act(p: &str) -> Act {
    match js_regex(p) {
        Some((src, flags)) if flags.is_empty() => {
            plain(format!("regex(new RegExp({}))", js_str(&src)))
        }
        Some((src, flags)) => plain(format!(
            "regex(new RegExp({}, {}))",
            js_str(&src),
            js_str(&flags)
        )),
        None => Act::Note(format!(
            "pattern not portable to JS, not checked: {}",
            js_str(p)
        )),
    }
}

impl Target {
    /// Library namespace: `z` or `v`.
    fn ns(self) -> &'static str {
        match self {
            Zod => "z",
            Valibot => "v",
        }
    }

    /// A validation/metadata action call, e.g. `min(2)` (Zod) / `minLength(2)` (Valibot).
    fn act(self, zod: &str, valibot: &str, arg: impl std::fmt::Display) -> Act {
        match self {
            Zod => plain(format!("{}({})", zod, arg)),
            Valibot => plain(format!("{}({})", valibot, arg)),
        }
    }

    /// Attach actions to a base schema: method chain (Zod) or `v.pipe` (Valibot).
    /// Plain actions keep their order; checks go last, notes onto the base.
    fn pipe(self, mut base: String, acts: Vec<Act>) -> String {
        let mut actions = Vec::new();
        let mut checks = Vec::new();
        for a in acts {
            match a {
                Act::Plain(s) => actions.push(s),
                Act::Check(f, msg) => checks.push(match self {
                    Zod => format!("refine({}, {})", f, js_str(&msg)),
                    Valibot => format!("check({}, {})", f, js_str(&msg)),
                }),
                Act::Note(n) => {
                    base.push_str(&format!(" /* {} */", n.replace("*/", "*\\/")));
                }
            }
        }
        actions.extend(checks);
        if actions.is_empty() {
            return base;
        }
        match self {
            Zod => actions.iter().fold(base, |acc, a| format!("{}.{}", acc, a)),
            Valibot => {
                let actions: Vec<String> = actions.iter().map(|a| format!("v.{}", a)).collect();
                format!("v.pipe({}, {})", base, actions.join(", "))
            }
        }
    }

    fn nullable(self, e: String) -> String {
        match self {
            Zod => format!("{}.nullable()", e),
            Valibot => format!("v.nullable({})", e),
        }
    }

    fn optional(self, e: String) -> String {
        match self {
            Zod => format!("{}.optional()", e),
            Valibot => format!("v.optional({})", e),
        }
    }

    /// `default` value: JSON is valid JS expression syntax.
    ///
    /// Like vld, both `undefined` and `null` take the default (zod's
    /// `.default()` / valibot's `v.optional(x, d)` would reject `null`).
    fn with_default(self, e: String, default: &Value) -> String {
        match self {
            Zod => format!("{}.nullish().transform((v) => v ?? {})", e, default),
            Valibot => format!("v.nullish({}, {})", e, default),
        }
    }

    fn union(self, mut variants: Vec<String>) -> String {
        if variants.len() == 1 {
            return variants.remove(0);
        }
        format!("{}.union([{}])", self.ns(), variants.join(", "))
    }

    fn intersection(self, mut parts: Vec<String>) -> String {
        match (self, parts.len()) {
            (_, 0) => format!("{}.unknown()", self.ns()),
            (_, 1) => parts.remove(0),
            (Zod, _) => parts[1..].iter().fold(parts[0].clone(), |acc, p| {
                format!("z.intersection({}, {})", acc, p)
            }),
            (Valibot, _) => format!("v.intersect([{}])", parts.join(", ")),
        }
    }

    fn literal(self, v: &Value) -> String {
        let ns = self.ns();
        match v {
            Value::Null => format!("{}.null()", ns),
            // JSON scalars are valid (escaped) JS literals.
            Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                format!("{}.literal({})", ns, v)
            }
            _ => format!("{}.unknown()", ns),
        }
    }
}

/// JSON string literal — always a valid JS string literal.
fn js_str(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

/// Object key: bare if it is a plain JS identifier, quoted otherwise.
fn js_key(k: &str) -> String {
    let mut chars = k.chars();
    let ident = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if ident {
        k.to_string()
    } else {
        js_str(k)
    }
}

#[doc(hidden)]
pub fn __to_zod_from_value(schema: &Value) -> String {
    convert(Zod, schema)
}

#[doc(hidden)]
pub fn __to_valibot_from_value(schema: &Value) -> String {
    convert(Valibot, schema)
}

/// Shared JSON Schema → Zod/Valibot walker.
///
/// `description` and `default` are handled here for every schema kind, so they
/// survive on unions, refs, enums, booleans etc.
fn convert(t: Target, schema: &Value) -> String {
    let (base, mut actions) = convert_core(t, schema);
    if let Some(desc) = schema.get("description").and_then(Value::as_str) {
        actions.push(t.act("describe", "description", js_str(desc)));
    }
    let e = t.pipe(base, actions);
    match schema.get("default") {
        Some(d) => t.with_default(e, d),
        None => e,
    }
}

/// Keywords that make a schema more than a bag of constraints.
const STRUCTURAL: &[&str] = &[
    "type",
    "$ref",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "properties",
    "items",
    "prefixItems",
    "enum",
    "const",
];

/// Base expression plus the actions to attach to it (no description/default).
fn convert_core(t: Target, schema: &Value) -> (String, Vec<Act>) {
    let ns = t.ns();

    match schema {
        Value::Bool(true) => return (format!("{}.unknown()", ns), vec![]),
        Value::Bool(false) => return (format!("{}.never()", ns), vec![]),
        _ => {}
    }

    if let Some(name) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(extract_ref_name)
    {
        return (format!("{}.lazy(() => {}Schema)", ns, name), vec![]);
    }

    // `{"not": {}}` / `{"not": true}`: nothing is valid (`vld::never()`).
    if matches!(schema.get("not"), Some(Value::Bool(true)))
        || schema
            .get("not")
            .and_then(Value::as_object)
            .is_some_and(|o| o.is_empty())
    {
        return (format!("{}.never()", ns), vec![]);
    }

    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        let mut rest = schema.clone();
        if let Some(o) = rest.as_object_mut() {
            o.remove("allOf");
            o.remove("description");
            o.remove("default");
        }
        // A plain `{"allOf": [a, b]}` is an intersection.
        if !rest.as_object().is_some_and(|o| !o.is_empty()) {
            return (
                t.intersection(all_of.iter().map(|v| convert(t, v)).collect()),
                vec![],
            );
        }
        // Core puts extra patterns / formats / multipleOf into `allOf` next to
        // `type`: keep the typed base and add those as further checks.
        let (base, mut acts) = convert_core(t, &rest);
        let ty = rest.get("type").and_then(Value::as_str);
        let mut others = Vec::new();
        for part in all_of {
            let fragment = part
                .as_object()
                .is_some_and(|o| !STRUCTURAL.iter().any(|k| o.contains_key(*k)));
            match ty {
                Some(ty) if fragment => acts.extend(constraint_acts(t, ty, part)),
                _ => others.push(convert(t, part)),
            }
        }
        if others.is_empty() {
            return (base, acts);
        }
        others.insert(0, t.pipe(base, acts));
        return (t.intersection(others), vec![]);
    }

    // Core may emit unions/nullables as either `oneOf` or `anyOf`.
    for key in ["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(key).and_then(Value::as_array) {
            let is_null = |v: &Value| v.get("type").and_then(Value::as_str) == Some("null");
            // Sibling keywords (`type`, `x-past`, … next to e.g. vld's
            // naive-datetime `anyOf`) apply to every branch.
            let mut siblings = schema.as_object().cloned().unwrap_or_default();
            for k in [key, "description", "default", "title"] {
                siblings.remove(k);
            }
            let rest: Vec<String> = variants
                .iter()
                .filter(|v| !is_null(v))
                .map(|v| match v.as_object() {
                    Some(o) if !siblings.is_empty() => {
                        let mut m = siblings.clone();
                        m.extend(o.clone());
                        convert(t, &Value::Object(m))
                    }
                    _ => convert(t, v),
                })
                .collect();
            let has_null = variants.iter().any(is_null);
            return (union_nullable(t, rest, has_null), vec![]);
        }
    }

    if let Some(c) = schema.get("const") {
        return (t.literal(c), vec![]);
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return (
            t.union(values.iter().map(|v| t.literal(v)).collect()),
            vec![],
        );
    }

    match schema.get("type") {
        Some(Value::String(ty)) => convert_typed(t, ty, schema),
        // `type: ["string", "null"]` → nullable string, `["string", "integer"]` → union
        Some(Value::Array(types)) => {
            let mut has_null = false;
            let mut rest = Vec::new();
            for ty in types.iter().filter_map(Value::as_str) {
                if ty == "null" {
                    has_null = true;
                } else {
                    let (base, actions) = convert_typed(t, ty, schema);
                    rest.push(t.pipe(base, actions));
                }
            }
            (union_nullable(t, rest, has_null), vec![])
        }
        _ => (format!("{}.unknown()", ns), vec![]),
    }
}

fn union_nullable(t: Target, variants: Vec<String>, nullable: bool) -> String {
    match (variants.is_empty(), nullable) {
        (true, true) => format!("{}.null()", t.ns()),
        (true, false) => format!("{}.never()", t.ns()),
        (false, true) => t.nullable(t.union(variants)),
        (false, false) => t.union(variants),
    }
}

fn convert_typed(t: Target, ty: &str, schema: &Value) -> (String, Vec<Act>) {
    let ns = t.ns();
    let mut acts = Vec::new();
    let base = match ty {
        "string" => format!("{}.string()", ns),
        "number" => format!("{}.number()", ns),
        "integer" => {
            acts.push(t.act("int", "integer", ""));
            format!("{}.number()", ns)
        }
        "boolean" => format!("{}.boolean()", ns),
        "null" => format!("{}.null()", ns),
        "array" => match schema.get("prefixItems").and_then(Value::as_array) {
            Some(prefix) => convert_tuple(t, prefix, schema.get("items")),
            None => {
                let items = schema
                    .get("items")
                    .map(|i| convert(t, i))
                    .unwrap_or_else(|| format!("{}.unknown()", ns));
                format!("{}.array({})", ns, items)
            }
        },
        "object" => convert_object(t, schema),
        _ => return (format!("{}.unknown()", ns), acts),
    };
    acts.extend(constraint_acts(t, ty, schema));
    (base, acts)
}

/// `z.tuple([...])` / `v.strictTuple([...])`, with a rest schema unless
/// `items: false`.
fn convert_tuple(t: Target, prefix: &[Value], items: Option<&Value>) -> String {
    let parts = prefix
        .iter()
        .map(|p| convert(t, p))
        .collect::<Vec<_>>()
        .join(", ");
    let rest = match items {
        Some(Value::Bool(false)) => None,
        Some(i @ Value::Object(_)) => Some(convert(t, i)),
        _ => Some(format!("{}.unknown()", t.ns())),
    };
    match (t, rest) {
        (Zod, None) => format!("z.tuple([{}])", parts),
        (Zod, Some(r)) => format!("z.tuple([{}]).rest({})", parts, r),
        (Valibot, None) => format!("v.strictTuple([{}])", parts),
        (Valibot, Some(r)) => format!("v.tupleWithRest([{}], {})", parts, r),
    }
}

/// JS predicate "`x` equals the JSON value `c`" (deep for arrays/objects).
fn js_eq(c: &Value) -> String {
    match c {
        Value::Object(_) | Value::Array(_) => {
            format!("JSON.stringify(x) === {}", js_str(&c.to_string()))
        }
        _ => format!("x === {}", c),
    }
}

/// Distinct-item count of array `a` (JSON identity).
const DISTINCT: &str = "new Set(a.map((x) => JSON.stringify(x))).size";

/// Constraint keywords of a `ty`-typed schema (or an `allOf` fragment of one).
fn constraint_acts(t: Target, ty: &str, schema: &Value) -> Vec<Act> {
    let u64_of = |k: &str| schema.get(k).and_then(Value::as_u64);
    let mut acts = Vec::new();
    match ty {
        "string" => {
            if let Some(n) = u64_of("minLength") {
                acts.push(t.act("min", "minLength", n));
            }
            if let Some(n) = u64_of("maxLength") {
                acts.push(t.act("max", "maxLength", n));
            }
            if let Some(fmt) = schema.get("format").and_then(Value::as_str) {
                acts.extend(format_acts(t, fmt));
            }
            if let Some(p) = schema.get("pattern").and_then(Value::as_str) {
                acts.push(pattern_act(p));
            }
            let fmt = schema.get("format").and_then(Value::as_str);
            for (key, op) in [("formatMinimum", ">="), ("formatMaximum", "<=")] {
                if let Some(b) = schema.get(key).and_then(Value::as_str) {
                    acts.push(match fmt {
                        // Offsets make lexical order wrong for date-times.
                        Some("date-time") => check(
                            &format!("(s) => Date.parse(s) {} Date.parse({})", op, js_str(b)),
                            &format!("Must be {} {}", op, b),
                        ),
                        _ => check(
                            &format!("(s) => s {} {}", op, js_str(b)),
                            &format!("Must be {} {}", op, b),
                        ),
                    });
                }
            }
            for (key, op, word) in [("x-past", "<", "past"), ("x-future", ">", "future")] {
                if schema.get(key).and_then(Value::as_bool) == Some(true) {
                    acts.push(check(
                        &format!("(s) => Date.parse(s) {} Date.now()", op),
                        &format!("Must be in the {}", word),
                    ));
                }
            }
        }
        "number" | "integer" => {
            for (key, zod, valibot) in [
                ("minimum", "min", "minValue"),
                ("maximum", "max", "maxValue"),
                ("exclusiveMinimum", "gt", "gtValue"),
                ("exclusiveMaximum", "lt", "ltValue"),
                ("multipleOf", "multipleOf", "multipleOf"),
            ] {
                if let Some(n) = schema.get(key).and_then(Value::as_f64) {
                    acts.push(t.act(zod, valibot, format_number(n)));
                }
            }
        }
        "array" => {
            // A tuple's length is fixed by its items; Zod tuples have no `.min()`.
            if schema.get("prefixItems").is_none() {
                // Valibot: `minLength`/`maxLength` apply to arrays (`minSize` is for Set/Map/Blob).
                if let Some(n) = u64_of("minItems") {
                    acts.push(t.act("min", "minLength", n));
                }
                if let Some(n) = u64_of("maxItems") {
                    acts.push(t.act("max", "maxLength", n));
                }
            }
            if schema.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
                acts.push(check(
                    &format!("(a) => {} === a.length", DISTINCT),
                    "Items must be unique",
                ));
            }
            if let Some(n) = u64_of("x-minUniqueItems") {
                acts.push(check(
                    &format!("(a) => {} >= {}", DISTINCT, n),
                    &format!("At least {} unique items", n),
                ));
            }
            if let Some(n) = u64_of("x-maxUniqueItems") {
                acts.push(check(
                    &format!("(a) => {} <= {}", DISTINCT, n),
                    &format!("At most {} unique items", n),
                ));
            }
            if let Some(c) = schema.get("contains").and_then(|c| c.get("const")) {
                let min = u64_of("minContains").unwrap_or(1);
                let count = format!("a.filter((x) => {}).length", js_eq(c));
                let mut cond = format!("{} >= {}", count, min);
                if let Some(max) = u64_of("maxContains") {
                    cond = format!("(n => n >= {} && n <= {})({})", min, max, count);
                }
                acts.push(check(
                    &format!("(a) => {}", cond),
                    &format!("Must contain {}", c),
                ));
            }
        }
        "object" => {
            for (key, op, valibot) in [
                ("minProperties", ">=", "minEntries"),
                ("maxProperties", "<=", "maxEntries"),
            ] {
                if let Some(n) = u64_of(key) {
                    acts.push(match t {
                        Zod => check(
                            &format!("(o) => Object.keys(o).length {} {}", op, n),
                            &format!("Must have {} {} keys", op, n),
                        ),
                        Valibot => plain(format!("{}({})", valibot, n)),
                    });
                }
            }
        }
        _ => {}
    }
    acts
}

fn convert_object(t: Target, schema: &Value) -> String {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let additional = schema.get("additionalProperties");

    let props = match schema.get("properties").and_then(Value::as_object) {
        Some(p) => p,
        // Record-like / free-form object.
        None => {
            let names = schema.get("propertyNames");
            if names.is_some() || matches!(additional, Some(Value::Object(_))) {
                // `propertyNames` validates the keys (it is a string schema).
                let key = match names {
                    Some(n @ Value::Object(o)) if !o.contains_key("type") => {
                        let mut n = n.clone();
                        n["type"] = Value::from("string");
                        convert(t, &n)
                    }
                    Some(n) => convert(t, n),
                    None => format!("{}.string()", t.ns()),
                };
                let value = match additional {
                    Some(a @ Value::Object(_)) => convert(t, a),
                    _ => format!("{}.unknown()", t.ns()),
                };
                return format!("{}.record({}, {})", t.ns(), key, value);
            }
            return match (t, additional) {
                (Zod, Some(Value::Bool(false))) => "z.object({}).strict()".to_string(),
                (Valibot, Some(Value::Bool(false))) => "v.strictObject({})".to_string(),
                (Zod, _) => "z.object({}).passthrough()".to_string(),
                (Valibot, _) => "v.objectWithRest({}, v.unknown())".to_string(),
            };
        }
    };

    let fields: Vec<String> = props
        .iter()
        .map(|(key, prop)| {
            let mut e = convert(t, prop);
            // A default already makes the field optional on input.
            if !required.contains(&key.as_str()) && prop.get("default").is_none() {
                e = t.optional(e);
            }
            format!("  {}: {}", js_key(key), e)
        })
        .collect();
    let entries = if fields.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{}\n}}", fields.join(",\n"))
    };

    // Unknown keys: `false` → reject, schema → validate as catch-all,
    // otherwise strip (default behaviour of both `z.object` and `v.object`).
    match (t, additional) {
        (Zod, Some(Value::Bool(false))) => format!("z.object({}).strict()", entries),
        (Zod, Some(a @ Value::Object(_))) => {
            format!("z.object({}).catchall({})", entries, convert(t, a))
        }
        (Zod, _) => format!("z.object({})", entries),
        (Valibot, Some(Value::Bool(false))) => format!("v.strictObject({})", entries),
        (Valibot, Some(a @ Value::Object(_))) => {
            format!("v.objectWithRest({}, {})", entries, convert(t, a))
        }
        (Valibot, _) => format!("v.object({})", entries),
    }
}

fn format_number(n: f64) -> String {
    if n == n.floor() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn extract_ref_name(ref_path: &str) -> Option<&str> {
    ref_path
        .strip_prefix("#/components/schemas/")
        .or_else(|| ref_path.strip_prefix("#/$defs/"))
        .or_else(|| ref_path.strip_prefix("#/definitions/"))
}

fn collect_ref_paths(schema: &Value) -> Vec<String> {
    let mut out = std::collections::BTreeSet::new();
    collect_ref_paths_rec(schema, &mut out);
    out.into_iter().collect()
}

fn collect_ref_paths_rec(value: &Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(ref_path)) = map.get("$ref") {
                out.insert(ref_path.clone());
            }
            for v in map.values() {
                collect_ref_paths_rec(v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_ref_paths_rec(v, out);
            }
        }
        _ => {}
    }
}
