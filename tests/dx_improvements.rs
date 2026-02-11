//! Tests for DX / API improvements (features 13–20).

// -----------------------------------------------------------------------
// 13. impl_default! macro
// -----------------------------------------------------------------------

mod default_macro {
    #[allow(unused_imports)]
    use vld::prelude::*;

    vld::schema! {
        #[derive(Debug, PartialEq)]
        pub struct Config {
            pub host: String => vld::string().with_default("localhost".into()),
            pub port: Option<i64> => vld::number().int().optional(),
            pub tags: Vec<String> => vld::array(vld::string()).with_default(vec![]),
        }
    }

    vld::impl_default!(Config { host, port, tags });

    #[test]
    fn default_produces_zero_values() {
        let cfg = Config::default();
        assert_eq!(cfg.host, ""); // String::default
        assert_eq!(cfg.port, None);
        assert!(cfg.tags.is_empty());
    }

    #[test]
    fn default_struct_can_parse() {
        let cfg = Config::parse(r#"{"host":"example.com"}"#).unwrap();
        assert_eq!(cfg.host, "example.com");
        assert_eq!(cfg.port, None);
        assert!(cfg.tags.is_empty());
    }
}

// -----------------------------------------------------------------------
// 14. .message() method
// -----------------------------------------------------------------------

mod message_method {
    use vld::prelude::*;

