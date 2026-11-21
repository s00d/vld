//! Stories: an MCP server whose tools take vld-validated arguments, called by
//! an rmcp client over `tokio::io::duplex`.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolRequestParams, CallToolResult, ClientConfig, JsonObject};
use rmcp::{ClientHandler, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde_json::{Value, json};
use vld::llm::{LlmSchema, Provider};
use vld_web::rmcp::{VldParameters, VldToolError, input_schema, parse_args};

vld::schema! {
    #[derive(Debug)]
    pub struct Forecast {
        pub city: String => vld::string().min(2),
        pub days: u8 => vld::u8().min(1).max(14),
    }
}

#[derive(Clone)]
struct Weather {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Weather {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Forecast (extractor)", input_schema = input_schema::<Forecast>())]
    async fn forecast(&self, VldParameters(f): VldParameters<Forecast>) -> String {
        format!("{} days in {}", f.days, f.city)
    }

    #[tool(description = "Forecast (parsed in the body)", input_schema = input_schema::<Forecast>())]
    async fn forecast_body(&self, args: JsonObject) -> Result<String, VldToolError> {
        let f: Forecast = parse_args(args)?;
        Ok(format!("{} days in {}", f.days, f.city))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Weather {}

#[derive(Clone, Default)]
struct Client;

impl ClientHandler for Client {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::default()
    }
}

fn call(name: &'static str, args: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
}

fn text(r: &CallToolResult) -> &str {
    r.content[0].as_text().map(|t| t.text.as_str()).unwrap()
}

/// Given an MCP server with two tools taking `Forecast` (city min 2, days
/// 1–14): one through the `VldParameters` extractor, one parsing in the body
/// and returning `Result<_, VldToolError>`
/// When a client lists the tools and calls them with good and bad arguments
/// Then both tools advertise vld's schema as `inputSchema`, good arguments
/// run the tool, and bad ones come back as `isError` results whose text is
/// the feedback lines (the body variant also carries the issues as
/// structured content), not as protocol errors.
#[tokio::test]
async fn bad_tool_arguments_are_is_error_results_with_feedback() {
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server = tokio::spawn(async move {
        Weather::new()
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = Client.serve(client_io).await.unwrap();

    let expected = Forecast::llm_schema(Provider::Generic).unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 2);
    for t in &tools {
        assert_eq!(
            Value::Object((*t.input_schema).clone()),
            expected,
            "{}",
            t.name
        );
    }

    for name in ["forecast", "forecast_body"] {
        let ok = client
            .call_tool(call(name, json!({"city": "Oslo", "days": 3})))
            .await
            .unwrap();
        assert_ne!(ok.is_error, Some(true), "{name}");
        assert_eq!(text(&ok), "3 days in Oslo");
    }

    let feedback =
        "city: expected string length >= 2, got \"O\"\ndays: expected number <= 14, got 30";
    let bad = json!({"city": "O", "days": 30});

    let r = client
        .call_tool(call("forecast", bad.clone()))
        .await
        .unwrap();
    assert_eq!(r.is_error, Some(true));
    assert_eq!(
        text(&r),
        format!("failed to deserialize parameters: {feedback}")
    );

    let r = client.call_tool(call("forecast_body", bad)).await.unwrap();
    assert_eq!(r.is_error, Some(true));
    assert_eq!(text(&r), feedback);
    assert_eq!(
        r.structured_content.unwrap()["issues"][1],
        json!({"path": "days", "expected": "expected number <= 14", "received": "30", "hint": null})
    );

    client.cancel().await.unwrap();
    server.abort();
}
