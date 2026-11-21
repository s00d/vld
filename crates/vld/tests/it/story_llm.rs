//! Stories: a request type sent to a model as a structured-output / tool
//! schema for each provider (`vld::llm`), the model's reply parsed back, and
//! the validation issues returned to the model as retry feedback.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vld::Validate;
use vld::llm::{
    LlmFeedbackOptions, LlmParseError, LlmSchema, LlmSchemaError, LlmSchemaExt, Provider,
    llm_schema, parse_llm, parse_llm_with,
};
use vld::prelude::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate)]
struct Address {
    #[vld(vld::string().min(2).max(80))]
    city: String,
    #[vld(vld::string().pattern(r"^[0-9]{5}$").optional())]
    zip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Contact {
    Email {
        #[vld(vld::string().email())]
        address: String,
    },
    Phone {
        #[vld(vld::string().min(7))]
        number: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate)]
struct Order {
    #[vld(vld::string().min(3).max(40))]
    customer: String,
    #[vld(vld::number().int().min(1).max(100))]
    quantity: i64,
    #[vld(vld::number().min(0.0).multiple_of(0.01))]
    price: f64,
    #[vld(vld::enumeration(&["low", "normal", "high"]))]
    priority: String,
    #[vld(vld::string().max(500).optional())]
    note: Option<String>,
    #[vld(vld::string().email().nullable())]
    cc: Option<String>,
    #[vld(vld::nested!(Address))]
    ship_to: Address,
    #[vld(vld::nested!(Address).optional())]
    bill_to: Option<Address>,
    #[vld(vld::nested!(Contact))]
    contact: Contact,
    #[vld(vld::array(vld::string().min(1)).min_len(1).max_len(5))]
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate)]
struct Category {
    #[vld(vld::string().non_empty())]
    name: String,
    #[vld(vld::array(vld::nested!(Category)))]
    children: Vec<Category>,
}

fn full_order() -> Order {
    Order {
        customer: "Ada".into(),
        quantity: 2,
        price: 9.99,
        priority: "high".into(),
        note: Some("ring twice".into()),
        cc: Some("ops@example.com".into()),
        ship_to: Address {
            city: "Paris".into(),
            zip: Some("75001".into()),
        },
        bill_to: Some(Address {
            city: "Lyon".into(),
            zip: Some("69001".into()),
        }),
        contact: Contact::Phone {
            number: "+3312345678".into(),
        },
        tags: vec!["gift".into()],
    }
}

fn validator(schema: &Value) -> jsonschema::Validator {
    jsonschema::draft202012::options()
        .build(schema)
        .unwrap_or_else(|e| panic!("invalid schema: {e}\n{schema:#}"))
}

fn snapshot(name: &str, value: &Value) {
    insta::with_settings!({ sort_maps => true, prepend_module_to_snapshot => false }, {
        insta::assert_json_snapshot!(name, value);
    });
}

/// Given an order type with string / number bounds, an enum, optional and
/// nullable fields, nested named types and an internally tagged enum
/// When its LLM schema is built for each provider
/// Then each matches its snapshot, is a valid draft 2020-12 schema, accepts
/// what serde writes for a full order, and the OpenAI schema (every key
/// required, optional ones nullable) also accepts an order with `null`s.
#[test]
fn order_schema_for_every_provider() {
    let full = serde_json::to_value(full_order()).unwrap();
    let sparse = serde_json::to_value(Order {
        note: None,
        cc: None,
        bill_to: None,
        ..full_order()
    })
    .unwrap();
    for (name, provider) in [
        ("llm_order_openai", Provider::OpenAiStrict),
        ("llm_order_anthropic", Provider::Anthropic),
        ("llm_order_gemini", Provider::Gemini),
        ("llm_order_generic", Provider::Generic),
    ] {
        let schema = Order::llm_schema(provider).unwrap();
        snapshot(name, &schema);
        let v = validator(&schema);
        assert!(v.is_valid(&full), "{name}: {:?}", v.validate(&full).err());
        assert_eq!(
            v.is_valid(&sparse),
            provider == Provider::OpenAiStrict,
            "{name}"
        );
        // The schema value API gives the same result as the type API.
        assert_eq!(
            llm_schema(&schema, provider).unwrap(),
            schema,
            "{name} is a fixed point"
        );
    }
}