    #[test]
    fn message_overrides_error() {
        let schema = vld::string().min(5).message("Too short!");
        let err = schema.parse(r#""ab""#).unwrap_err();
        assert_eq!(err.issues[0].message, "Too short!");
    }

    #[test]
    fn message_does_not_affect_success() {
        let schema = vld::string().min(2).message("error");
        let result = schema.parse(r#""hello""#).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn message_replaces_all_issues() {
        let schema = vld::number().int().min(0).max(100).message("Bad number");
        let err = schema.parse(r#""not a number""#).unwrap_err();
        for issue in &err.issues {
            assert_eq!(issue.message, "Bad number");
        }
    }

    #[test]
    fn message_chain() {
        let schema = vld::string().email().message("Invalid email");
        let err = schema.parse(r#""not-email""#).unwrap_err();
        assert_eq!(err.issues[0].message, "Invalid email");
    }
}

// -----------------------------------------------------------------------
// 15. Fluent error builder for super_refine
// -----------------------------------------------------------------------

mod fluent_error_builder {
    use vld::prelude::*;

    #[test]
    fn issue_builder_basic() {
        let mut errors = VldError::new();
        errors
            .issue(IssueCode::Custom {
                code: "test".into(),
            })
            .message("test message")
            .finish();
        assert_eq!(errors.issues.len(), 1);
        assert_eq!(errors.issues[0].message, "test message");
    }

    #[test]
    fn issue_builder_with_path_and_received() {
        let mut errors = VldError::new();
        errors
            .issue(IssueCode::Custom {
                code: "field_check".into(),
            })
            .message("invalid value")
            .path_field("user")
            .path_field("email")
            .received(&serde_json::json!("bad"))
            .finish();

        assert_eq!(errors.issues.len(), 1);
        let issue = &errors.issues[0];
        assert_eq!(issue.path.len(), 2);
        assert_eq!(issue.received, Some(serde_json::json!("bad")));
    }

    #[test]
    fn issue_builder_with_super_refine() {
        let schema = vld::string().super_refine(|s, errors| {
            if !s.contains('@') {
                errors
                    .issue(IssueCode::Custom {
                        code: "no_at".into(),
                    })
                    .message("Missing @ symbol")
                    .finish();
            }
            if s.len() < 3 {
                errors
                    .issue(IssueCode::Custom {
                        code: "too_short".into(),
                    })
                    .message("Too short")
                    .finish();
            }
        });

        let err = schema.parse(r#""ab""#).unwrap_err();
        assert_eq!(err.issues.len(), 2);
        assert_eq!(err.issues[0].message, "Missing @ symbol");
        assert_eq!(err.issues[1].message, "Too short");
    }

    #[test]
    fn issue_builder_default_message() {
        let mut errors = VldError::new();
        errors
            .issue(IssueCode::Custom {
                code: "my_code".into(),
            })
            .finish(); // no .message() call
        assert!(errors.issues[0].message.contains("my_code"));
    }
}

// -----------------------------------------------------------------------
// 16. ZObject::field_optional()
// -----------------------------------------------------------------------

mod field_optional {
    use vld::prelude::*;

    #[test]
    fn field_optional_missing_field() {
        let schema = vld::object()
            .field("name", vld::string().min(1))
            .field_optional("nickname", vld::string().min(1));

        let result = schema.parse(r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(result.get("name").unwrap(), "Alice");
        assert_eq!(result.get("nickname").unwrap(), &serde_json::Value::Null);
    }

    #[test]
    fn field_optional_present() {
        let schema = vld::object()
            .field("name", vld::string())
            .field_optional("age", vld::number().int());

        let result = schema.parse(r#"{"name": "Bob", "age": 30}"#).unwrap();
        assert_eq!(result.get("age").unwrap(), 30);
    }

    #[test]
    fn field_optional_null_value() {
        let schema = vld::object()
            .field("name", vld::string())
            .field_optional("bio", vld::string());

        let result = schema.parse(r#"{"name": "Bob", "bio": null}"#).unwrap();
        assert_eq!(result.get("bio").unwrap(), &serde_json::Value::Null);
    }

    #[test]
    fn field_optional_invalid_when_present() {
        let schema = vld::object()
            .field("name", vld::string())
            .field_optional("age", vld::number().int().min(0));

        let err = schema.parse(r#"{"name": "Bob", "age": -5}"#).unwrap_err();
        assert!(!err.issues.is_empty());
    }
}

// -----------------------------------------------------------------------
// 17. Conditional validation — when()
// -----------------------------------------------------------------------

mod conditional_validation {
    use vld::prelude::*;

    #[test]
    fn when_condition_met_passes() {
        let schema = vld::object()
            .field("role", vld::string())
            .field_optional("admin_key", vld::string())
            .when("role", "admin", "admin_key", vld::string().min(10));

        let result = schema.parse(r#"{"role": "admin", "admin_key": "super-secret-key"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn when_condition_met_fails() {
        let schema = vld::object()
            .field("role", vld::string())
            .field_optional("admin_key", vld::string())
            .when("role", "admin", "admin_key", vld::string().min(10));

        let err = schema
            .parse(r#"{"role": "admin", "admin_key": "short"}"#)
            .unwrap_err();
        assert!(!err.issues.is_empty());
    }

    #[test]
    fn when_condition_not_met_skips() {
        let schema = vld::object()
            .field("role", vld::string())
            .field_optional("admin_key", vld::string())
            .when("role", "admin", "admin_key", vld::string().min(10));

        let result = schema.parse(r#"{"role": "user"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn when_with_numeric_condition() {
        let schema = vld::object()
            .field("type", vld::number().int())
            .field_optional("detail", vld::string())
            .when("type", serde_json::json!(1), "detail", vld::string().min(5));

        // type == 1, detail too short → fail
        let err = schema.parse(r#"{"type": 1, "detail": "ab"}"#).unwrap_err();
        assert!(!err.issues.is_empty());

        // type == 2, detail too short → ok (condition not met)
        let ok = schema.parse(r#"{"type": 2, "detail": "ab"}"#);
        assert!(ok.is_ok());
    }
}

// -----------------------------------------------------------------------
// 18. i18n module
// -----------------------------------------------------------------------

mod i18n_module {
    use std::collections::HashMap;
    use vld::i18n::{translate_error, FnResolver, MapResolver};
    use vld::prelude::*;

    #[test]
    fn translate_with_map_resolver() {
        let mut map = HashMap::new();
        map.insert("too_small".into(), "Минимум {minimum}".into());
        let resolver = MapResolver::new(map);

        let err = vld::string().min(5).parse(r#""ab""#).unwrap_err();
        let translated = translate_error(&err, &resolver);
        assert!(translated.issues[0].message.contains("5"));
        assert!(translated.issues[0].message.contains("Минимум"));
    }

    #[test]
    fn translate_with_fn_resolver() {
        let resolver = FnResolver::new(|key| match key {
            "too_small" => Some("Zu kurz! Min: {minimum}".into()),
            _ => None,
        });

        let err = vld::string().min(3).parse(r#""ab""#).unwrap_err();
        let translated = translate_error(&err, &resolver);
        assert!(translated.issues[0].message.contains("Zu kurz"));
        assert!(translated.issues[0].message.contains("3"));
    }

    #[test]
    fn untranslated_keys_keep_original_message() {
        let resolver = FnResolver::new(|_| None);

        let err = vld::string().email().parse(r#""bad""#).unwrap_err();
        let translated = translate_error(&err, &resolver);
        assert_eq!(translated.issues[0].message, err.issues[0].message);
    }

    #[test]
    fn builtin_russian() {
        let resolver = vld::i18n::russian();
        let err = vld::number().min(10.0).parse("5").unwrap_err();
        let translated = translate_error(&err, &resolver);
        assert!(translated.issues[0].message.contains("10"));
    }

    #[test]
    fn builtin_german() {
        let resolver = vld::i18n::german();
        let err = vld::number().min(10.0).parse("5").unwrap_err();
        let translated = translate_error(&err, &resolver);
        assert!(translated.issues[0].message.contains("10"));
    }

    #[test]
    fn builtin_spanish() {
        let resolver = vld::i18n::spanish();
        let err = vld::number().min(10.0).parse("5").unwrap_err();
        let translated = translate_error(&err, &resolver);
        assert!(translated.issues[0].message.contains("10"));
    }
}

// -----------------------------------------------------------------------
// 19. Schema diffing
// -----------------------------------------------------------------------

#[cfg(feature = "diff")]
mod schema_diffing {
    use serde_json::json;
    use vld::diff::diff_schemas;

    #[test]
    fn no_changes() {
        let schema = json!({"type": "string"});
        let diff = diff_schemas(&schema, &schema);
        assert!(diff.changes.is_empty());
        assert!(!diff.has_breaking());
    }

    #[test]
    fn type_change_is_breaking() {
        let old = json!({"type": "string"});
        let new = json!({"type": "number"});
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking());
    }

    #[test]
    fn added_required_field_is_breaking() {
        let old = json!({
            "type": "object",
            "required": ["name"],
            "properties": { "name": { "type": "string" } }
        });
        let new = json!({
            "type": "object",
            "required": ["name", "email"],
            "properties": {
                "name": { "type": "string" },
                "email": { "type": "string" }
            }
        });
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking());
        assert!(diff
            .breaking_changes()
            .iter()
            .any(|c| c.description.contains("email")));
    }

    #[test]
    fn added_optional_field_is_non_breaking() {
        let old = json!({
            "type": "object",
            "required": ["name"],
            "properties": { "name": { "type": "string" } }
        });
        let new = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            }
        });
        let diff = diff_schemas(&old, &new);
        assert!(!diff.has_breaking());
    }

    #[test]
    fn removed_field_is_breaking() {
        let old = json!({
            "type": "object",
            "required": ["name", "email"],
            "properties": {
                "name": { "type": "string" },
                "email": { "type": "string" }
            }
        });
        let new = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        });
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking());
    }

    #[test]
    fn tightened_minimum_is_breaking() {
        let old = json!({"type": "number", "minimum": 0});
        let new = json!({"type": "number", "minimum": 5});
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking());
    }

    #[test]
    fn relaxed_minimum_is_non_breaking() {
        let old = json!({"type": "number", "minimum": 5});
        let new = json!({"type": "number", "minimum": 0});
        let diff = diff_schemas(&old, &new);
        assert!(!diff.has_breaking());
    }

    #[test]
    fn format_added_is_breaking() {
        let old = json!({"type": "string"});
        let new = json!({"type": "string", "format": "email"});
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking());
    }

    #[test]
    fn format_removed_is_non_breaking() {
        let old = json!({"type": "string", "format": "email"});
        let new = json!({"type": "string"});
        let diff = diff_schemas(&old, &new);
        assert!(!diff.has_breaking());
    }

    #[test]
    fn enum_value_removed_is_breaking() {
        let old = json!({"type": "string", "enum": ["a", "b", "c"]});
        let new = json!({"type": "string", "enum": ["a", "b"]});
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking());
    }

    #[test]
    fn enum_value_added_is_non_breaking() {
        let old = json!({"type": "string", "enum": ["a", "b"]});
        let new = json!({"type": "string", "enum": ["a", "b", "c"]});
        let diff = diff_schemas(&old, &new);
        assert!(!diff.has_breaking());
    }

    #[test]
    fn display_works() {
        let old = json!({"type": "string"});
        let new = json!({"type": "number"});
        let diff = diff_schemas(&old, &new);
        let display = format!("{}", diff);
        assert!(display.contains("BREAKING"));
    }

    #[test]
    fn nested_property_change() {
        let old = json!({
            "type": "object",
            "required": ["addr"],
            "properties": {
                "addr": {
                    "type": "object",
                    "required": ["city"],
                    "properties": {
                        "city": { "type": "string", "minLength": 1 }
                    }
                }
            }
        });
        let new = json!({
            "type": "object",
            "required": ["addr"],
            "properties": {
                "addr": {
                    "type": "object",
                    "required": ["city"],
                    "properties": {
                        "city": { "type": "string", "minLength": 3 }
                    }
                }
            }
        });
        let diff = diff_schemas(&old, &new);
        assert!(diff.has_breaking()); // minLength increased
        assert!(diff
            .breaking_changes()
            .iter()
            .any(|c| c.path.contains("properties.addr")));
    }
}

// -----------------------------------------------------------------------
// 20. WASM / std feature (compile-time check)
// -----------------------------------------------------------------------

#[cfg(feature = "std")]
mod wasm_std {
    #[test]
    fn path_input_available_with_std() {
        // The `std` feature is on by default, so Path should work.
        use vld::input::VldInput;
        let path = std::path::Path::new("nonexistent.json");
        let err = path.to_json_value();
        assert!(err.is_err()); // File doesn't exist, but the impl is there.
    }
}
