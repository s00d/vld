//! Model test: random env text through `from_vars_prefixed`.
//!
//! proptest picks, per field of [`Settings`], whether its variable is set,
//! its text (words, integers, leading zeros, booleans in any case, empty)
//! and the case of its name (`APP_PORT` / `app_port`), and mixes in
//! variables without the prefix. A model of the documented rules predicts
//! the settings or the exact `(variable, code)` list:
//!
//! - the prefix is stripped in any case, the rest lower-cased, `__` nests;
//! - only `true` / `false` and canonical integers become booleans / numbers,
//!   a string field gets the text back, a single value is a one-item list;
//! - an issue is named by the variable read, or `APP_` + the upper-cased key
//!   path when the variable is missing.

use super::common::*;

use proptest::prelude::*;
use vld_app::env::from_vars_prefixed;

fn env_text() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{1,8}",
        (-5i64..70000).prop_map(|n| n.to_string()),
        (0i64..999).prop_map(|n| format!("00{n}")),
        (any::<bool>(), any::<bool>()).prop_map(|(b, upper)| {
            let s = b.to_string();
            if upper { s.to_uppercase() } else { s }
        }),
        Just(String::new()),
    ]
}

/// A variable that may be unset: `(text, lower-case name)`.
fn var() -> impl Strategy<Value = Option<(String, bool)>> {
    proptest::option::weighted(0.85, (env_text(), any::<bool>()))
}

/// The integer a text is coerced to.
fn int_of(t: &str) -> Option<i64> {
    t.parse::<i64>().ok().filter(|n| n.to_string() == t)
}

fn bool_of(t: &str) -> Option<bool> {
    match t {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn vars_match_the_model(
        host in var(), port in var(), debug in var(), tags in var(),
        pool_size in var(), name in var(),
        junk in proptest::collection::vec(("(OTHER|APPX|AP)_[A-Z]{1,5}", env_text()), 0..4),
    ) {
        let fields = [
            ("HOST", &host), ("PORT", &port), ("DEBUG", &debug), ("TAGS", &tags),
            ("DB__POOL_SIZE", &pool_size), ("DB__NAME", &name),
        ];
        let name_of = |key: &str, lower: bool| {
            let n = format!("APP_{key}");
            if lower { n.to_lowercase() } else { n }
        };
        let mut vars: Vec<(String, String)> = junk;
        for (key, v) in fields {
            if let Some((text, lower)) = v {
                vars.push((name_of(key, *lower), text.clone()));
            }
        }
        let got = from_vars_prefixed::<Settings, _, _, _>(vars, "APP_");

        // Model: `None` for a field that has an issue.
        let mut want_issues: Vec<(String, &str)> = Vec::new();
        let mut issue = |key: &str, v: &Option<(String, bool)>, code| {
            let var = match v {
                Some((_, lower)) => name_of(key, *lower),
                None => format!("APP_{key}"),
            };
            want_issues.push((var, code));
        };
        let w_host = match &host {
            Some((t, _)) if !t.is_empty() => Some(t.clone()),
            Some(_) => { issue("HOST", &host, "too_small"); None }
            None => { issue("HOST", &host, "missing_field"); None }
        };
        let w_port = match &port {
            Some((t, _)) => match int_of(t) {
                Some(n) if n < 1 => { issue("PORT", &port, "too_small"); None }
                Some(n) if n > 65535 => { issue("PORT", &port, "too_big"); None }
                Some(n) => Some(n),
                None => { issue("PORT", &port, "invalid_type"); None }
            },
            None => { issue("PORT", &port, "missing_field"); None }
        };
        let w_debug = match &debug {
            Some((t, _)) => bool_of(t).or_else(|| { issue("DEBUG", &debug, "invalid_type"); None }),
            None => { issue("DEBUG", &debug, "missing_field"); None }
        };
        let w_tags = match &tags {
            Some((t, _)) => Some(vec![t.clone()]),
            None => { issue("TAGS", &tags, "missing_field"); None }
        };
        let w_db = if pool_size.is_none() && name.is_none() {
            issue("DB", &None, "missing_field");
            None
        } else {
            let w_pool = match &pool_size {
                Some((t, _)) => match int_of(t) {
                    Some(n) if n < 1 => { issue("DB__POOL_SIZE", &pool_size, "too_small"); None }
                    Some(n) => Some(n),
                    None => { issue("DB__POOL_SIZE", &pool_size, "invalid_type"); None }
                },
                None => { issue("DB__POOL_SIZE", &pool_size, "missing_field"); None }
            };
            let w_name = match &name {
                Some((t, _)) => Some(t.clone()),
                None => { issue("DB__NAME", &name, "missing_field"); None }
            };
            w_pool.zip(w_name).map(|(pool_size, name)| Db { pool_size, name })
        };

        if want_issues.is_empty() {
            let want = Settings {
                host: w_host.unwrap(), port: w_port.unwrap(), debug: w_debug.unwrap(),
                tags: w_tags.unwrap(), db: w_db.unwrap(),
            };
            prop_assert_eq!(got.unwrap(), want);
        } else {
            let want: Vec<(String, String)> =
                want_issues.into_iter().map(|(v, c)| (v, c.to_string())).collect();
            prop_assert_eq!(var_issues(got), want);
        }
    }
}
