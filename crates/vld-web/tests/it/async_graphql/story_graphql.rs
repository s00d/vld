//! Stories of a GraphQL API validating its input with
//! `vld_web::async_graphql`: the custom validator on arguments, validation
//! inside a resolver, and the error the client reads.

use async_graphql::{EmptySubscription, InputObject, Object, Response, Schema, ServerError};
use serde_json::{Value, json};
use vld::error::{IssueCode, VldError};
use vld_web::async_graphql::prelude::*;

#[derive(InputObject)]
struct NewUser {
    name: String,
    email: String,
    tags: Vec<String>,
}

vld::schema! {
    pub struct NewUserRules {
        pub name: String => vld::string().min(2),
        pub email: String => vld::string().email(),
        pub tags: Vec<String> => vld::array(vld::string().min(1)).max_len(2),
    }
}

struct Query;

#[Object]
impl Query {
    /// The validator on a required argument.
    async fn register(
        &self,
        #[graphql(validator(custom = "VldValidator::<NewUserRules>::new()"))] user: NewUser,
    ) -> String {
        format!("{} <{}>", user.name, user.email)
    }

    /// The validator on an optional argument: checked only when present.
    async fn maybe(
        &self,
        #[graphql(validator(custom = "VldValidator::<NewUserRules>::default()"))] user: Option<
            NewUser,
        >,
    ) -> String {
        user.map_or_else(|| "none".into(), |u| u.name)
    }
}

struct Mutation;

#[Object]
impl Mutation {
    /// Validation inside the resolver, plus a business rule as a `VldError`.
    async fn create(&self, user: NewUser) -> async_graphql::Result<String> {
        validate::<NewUserRules, _>(&user)?;
        if user.name == "root" {
            return Err(vld_error(&VldError::single(
                IssueCode::Custom {
                    code: "reserved".into(),
                },
                "Name is reserved",
            )));
        }
        Ok(user.name)
    }
}

fn schema() -> Schema<Query, Mutation, EmptySubscription> {
    Schema::new(Query, Mutation, EmptySubscription)
}

/// The only error of a response, with its `extensions.issues` as JSON.
#[track_caller]
fn only_error(res: &Response) -> (&ServerError, Value) {
    assert_eq!(res.errors.len(), 1, "{:?}", res.errors);
    let err = &res.errors[0];
    let issues = err
        .extensions
        .as_ref()
        .and_then(|e| e.get("issues"))
        .expect("extensions.issues")
        .clone()
        .into_json()
        .unwrap();
    (err, issues)
}

fn data(res: &Response) -> Value {
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    res.data.clone().into_json().unwrap()
}

/// Given a `register` query whose `user` argument has `VldValidator`,
/// when clients send valid and invalid users (inline and as variables),
/// then a valid user reaches the resolver, and an invalid one is a GraphQL
/// error with `extensions.issues` in the HTTP issue shape, and no data.
#[tokio::test]
async fn a_register_query_validates_its_argument() {
    let schema = schema();

    let res = schema
        .execute(r#"{ register(user: {name: "Alice", email: "a@b.co", tags: ["x"]}) }"#)
        .await;
    assert_eq!(data(&res), json!({"register": "Alice <a@b.co>"}));

    let res = schema
        .execute(r#"{ register(user: {name: "A", email: "bad", tags: ["", "b", "c"]}) }"#)
        .await;
    let (err, issues) = only_error(&res);
    assert_eq!(
        err.message,
        r#"Failed to parse "NewUser": Validation failed"#
    );
    let pairs: Vec<(&str, &str)> = issues
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            assert!(i["message"].is_string(), "{i}");
            (i["path"].as_str().unwrap(), i["code"].as_str().unwrap())
        })
        .collect();
    assert_eq!(
        pairs,
        [
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".tags", "too_big"),
            (".tags[0]", "too_small"),
        ]
    );
    assert_eq!(res.data, async_graphql::Value::Null);

    // Variables go through the same validator.
    let res = schema
        .execute(
            async_graphql::Request::new("query($u: NewUser!) { register(user: $u) }").variables(
                async_graphql::Variables::from_json(
                    json!({"u": {"name": "Bob", "email": "nope", "tags": []}}),
                ),
            ),
        )
        .await;
    let (_, issues) = only_error(&res);
    assert_eq!(issues[0]["path"], ".email");
    assert_eq!(issues.as_array().unwrap().len(), 1);
}

/// Given an optional argument with the validator,
/// when it is omitted, valid or invalid,
/// then only a present, invalid value is an error.
#[tokio::test]
async fn an_optional_argument_is_validated_when_present() {
    let schema = schema();
    assert_eq!(
        data(&schema.execute("{ maybe }").await),
        json!({"maybe": "none"})
    );
    assert_eq!(
        data(
            &schema
                .execute(r#"{ maybe(user: {name: "Al", email: "a@b.co", tags: []}) }"#)
                .await
        ),
        json!({"maybe": "Al"})
    );
    let res = schema
        .execute(r#"{ maybe(user: {name: "A", email: "a@b.co", tags: []}) }"#)
        .await;
    let (_, issues) = only_error(&res);
    assert_eq!(
        issues,
        json!([{"path": ".name", "message": issues[0]["message"], "code": "too_small"}])
    );
}

/// Given a `create` mutation validating in its resolver,
/// when the input breaks the schema or a business rule,
/// then the error message is `Validation failed`, it carries the resolver's
/// path, and `extensions.issues` lists the issues.
#[tokio::test]
async fn a_create_mutation_validates_in_its_resolver() {
    let schema = schema();

    let res = schema
        .execute(r#"mutation { create(user: {name: "Al", email: "a@b.co", tags: []}) }"#)
        .await;
    assert_eq!(data(&res), json!({"create": "Al"}));

    let res = schema
        .execute(r#"mutation { create(user: {name: "Al", email: "x", tags: []}) }"#)
        .await;
    let (err, issues) = only_error(&res);
    assert_eq!(err.message, "Validation failed");
    assert_eq!(serde_json::to_value(&err.path).unwrap(), json!(["create"]));
    assert_eq!(issues[0]["path"], ".email");
    assert_eq!(issues[0]["code"], "invalid_string");

    let res = schema
        .execute(r#"mutation { create(user: {name: "root", email: "r@b.co", tags: []}) }"#)
        .await;
    let (err, issues) = only_error(&res);
    assert_eq!(err.message, "Validation failed");
    assert_eq!(
        issues,
        json!([{"path": "", "message": "Name is reserved", "code": "reserved"}])
    );
}
