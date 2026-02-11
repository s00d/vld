//! Property-based (fuzz) tests — guarantee that validators never panic
//! on arbitrary JSON input.

use proptest::prelude::*;
use serde_json::Value;
use vld::prelude::*;

// -----------------------------------------------------------------------
// Helpers: arbitrary JSON value generators
// -----------------------------------------------------------------------

fn arb_json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(|f| serde_json::json!(f)),
        any::<i64>().prop_map(|i| serde_json::json!(i)),
        ".*".prop_map(|s: String| Value::String(s)),
    ];
    leaf.prop_recursive(
        3,  // max depth
        64, // max nodes
        8,  // items per collection
        |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..8).prop_map(Value::Array),
                prop::collection::vec(("[a-z_]{1,8}", inner), 0..6)
                    .prop_map(|pairs| { Value::Object(pairs.into_iter().collect()) }),
            ]
        },
    )
}

// -----------------------------------------------------------------------
// 1. ZString — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn string_schema_never_panics(val in arb_json_value()) {
        let schema = vld::string().min(1).max(100);
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn string_email_never_panics(val in arb_json_value()) {
        let schema = vld::string().email();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn string_coerce_never_panics(val in arb_json_value()) {
        let schema = vld::string().coerce().min(0);
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn string_trim_never_panics(s in ".*") {
        let schema = vld::string().trim().min(1);
        let val = Value::String(s);
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 2. ZNumber — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn number_schema_never_panics(val in arb_json_value()) {
        let schema = vld::number().min(0.0).max(1000.0);
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn number_int_never_panics(val in arb_json_value()) {
        let schema = vld::number().int().min(0).max(1000);
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn number_coerce_never_panics(val in arb_json_value()) {
        let schema = vld::number().coerce();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn number_arbitrary_f64(f in any::<f64>()) {
        let schema = vld::number();
        let val = serde_json::json!(f);
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 3. ZBoolean — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn boolean_schema_never_panics(val in arb_json_value()) {
        let schema = vld::boolean();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn boolean_coerce_never_panics(val in arb_json_value()) {
        let schema = vld::boolean().coerce();
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 4. ZArray — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn array_schema_never_panics(val in arb_json_value()) {
        let schema = vld::array(vld::string());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn array_nested_never_panics(val in arb_json_value()) {
        let schema = vld::array(vld::array(vld::number()));
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn array_with_limits_never_panics(val in arb_json_value()) {
        let schema = vld::array(vld::any()).min_len(1).max_len(5);
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 5. ZObject — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn object_schema_never_panics(val in arb_json_value()) {
        let schema = vld::object()
            .field("name", vld::string())
            .field("age", vld::number());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn object_strict_never_panics(val in arb_json_value()) {
        let schema = vld::object()
            .field("id", vld::number().int())
            .strict();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn object_passthrough_never_panics(val in arb_json_value()) {
        let schema = vld::object()
            .field("x", vld::any())
            .passthrough();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn object_field_optional_never_panics(val in arb_json_value()) {
        let schema = vld::object()
            .field("req", vld::string())
            .field_optional("opt", vld::number());
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 6. Modifiers — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn optional_never_panics(val in arb_json_value()) {
        let schema = vld::string().optional();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn nullable_never_panics(val in arb_json_value()) {
        let schema = vld::number().nullable();
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn with_default_never_panics(val in arb_json_value()) {
        let schema = vld::string().with_default("fallback".into());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn catch_never_panics(val in arb_json_value()) {
        let schema = vld::number().catch(0.0);
        let result = schema.parse_value(&val);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn nullish_never_panics(val in arb_json_value()) {
        let schema = vld::string().nullish();
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 7. Combinators — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn union_never_panics(val in arb_json_value()) {
        let schema = vld::union(vld::string(), vld::number());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn intersection_never_panics(val in arb_json_value()) {
        let schema = vld::intersection(
            vld::number().min(0.0),
            vld::number().max(100.0),
        );
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn refine_never_panics(val in arb_json_value()) {
        let schema = vld::string().refine(|s: &String| s.len() < 50, "too long");
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn transform_never_panics(val in arb_json_value()) {
        let schema = vld::string().transform(|s: String| s.len());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn message_never_panics(val in arb_json_value()) {
        let schema = vld::string().min(3).message("nope");
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 8. Literal / Enum / Any — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn literal_string_never_panics(val in arb_json_value()) {
        let schema = vld::literal("admin");
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn literal_int_never_panics(val in arb_json_value()) {
        let schema = vld::literal(42i64);
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn enum_never_panics(val in arb_json_value()) {
        let schema = vld::enumeration(&["a", "b", "c"]);
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn any_never_panics(val in arb_json_value()) {
        let schema = vld::any();
        let result = schema.parse_value(&val);
        prop_assert!(result.is_ok());
    }
}

// -----------------------------------------------------------------------
// 9. Discriminated union — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn discriminated_union_never_panics(val in arb_json_value()) {
        let schema = vld::discriminated_union("type")
            .variant("a", vld::object().field("type", vld::literal("a")))
            .variant("b", vld::object().field("type", vld::literal("b")));
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 10. schema! macro generated struct — never panics
// -----------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    struct FuzzUser {
        name: String => vld::string().min(1).max(100),
        age: Option<i64> => vld::number().int().optional(),
        tags: Vec<String> => vld::array(vld::string()).with_default(vec![]),
    }
}

proptest! {
    #[test]
    fn schema_macro_parse_never_panics(val in arb_json_value()) {
        let _ = FuzzUser::parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 11. Record / Map / Set — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn record_never_panics(val in arb_json_value()) {
        let schema = vld::record(vld::number());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn map_never_panics(val in arb_json_value()) {
        let schema = vld::map(vld::string(), vld::number());
        let _ = schema.parse_value(&val);
    }

    #[test]
    fn set_never_panics(val in arb_json_value()) {
        let schema = vld::set(vld::string());
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 12. Error formatting — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn error_display_never_panics(val in arb_json_value()) {
        let schema = vld::object()
            .field("x", vld::string().email())
            .field("y", vld::number().int().positive());
        if let Err(e) = schema.parse_value(&val) {
            let _ = format!("{}", e);
            let _ = vld::format::prettify_error(&e);
            let _ = vld::format::flatten_error(&e);
            let _ = vld::format::treeify_error(&e);
        }
    }
}

// -----------------------------------------------------------------------
// 13. i18n translation — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn i18n_translate_never_panics(val in arb_json_value()) {
        let schema = vld::string().min(3).email();
        if let Err(e) = schema.parse_value(&val) {
            let resolver = vld::i18n::russian();
            let _ = vld::i18n::translate_error(&e, &resolver);
        }
    }
}

// -----------------------------------------------------------------------
// 14. Schema diffing — never panics on arbitrary JSON
// -----------------------------------------------------------------------

#[cfg(feature = "diff")]
proptest! {
    #[test]
    fn diff_never_panics(old in arb_json_value(), new in arb_json_value()) {
        let diff = vld::diff::diff_schemas(&old, &new);
        let _ = format!("{}", diff);
        let _ = diff.has_breaking();
    }
}

// -----------------------------------------------------------------------
// 15. Conditional validation — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn conditional_when_never_panics(val in arb_json_value()) {
        let schema = vld::object()
            .field("role", vld::string())
            .field_optional("key", vld::string())
            .when("role", "admin", "key", vld::string().min(10));
        let _ = schema.parse_value(&val);
    }
}

// -----------------------------------------------------------------------
// 16. super_refine with fluent builder — never panics
// -----------------------------------------------------------------------

proptest! {
    #[test]
    fn super_refine_builder_never_panics(val in arb_json_value()) {
        let schema = vld::string().super_refine(|s, errors| {
            if s.len() < 3 {
                errors
                    .issue(IssueCode::Custom { code: "short".into() })
                    .message("too short")
                    .finish();
            }
        });
        let _ = schema.parse_value(&val);
    }
}
