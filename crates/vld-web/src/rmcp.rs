//! [rmcp](https://docs.rs/rmcp) (the MCP Rust SDK) tool arguments validated
//! by vld.
//!
//! rmcp deserializes tool arguments with serde and never checks them against
//! the advertised schema. Here the arguments go through the vld schema, and a
//! failure is returned as a tool result with `isError: true` whose text is
//! [`VldError::llm_feedback`] (SEP-1303), so the model can correct the call.
//!
//! - [`input_schema::<T>()`](input_schema): the tool's `inputSchema`, from vld.
//! - [`VldParameters<T>`]: a tool argument extractor (like rmcp's `Parameters`).
//! - [`parse_args`] + [`VldToolError`]: parse inside the tool and return
//!   `Result<_, VldToolError>`; the error carries the issues as structured
//!   content too.
//!
//! ```ignore
//! use rmcp::{handler::server::router::tool::ToolRouter, model::JsonObject, tool, tool_router};
//! use vld_web::rmcp::{VldParameters, VldToolError, input_schema, parse_args};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Forecast {
//!         pub city: String => vld::string().min(2),
//!         pub days: u8 => vld::u8().min(1).max(14),
//!     }
//! }
//!
//! #[tool_router]
//! impl Weather {
//!     #[tool(description = "Weather forecast", input_schema = input_schema::<Forecast>())]
//!     async fn forecast(&self, VldParameters(f): VldParameters<Forecast>) -> String {
//!         format!("{} days in {}", f.days, f.city)
//!     }
//!
//!     #[tool(description = "Same, parsed in the body", input_schema = input_schema::<Forecast>())]
//!     async fn forecast2(&self, args: JsonObject) -> Result<String, VldToolError> {
//!         let f: Forecast = parse_args(args)?;
//!         Ok(format!("{} days in {}", f.days, f.city))
//!     }
//! }
//! ```

use std::fmt;
use std::sync::Arc;

use rmcp::ErrorData;
use rmcp::handler::server::common::FromContextPart;
use rmcp::handler::server::tool::{IntoCallToolResult, ToolCallContext};
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock, JsonObject};
use serde_json::{Value, json};
use vld::error::VldError;
use vld::llm::{LlmFeedbackOptions, LlmSchema, Provider};
use vld::schema::VldParse;

/// The message prefix rmcp's tool router turns into an `isError` result.
const ARGS_ERROR_PREFIX: &str = "failed to deserialize parameters: ";

/// The tool's `inputSchema`: `T`'s JSON Schema (draft 2020-12, nested types
/// in `$defs`), for `#[tool(input_schema = input_schema::<T>())]` or
/// `Tool::new`.
///
/// # Panics
///
/// If `T`'s schema has an unresolved `$ref` (a vld bug) or is not an object.
pub fn input_schema<T: LlmSchema>() -> Arc<JsonObject> {
    match T::llm_schema(Provider::Generic) {
        Ok(Value::Object(schema)) => Arc::new(schema),
        other => panic!(
            "vld: no MCP input schema for {}: {other:?}",
            std::any::type_name::<T>()
        ),
    }
}

/// Validation failure of tool arguments, returned to the client as a tool
/// result with `isError: true`: the text is the feedback lines, the
/// structured content `{"issues": [{path, expected, received, hint}]}`.
#[derive(Debug, Clone)]
pub struct VldToolError {
    /// The validation issues.
    pub error: VldError,
    options: LlmFeedbackOptions,
}

impl VldToolError {
    /// Wrap issues; unknown-key hints list the keys of `T`'s schema.
    pub fn new<T: LlmSchema>(error: VldError) -> Self {
        Self {
            error,
            options: LlmFeedbackOptions {
                schema: T::llm_schema(Provider::Generic).ok(),
                ..Default::default()
            },
        }
    }

    /// The text sent to the model.
    pub fn feedback(&self) -> String {
        self.error.llm_feedback_with(&self.options)
    }

    /// The issues as JSON.
    pub fn feedback_json(&self) -> Value {
        self.error.llm_feedback_json_with(&self.options)
    }
}

impl From<VldError> for VldToolError {
    fn from(error: VldError) -> Self {
        Self {
            error,
            options: LlmFeedbackOptions::default(),
        }
    }
}

impl fmt::Display for VldToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.feedback())
    }
}

impl std::error::Error for VldToolError {}

impl IntoCallToolResult for VldToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        let mut result = CallToolResult::error(vec![ContentBlock::text(self.feedback())]);
        result.structured_content = Some(json!({ "issues": self.feedback_json() }));
        Ok(result.into())
    }
}

/// Parse tool arguments as `T`.
pub fn parse_args<T: VldParse + LlmSchema>(args: JsonObject) -> Result<T, VldToolError> {
    T::vld_parse_value(&Value::Object(args)).map_err(VldToolError::new::<T>)
}

/// Tool arguments validated by vld; a failure becomes an `isError` result
/// with the feedback text. Set the tool's schema with
/// `#[tool(input_schema = input_schema::<T>())]` (rmcp derives it only for
/// its own `Parameters`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VldParameters<T>(pub T);

crate::impl_deref!(VldParameters);

impl<S, T: VldParse + LlmSchema> FromContextPart<ToolCallContext<'_, S>> for VldParameters<T> {
    fn from_context_part(context: &mut ToolCallContext<'_, S>) -> Result<Self, ErrorData> {
        let args = context.arguments.take().unwrap_or_default();
        parse_args(args).map(Self).map_err(|e| {
            ErrorData::invalid_params(
                format!("{ARGS_ERROR_PREFIX}{}", e.feedback()),
                Some(json!({ "issues": e.feedback_json() })),
            )
        })
    }
}