/// Given a recursive type (`Category` holds `Vec<Category>`) and a `lazy`
/// field (emitted as the empty schema)
/// When LLM schemas are built
/// Then the self-reference becomes `$ref: "#"` (OpenAI, Gemini, Generic),
/// Anthropic rejects the recursion, and OpenAI rejects the untyped field.
#[test]
fn recursion_by_provider() {
    let openai = Category::llm_schema(Provider::OpenAiStrict).unwrap();
    snapshot("llm_category_openai", &openai);
    assert_eq!(
        openai["properties"]["children"]["items"],
        json!({"$ref": "#"})
    );
    let tree = json!({"name": "a", "children": [{"name": "b", "children": []}]});
    assert!(validator(&openai).is_valid(&tree));
    assert!(Category::llm_schema(Provider::Gemini).is_ok());
    assert_eq!(
        Category::llm_schema(Provider::Anthropic),
        Err(LlmSchemaError::Recursive { name: "#".into() })
    );

    let lazy = vld::object().field("tree", vld::lazy(vld::string));
    assert_eq!(
        lazy.llm_schema(Provider::OpenAiStrict),
        Err(LlmSchemaError::Untyped {
            path: "#/properties/tree".into()
        })
    );
    assert_eq!(
        lazy.llm_schema(Provider::Generic).unwrap()["properties"]["tree"],
        json!({})
    );
}

