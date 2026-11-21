//! LLM structured outputs and tool calls: provider-ready JSON Schemas, JSON
//! extraction from model replies, and validation feedback a model can act on.
//!
//! - [`llm_schema`] / [`LlmSchema::llm_schema`] / [`LlmSchemaExt::llm_schema`]
//!   rewrite vld's JSON Schema for a [`Provider`]: `$ref`s point into `$defs`,
//!   `oneOf` becomes `anyOf`, nullable `anyOf` becomes a type array, keywords
//!   the provider rejects move into `description` as text, and provider
//!   limits are checked before the request is sent ([`LlmSchemaError`]).
//! - [`parse_llm`] finds the JSON in a reply (raw, fenced, or inside prose)
//!   and validates it.
//! - [`VldError::llm_feedback`](crate::error::VldError::llm_feedback) renders
//!   the issues as short lines for a retry prompt.
//!
//! ```
//! use vld::llm::{LlmSchemaExt, Provider, parse_llm_with};
//!
//! let schema = vld::object()
//!     .field("city", vld::string().min(2))
//!     .field("days", vld::number().int().min(1).max(14));
//! let js = schema.llm_schema(Provider::OpenAiStrict).unwrap();
//! assert_eq!(js["additionalProperties"], false);
//! assert_eq!(js["properties"]["city"]["description"], "min length 2");
//!
//! let err = parse_llm_with(&schema, "Sure! ```json\n{\"city\": \"X\", \"days\": 30}\n```")
//!     .unwrap_err();
//! assert_eq!(
//!     err.llm_feedback(),
//!     "city: expected string length >= 2, got \"X\"\ndays: expected number <= 14, got 30",
//! );
//! ```

use std::fmt;

use serde_json::Value;

use crate::openapi::{CollectNestedSchemas, JsonSchema, NestedSchemaEntry};

mod feedback;
mod parse;
mod schema;

pub use feedback::LlmFeedbackOptions;
pub use parse::{LlmParseError, extract_json, parse_llm, parse_llm_with};

/// The model API a schema is prepared for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Provider {
    /// OpenAI structured outputs / function calling with `strict: true`:
    /// object root, every property required (optional ones nullable),
    /// `additionalProperties: false` everywhere.
    OpenAiStrict,
    /// Anthropic strict tool use and structured outputs:
    /// `additionalProperties: false`, no numeric / length bounds, no recursion.
    Anthropic,
    /// Gemini `responseSchema` / `responseJsonSchema`.
    Gemini,
    /// Plain JSON Schema draft 2020-12 with `$defs` (MCP `inputSchema`,
    /// validators, other models). Keeps every keyword.
    Generic,
}

/// A schema the provider would reject, found before sending it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LlmSchemaError {
    /// A `$ref` whose target is neither the root nor a collected schema.
    UnresolvedRef {
        /// The `$ref` value.
        reference: String,
    },
    /// The provider needs an object at the root (OpenAI, Anthropic).
    RootNotObject,
    /// A keyword the provider does not support and that cannot become text
    /// without changing the accepted shape (`allOf`, `not`, `prefixItems`,
    /// a map's `additionalProperties` schema, …).
    Unsupported {
        /// JSON pointer of the schema (`#/properties/tags`).
        path: String,
        /// The keyword.
        keyword: String,
    },
    /// A schema with no `type` (an unconstrained value) where the provider
    /// needs one (OpenAI, Gemini).
    Untyped {
        /// JSON pointer of the schema.
        path: String,
    },
    /// A recursive schema where the provider does not allow recursion
    /// (Anthropic). `name` is the `$defs` entry, or `#` for the root.
    Recursive {
        /// The schema that references itself.
        name: String,
    },
    /// A provider limit is exceeded.
    TooMany {
        /// What was counted (`properties`, `nesting depth`, `enum values`,
        /// `name and enum characters`, `optional parameters`, `union parameters`).
        what: &'static str,
        /// The count in this schema.
        count: usize,
        /// The provider limit.
        max: usize,
    },
}

impl fmt::Display for LlmSchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedRef { reference } => write!(f, "unresolved $ref {reference}"),
            Self::RootNotObject => write!(f, "the root schema must be an object"),
            Self::Unsupported { path, keyword } => {
                write!(f, "`{keyword}` at {path} is not supported by the provider")
            }
            Self::Untyped { path } => write!(f, "schema at {path} has no type"),
            Self::Recursive { name } => write!(f, "recursive schema {name} is not supported"),
            Self::TooMany { what, count, max } => {
                write!(f, "too many {what}: {count} (limit {max})")
            }
        }
    }
}

impl std::error::Error for LlmSchemaError {}

/// Prepare a JSON Schema (from [`JsonSchema::json_schema`] or hand-written)
/// for `provider`.
///
/// `$ref`s must point into the schema's own `$defs`; for named nested types
/// use [`LlmSchema::llm_schema`] or [`LlmSchemaExt::llm_schema`], which
/// collect them.
pub fn llm_schema(schema: &Value, provider: Provider) -> Result<Value, LlmSchemaError> {
    llm_schema_with_defs(None, schema, &[], provider)
}

/// [`llm_schema`] with the named schemas `$ref: #/components/schemas/<name>`
/// points to. A reference to `root_name` becomes `#` (recursion), the others
/// `#/$defs/<name>`.
pub fn llm_schema_with_defs(
    root_name: Option<&str>,
    root: &Value,
    defs: &[(&str, Value)],
    provider: Provider,
) -> Result<Value, LlmSchemaError> {
    schema::convert(root_name, root, defs, provider)
}

fn from_entries(
    root_name: Option<&str>,
    root: &Value,
    nested: Vec<NestedSchemaEntry>,
    provider: Provider,
) -> Result<Value, LlmSchemaError> {
    let defs: Vec<(&str, Value)> = nested.into_iter().map(|(n, f)| (n, f())).collect();
    llm_schema_with_defs(root_name, root, &defs, provider)
}

/// The LLM schema of a `schema!` / `#[derive(Validate)]` type:
/// `Weather::llm_schema(Provider::OpenAiStrict)`.
pub trait LlmSchema {
    #[doc(hidden)]
    fn __vld_llm_parts() -> (&'static str, Value, Vec<NestedSchemaEntry>);

    /// This type's schema with every nested named schema in `$defs`,
    /// prepared for `provider`.
    fn llm_schema(provider: Provider) -> Result<Value, LlmSchemaError> {
        let (name, root, nested) = Self::__vld_llm_parts();
        from_entries(Some(name), &root, nested, provider)
    }
}

/// The LLM schema of a schema value: `vld::object()...llm_schema(provider)`.
pub trait LlmSchemaExt: JsonSchema + CollectNestedSchemas {
    /// This schema with every nested named schema in `$defs`, prepared for
    /// `provider`.
    fn llm_schema(&self, provider: Provider) -> Result<Value, LlmSchemaError> {
        let mut nested = Vec::new();
        self.collect_nested_schemas(&mut nested);
        from_entries(None, &self.json_schema(), nested, provider)
    }
}

impl<S: JsonSchema + CollectNestedSchemas> LlmSchemaExt for S {}
