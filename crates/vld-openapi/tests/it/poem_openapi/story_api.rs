//! Stories: a poem-openapi service takes vld types as request bodies — the
//! spec documents the vld constraints (nested components included), `VldJson`
//! answers 422 with every issue, `Json` keeps poem-openapi's 400.

use poem::Route;
use poem::http::StatusCode;
use poem::test::{TestClient, TestResponse};
use poem_openapi::payload::Json;
use poem_openapi::{OpenApi, OpenApiService};
use serde_json::{Value, json};
use vld_openapi::poem_openapi::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct Address {
        pub city: String => vld::string().min(1),
    }
}
impl_type!(Address);

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct NewUser {
        pub name: String => vld::string().min(2).max(20),
        pub age: i64 => vld::number().int().min(18),
        pub score: f64 => vld::number().gt(0.0),
        pub tags: Vec<String> => vld::array(vld::string()).max_len(3),
        pub address: Address => vld::nested!(Address),
    }
}
impl_type!(NewUser, "User");

struct Api;

#[OpenApi]
impl Api {
    #[oai(path = "/users", method = "post")]
    async fn create(&self, body: VldJson<NewUser>) -> VldJson<NewUser> {
        body
    }

    #[oai(path = "/plain", method = "post")]
    async fn plain(&self, body: Json<NewUser>) -> Json<String> {
        Json(body.0.address.city)
    }
}

fn service() -> OpenApiService<Api, ()> {
    OpenApiService::new(Api, "Users", "1.0")
}

fn client() -> TestClient<Route> {
    TestClient::new(Route::new().nest("/", service()))
}

fn user() -> Value {
    json!({"name": "Al", "age": 18, "score": 0.5, "tags": ["a"], "address": {"city": "Oslo"}})
}

async fn post(cli: &TestClient<Route>, path: &str, body: &Value) -> TestResponse {
    cli.post(path)
        .content_type("application/json")
        .body(body.to_string())
        .send()
        .await
}

/// `(path, code)` of a 422 body; checks the body shape on the way.
async fn issues(resp: TestResponse) -> Vec<(String, String)> {
    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    resp.assert_content_type("application/json");
    let body: Value =
        serde_json::from_str(&resp.0.into_body().into_string().await.unwrap()).unwrap();
    assert_eq!(body["error"], "Validation failed");
    body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            assert!(i["message"].is_string(), "{i}");
            (
                i["path"].as_str().unwrap().into(),
                i["code"].as_str().unwrap().into(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// Given a create-user operation taking `VldJson<NewUser>`,
/// when valid, invalid, malformed and wrongly typed bodies are posted,
/// then the valid one is echoed back, invalid ones are 422 with every
/// `(path, code)`, malformed JSON is 422 `parse_error`, and a non-JSON
/// content type is poem-openapi's 415.
#[tokio::test]
async fn vld_json_answers_422_with_every_issue() {
    let cli = client();

    let resp = post(&cli, "/users", &user()).await;
    resp.assert_status_is_ok();
    resp.assert_json(user()).await;

    let bad = json!({"name": "A", "age": 17, "score": 0, "tags": ["a", "b", "c", "d"], "address": {"city": ""}});
    assert_eq!(
        issues(post(&cli, "/users", &bad).await).await,
        pairs(&[
            (".name", "too_small"),
            (".age", "too_small"),
            (".score", "too_small"),
            (".tags", "too_big"),
            (".address.city", "too_small"),
        ])
    );
    assert_eq!(
        issues(post(&cli, "/users", &json!({"name": "Al"})).await).await,
        pairs(&[
            (".age", "missing_field"),
            (".score", "missing_field"),
            (".tags", "missing_field"),
            (".address", "missing_field"),
        ])
    );

    let resp = cli
        .post("/users")
        .content_type("application/json")
        .body("{\"name\":")
        .send()
        .await;
    assert_eq!(issues(resp).await, pairs(&[("", "parse_error")]));

    let resp = cli
        .post("/users")
        .content_type("text/plain")
        .body(user().to_string())
        .send()
        .await;
    resp.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

/// Given an operation taking poem-openapi's own `Json<NewUser>`,
/// when a valid and an invalid body are posted,
/// then the valid one reaches the handler and the invalid one is
/// poem-openapi's 400 carrying vld's message.
#[tokio::test]
async fn plain_json_validates_with_vld_and_keeps_poems_400() {
    let cli = client();

    let resp = post(&cli, "/plain", &user()).await;
    resp.assert_status_is_ok();
    resp.assert_json("Oslo").await;

    let mut bad = user();
    bad["age"] = json!(3);
    let resp = post(&cli, "/plain", &bad).await;
    resp.assert_status(StatusCode::BAD_REQUEST);
    let text = resp.0.into_body().into_string().await.unwrap();
    assert!(text.contains("failed to parse \"User\""), "{text}");
    assert!(text.contains("age"), "{text}");
}

/// Given the service,
/// when its spec is generated,
/// then both operations reference the `User` component, which carries the
/// vld constraints in the OpenAPI 3.0 dialect, and the nested `Address` is a
/// component of its own.
#[test]
fn the_spec_documents_vld_constraints() {
    let spec: Value = serde_json::from_str(&service().spec()).unwrap();
    let user_ref = json!({"$ref": "#/components/schemas/User"});
    for path in ["/users", "/plain"] {
        assert_eq!(
            spec["paths"][path]["post"]["requestBody"]["content"]["application/json; charset=utf-8"]
                ["schema"],
            user_ref,
            "{path}"
        );
    }
    assert_eq!(
        spec["paths"]["/users"]["post"]["responses"]["200"]["content"]["application/json; charset=utf-8"]
            ["schema"],
        user_ref
    );

    let schemas = &spec["components"]["schemas"];
    let user = &schemas["User"];
    assert_eq!(user["type"], "object");
    assert_eq!(
        user["required"],
        json!(["name", "age", "score", "tags", "address"])
    );
    let props = &user["properties"];
    assert_eq!(
        props["name"],
        json!({"type": "string", "minLength": 2, "maxLength": 20})
    );
    assert_eq!(props["age"]["type"], "integer");
    assert_eq!(props["age"]["minimum"], 18.0);
    assert_eq!(
        (
            &props["score"]["minimum"],
            &props["score"]["exclusiveMinimum"]
        ),
        (&json!(0.0), &json!(true))
    );
    assert_eq!(props["tags"]["maxItems"], 3);
    assert_eq!(props["tags"]["items"], json!({"type": "string"}));
    assert_eq!(
        props["address"],
        json!({"$ref": "#/components/schemas/Address"})
    );
    assert_eq!(schemas["Address"]["properties"]["city"]["minLength"], 1);
    assert_eq!(schemas["Address"]["required"], json!(["city"]));
}

/// Given a hand-written JSON Schema,
/// when it is converted with `json_schema_to_meta`,
/// then 2020-12 `type: [.., "null"]` becomes `nullable` and `const` an enum.
#[test]
fn any_json_schema_converts() {
    let meta = json_schema_to_meta(&json!({"type": ["string", "null"], "const": "x"}));
    assert_eq!(
        serde_json::to_value(meta).unwrap(),
        json!({"type": "string", "nullable": true, "enum": ["x"]})
    );
}