/// Given schemas past a provider limit or with an unsupported shape
/// When LLM schemas are built
/// Then the error names the limit or the keyword, before any request is sent.
#[test]
fn limits_and_unsupported_shapes() {
    // Anthropic: at most 24 optional parameters.
    let mut wide = vld::object();
    for i in 0..25 {
        wide = wide.field(format!("f{i}"), vld::string().optional());
    }
    assert_eq!(
        wide.llm_schema(Provider::Anthropic),
        Err(LlmSchemaError::TooMany {
            what: "optional parameters",
            count: 25,
            max: 24
        })
    );
    assert!(wide.llm_schema(Provider::OpenAiStrict).is_ok());

    // OpenAI: at most 10 levels of object nesting.
    let mut deep = json!({"type": "object", "properties": {"leaf": {"type": "string"}}});
    for _ in 0..10 {
        deep = json!({"type": "object", "properties": {"next": deep}});
    }
    assert_eq!(
        llm_schema(&deep, Provider::OpenAiStrict),
        Err(LlmSchemaError::TooMany {
            what: "nesting depth",
            count: 11,
            max: 10
        })
    );
    assert!(llm_schema(&deep, Provider::Gemini).is_ok());

    // OpenAI: at most 1000 enum values.
    let many: Vec<String> = (0..1001).map(|i| format!("v{i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    let e = vld::object().field("e", vld::enumeration(&refs));
    assert!(matches!(
        e.llm_schema(Provider::OpenAiStrict),
        Err(LlmSchemaError::TooMany {
            what: "enum values",
            count: 1001,
            ..
        })
    ));

    // Strict providers need an object root without free-form maps.
    assert_eq!(
        vld::string().llm_schema(Provider::Anthropic),
        Err(LlmSchemaError::RootNotObject)
    );
    assert_eq!(
        vld::object()
            .field("labels", vld::record(vld::string()))
            .llm_schema(Provider::OpenAiStrict),
        Err(LlmSchemaError::Unsupported {
            path: "#/properties/labels".into(),
            keyword: "additionalProperties".into()
        })
    );
    let not = json!({"type": "object", "properties": {"a": {"not": {"type": "null"}}}});
    assert_eq!(
        llm_schema(&not, Provider::Gemini),
        Err(LlmSchemaError::Unsupported {
            path: "#/properties/a".into(),
            keyword: "not".into()
        })
    );
    assert!(llm_schema(&not, Provider::Generic).is_ok());
    assert_eq!(
        llm_schema(
            &json!({"$ref": "#/components/schemas/X"}),
            Provider::Generic
        ),
        Err(LlmSchemaError::UnresolvedRef {
            reference: "#/components/schemas/X".into()
        })
    );
    // Anthropic regexes: no lookaround (the pattern becomes text instead).
    let re =
        json!({"type": "object", "properties": {"p": {"type": "string", "pattern": "^(?!x).+$"}}});
    assert_eq!(
        llm_schema(&re, Provider::Anthropic).unwrap()["properties"]["p"],
        json!({"type": "string", "description": "pattern ^(?!x).+$"})
    );
}

/// Given model replies in the usual shapes (raw JSON, a fenced block after
/// prose, JSON inside prose with braces in strings, no JSON at all)
/// When parsed with `parse_llm`
/// Then valid JSON gives the typed order, a reply without JSON is `NoJson`,
/// and invalid JSON is a validation error whose feedback names each path,
/// the rule and the received value, ready for a retry prompt.
#[test]
fn reply_is_parsed_and_issues_become_feedback() {
    let order = serde_json::to_string(&full_order()).unwrap();
    for reply in [
        order.clone(),
        format!("Sure, here it is:\n```json\n{order}\n```\nAnything else?"),
        format!("Result {order} -- done {{not json}}"),
    ] {
        assert_eq!(parse_llm::<Order>(&reply).unwrap(), full_order(), "{reply}");
    }
    let err = parse_llm::<Order>("I cannot help with that.").unwrap_err();
    assert!(matches!(err, LlmParseError::NoJson));
    assert_eq!(err.llm_feedback(), "no JSON object or array found");

    let bad = json!({
        "customer": "Al", "quantity": 0, "price": 1.0, "priority": "urgent",
        "cc": null, "ship_to": {"city": "P"}, "contact": {"kind": "fax"}, "tags": []
    });
    let err = match parse_llm::<Order>(&format!("```\n{bad}\n```")) {
        Err(LlmParseError::Invalid(e)) => e,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        err.llm_feedback(),
        [
            r#"customer: expected string length >= 3, got "Al""#,
            "quantity: expected number >= 1, got 0",
            r#"priority: expected one of low, normal, high, got "urgent""#,
            r#"ship_to.city: expected string length >= 2, got "P""#,
            r#"contact.kind: expected one of email, phone, got "fax""#,
            "tags: expected array length >= 1",
        ]
        .join("\n")
    );

    let opts = LlmFeedbackOptions {
        max_issues: 2,
        retry_prompt: true,
        ..Default::default()
    };
    assert_eq!(
        err.llm_feedback_with(&opts),
        "Your previous output was invalid:\n\
         customer: expected string length >= 3, got \"Al\"\n\
         quantity: expected number >= 1, got 0\n\
         and 4 more\n\
         Return corrected JSON only."
    );
    assert_eq!(
        err.llm_feedback_json()[1],
        json!({"path": "quantity", "expected": "expected number >= 1", "received": "0", "hint": null})
    );
}

/// Given a strict object sent with its LLM schema and a redacted secret field
/// When the model adds an unknown key and a bad secret
/// Then the feedback hints the allowed keys from the schema and never echoes
/// the secret.
#[test]
fn unknown_keys_hint_allowed_keys_and_secrets_stay_hidden() {
    let schema = vld::object()
        .field(
            "user",
            vld::object()
                .field("name", vld::string())
                .field("token", vld::string().len(32).redact())
                .strict(),
        )
        .strict();
    let sent = schema.llm_schema(Provider::OpenAiStrict).unwrap();
    let err = match parse_llm_with(
        &schema,
        r#"{"user": {"name": "a", "token": "s3cr3t", "nick": "x"}}"#,
    ) {
        Err(LlmParseError::Invalid(e)) => e,
        other => panic!("{other:?}"),
    };
    let opts = LlmFeedbackOptions {
        schema: Some(sent),
        ..Default::default()
    };
    let text = err.llm_feedback_with(&opts);
    assert_eq!(
        text,
        "user.token: expected string length = 32, got [redacted]\n\
         user.nick: unknown key, remove it; allowed keys: name, token"
    );
    assert!(!text.contains("s3cr3t"));
    assert_eq!(
        err.llm_feedback_json_with(&opts)[1]["hint"],
        "allowed keys: name, token"
    );
}
