//! Stories: a rig tool whose arguments are validated by vld.

use rig_core::tool::{Tool, ToolContext, ToolErrorKind, tool_definition};
use serde_json::json;
use vld::llm::{LlmSchema, LlmSchemaError, Provider};
use vld_web::rig::{VldTool, VldToolHandler};

vld::schema! {
    #[derive(Debug)]
    pub struct AddArgs {
        pub a: i64 => vld::number().int(),
        pub b: i64 => vld::number().int().min(0),
        pub note: Option<String> => vld::string().max(20).optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Node {
        pub children: Vec<Node> => vld::array(vld::nested!(Node)),
    }
}

struct Add;

impl VldToolHandler for Add {
    const NAME: &'static str = "add";
    type Args = AddArgs;
    type Output = i64;
    type Error = std::convert::Infallible;

    fn description(&self) -> String {
        "Add two numbers".into()
    }

    async fn call(&self, args: AddArgs) -> Result<i64, Self::Error> {
        Ok(args.a + args.b)
    }
}

struct Tree;

impl VldToolHandler for Tree {
    const NAME: &'static str = "tree";
    type Args = Node;
    type Output = usize;
    type Error = std::convert::Infallible;

    fn description(&self) -> String {
        "Count children".into()
    }

    async fn call(&self, args: Node) -> Result<usize, Self::Error> {
        Ok(args.children.len())
    }
}

/// Given an `add` tool with vld arguments (`b >= 0`, optional `note`)
/// When rig asks for its definition and calls it with good and bad arguments
/// Then the definition carries vld's schema for the chosen provider, good
/// arguments run the handler, and bad ones are an `invalid_args` error whose
/// model-visible text is the feedback; a provider that rejects the schema
/// (Anthropic, recursion) fails when the tool is built.
#[tokio::test]
async fn tool_definition_and_call_use_vld() {
    let tool = VldTool::new(Add).unwrap();
    let def = tool_definition(&tool);
    assert_eq!(def.name.as_str(), "add");
    assert_eq!(def.description, "Add two numbers");
    assert_eq!(
        def.parameters,
        AddArgs::llm_schema(Provider::Generic).unwrap()
    );

    let strict = VldTool::for_provider(Add, Provider::OpenAiStrict).unwrap();
    assert_eq!(strict.parameters()["required"], json!(["a", "b", "note"]));

    let mut cx = ToolContext::default();
    assert_eq!(
        tool.call(&mut cx, json!({"a": 2, "b": 3})).await.unwrap(),
        5
    );

    let err = tool
        .call(&mut cx, json!({"a": 2.5, "b": -1, "note": "x".repeat(30)}))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ToolErrorKind::InvalidArgs);
    assert_eq!(
        err.message(),
        "a: expected integer, got 2.5\nb: expected number >= 0, got -1\n\
         note: expected string length <= 20, got \"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\""
    );

    assert!(VldTool::new(Tree).is_ok());
    assert!(matches!(
        VldTool::for_provider(Tree, Provider::Anthropic),
        Err(LlmSchemaError::Recursive { .. })
    ));
}
