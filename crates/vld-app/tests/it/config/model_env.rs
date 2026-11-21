//! Model test: random env text values through config-rs and `from_builder`.
//!
//! proptest picks the text of `APP_HOST`, `APP_PORT`, `APP_DEBUG` and
//! `APP_TAGS`: words, integers (signs, leading zeros, out of range), booleans
//! in any letter case. config-rs reads them with `try_parsing(true)` and
//! `from_builder` validates them into [`Flat`]. A model of the two documented
//! steps predicts the result:
//!
//! - config-rs parses a value as a boolean (any case), else an `i64`, else
//!   keeps the text;
//! - `vld_app::config` gives a string field the parsed value's text and wraps a
//!   single value into the list; number and boolean fields take only the
//!   matching parsed type.
//!
//! The model predicts the settings or the exact `(path, code)` list, and
//! `from_value_coerced` on the parsed JSON must agree with `from_builder`.

use super::common::*;

use proptest::prelude::*;
use serde_json::{Value, json};
use vld_app::config::{from_builder, from_value_coerced};

/// What config-rs `try_parsing` makes of an env value.
fn parsed(text: &str) -> Value {
    if let Ok(b) = text.to_lowercase().parse::<bool>() {
        json!(b)
    } else if let Ok(n) = text.parse::<i64>() {
        json!(n)
    } else {
        json!(text)
    }
}

/// The text a string field gets back from a parsed value.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn env_text() -> impl Strategy<Value = String> {
    prop_oneof![
        // Words that parse as neither a boolean nor a number.
        "[a-z]{1,8}".prop_filter("not a number or boolean", |w| {
            w.parse::<f64>().is_err() && w.parse::<bool>().is_err()
        }),
        (-5i64..70000).prop_map(|n| n.to_string()),
        (0i64..999).prop_map(|n| format!("00{n}")),
        (any::<bool>(), any::<bool>()).prop_map(|(b, upper)| {
            let s = b.to_string();
            if upper { s.to_uppercase() } else { s }
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn env_layer_matches_the_model(
        host in env_text(),
        port in env_text(),
        debug in env_text(),
        tags in env_text(),
    ) {
        let vars = [("APP_HOST", &host), ("APP_PORT", &port), ("APP_DEBUG", &debug), ("APP_TAGS", &tags)]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let env = config::Environment::with_prefix("APP").source(Some(vars)).try_parsing(true);
        let got = from_builder::<Flat>(config::Config::builder().add_source(env));

        // Model.
        let (p_host, p_port, p_debug, p_tags) = (parsed(&host), parsed(&port), parsed(&debug), parsed(&tags));
        let mut expected_issues = Vec::new();
        let port_value = match p_port.as_i64() {
            Some(n) if n < 1 => { expected_issues.push((".port", "too_small")); None }
            Some(n) if n > 65535 => { expected_issues.push((".port", "too_big")); None }
            Some(n) => Some(n),
            None => { expected_issues.push((".port", "invalid_type")); None }
        };
        let debug_value = p_debug.as_bool();
        if debug_value.is_none() {
            expected_issues.push((".debug", "invalid_type"));
        }

        let coerced = from_value_coerced::<Flat>(
            &json!({"host": p_host, "port": p_port, "debug": p_debug, "tags": p_tags}),
        );
        if expected_issues.is_empty() {
            let s = got.unwrap();
            prop_assert_eq!(&s, &Flat {
                host: text_of(&p_host),
                port: port_value.unwrap(),
                debug: debug_value.unwrap(),
                tags: vec![text_of(&p_tags)],
            });
            prop_assert_eq!(coerced.unwrap(), s);
        } else {
            prop_assert_eq!(issues(got), pairs(&expected_issues));
            prop_assert_eq!(issues(coerced), pairs(&expected_issues));
        }
    }
}
