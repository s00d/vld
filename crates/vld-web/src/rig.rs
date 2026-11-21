//! [rig](https://docs.rs/rig-core) tools whose arguments are validated by vld.
//!
//! Implement [`VldToolHandler`] (name, description, a vld `Args` type and
//! `call`) and register [`VldTool::new(handler)?`](VldTool::new) as a rig
//! [`Tool`]: its `parameters()` is the vld schema prepared for the chosen
//! [`Provider`], and arguments that fail validation return
//! [`ToolExecutionError::invalid_args`] whose model-visible text is
//! [`VldError::llm_feedback`](vld::error::VldError::llm_feedback), so the
//! model can retry the call.
//!
//! ```ignore
//! use vld_web::rig::{VldTool, VldToolHandler};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct AddArgs { pub a: i64 => vld::i64(), pub b: i64 => vld::i64().min(0) }
//! }
//!
//! struct Add;
//! impl VldToolHandler for Add {
//!     const NAME: &'static str = "add";
//!     type Args = AddArgs;
//!     type Output = i64;
//!     type Error = std::convert::Infallible;
//!     fn description(&self) -> String { "Add two numbers".into() }
//!     async fn call(&self, args: AddArgs) -> Result<i64, Self::Error> { Ok(args.a + args.b) }
//! }
//!
//! let tool = VldTool::new(Add)?; // a rig `Tool`
//! ```

use std::future::Future;

use rig_core::tool::{IntoToolOutput, Tool, ToolContext, ToolExecutionError};
use serde_json::Value;
use vld::llm::{LlmFeedbackOptions, LlmSchema, LlmSchemaError, Provider};
use vld::schema::VldParse;

/// A rig tool with vld-validated arguments; wrap it in [`VldTool`].
pub trait VldToolHandler: Send + Sync {
    /// Registration and provider-facing name.
    const NAME: &'static str;
    /// The arguments, validated by vld before [`call`](Self::call).
    type Args: VldParse + LlmSchema + Send;
    /// The tool's output.
    type Output: IntoToolOutput;
    /// The tool's own error (not argument validation).
    type Error: std::error::Error + Send + Sync + 'static;

    /// What the tool does, for the model.
    fn description(&self) -> String;

    /// Run the tool on validated arguments.
    fn call(
        &self,
        args: Self::Args,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// A [`VldToolHandler`] as a rig [`Tool`].
pub struct VldTool<H> {
    handler: H,
    schema: Value,
}

impl<H: VldToolHandler> VldTool<H> {
    /// The tool with a plain JSON Schema ([`Provider::Generic`]).
    pub fn new(handler: H) -> Result<Self, LlmSchemaError> {
        Self::for_provider(handler, Provider::Generic)
    }

    /// The tool with its schema prepared for `provider` (e.g.
    /// [`Provider::OpenAiStrict`] for strict function calling); fails when
    /// the provider would reject the schema.
    pub fn for_provider(handler: H, provider: Provider) -> Result<Self, LlmSchemaError> {
        Ok(Self {
            schema: H::Args::llm_schema(provider)?,
            handler,
        })
    }

    /// The wrapped handler.
    pub fn handler(&self) -> &H {
        &self.handler
    }
}

/// Parse tool arguments as `T`; a failure is an `invalid_args` error whose
/// model-visible text is the feedback (unknown keys hinted from `schema`).
pub fn parse_args<T: VldParse>(
    args: &Value,
    schema: Option<&Value>,
) -> Result<T, ToolExecutionError> {
    T::vld_parse_value(args).map_err(|e| {
        let options = LlmFeedbackOptions {
            schema: schema.cloned(),
            ..Default::default()
        };
        ToolExecutionError::invalid_args(e.llm_feedback_with(&options))
    })
}

impl<H: VldToolHandler> Tool for VldTool<H> {
    const NAME: &'static str = H::NAME;
    type Args = Value;
    type Output = H::Output;
    type Error = ToolExecutionError;

    fn description(&self) -> String {
        self.handler.description()
    }

    fn parameters(&self) -> Value {
        self.schema.clone()
    }

    async fn call(
        &self,
        _: &mut ToolContext,
        args: Value,
    ) -> Result<H::Output, ToolExecutionError> {
        let args = parse_args::<H::Args>(&args, Some(&self.schema))?;
        self.handler
            .call(args)
            .await
            .map_err(ToolExecutionError::from_error)
    }
}
