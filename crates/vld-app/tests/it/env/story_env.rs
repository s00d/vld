//! Stories: settings from environment variables.

use super::common::*;

use std::error::Error;
use vld_app::env::prelude::*;

fn valid() -> Vec<(&'static str, &'static str)> {
    vec![
        ("APP_HOST", "0.0.0.0"),
        ("APP_PORT", "8080"),
        ("APP_DEBUG", "true"),
        ("APP_TAGS", "web"),
        ("APP_DB__POOL_SIZE", "10"),
        ("APP_DB__NAME", "01234"),
        ("PATH", "/usr/bin"),
        ("APPLICATION", "other"),
    ]
}

/// Given the app's variables mixed with unrelated ones,
/// when they are validated with `from_vars_prefixed("APP_")`,
/// then the prefix is stripped in any case, `__` builds the `db` section,
/// numbers and booleans are typed, text stays text where the schema wants
/// it (`01234`, a numeric host), one value is a one-item list, a repeated
/// name keeps the later value, and names with an empty part are skipped.
#[test]
fn prefixed_variables_into_nested_settings() {
    let s: Settings = from_vars_prefixed(valid(), "APP_").unwrap();
    assert_eq!((s.host.as_str(), s.port, s.debug), ("0.0.0.0", 8080, true));
    assert_eq!(s.tags, ["web"]);
    assert_eq!(
        s.db,
        Db {
            pool_size: 10,
            name: "01234".into()
        }
    );

    let mut vars = valid();
    vars.extend([
        ("app_host", "12345"), // lower-case name, numeric text; later wins
        ("App_Db__Pool_Size", "20"),
        ("APP_", "x"),      // empty key
        ("APP__PORT", "x"), // empty part
        ("APP_DB__", "x"),
    ]);
    let s: Settings = from_vars_prefixed(vars, "APP_").unwrap();
    assert_eq!((s.host.as_str(), s.db.pool_size), ("12345", 20));

    // Owned pairs (as `std::env::vars()` yields) and no prefix.
    let owned: Vec<(String, String)> = valid()
        .into_iter()
        .filter_map(|(k, v)| Some((k.strip_prefix("APP_")?.to_string(), v.to_string())))
        .collect();
    let s: Settings = from_vars(owned).unwrap();
    assert_eq!(s.port, 8080);
}

/// Given variables with bad values and missing ones,
/// when they are validated,
/// then every issue is named by its variable: the name as read (any case),
/// or `PREFIX` + the upper-cased key path when the variable is missing; the
/// error's `Display` lists them and its `source()` is the `VldError`.
#[test]
fn errors_are_named_by_variable() {
    let vars = [
        ("app_port", "http"),
        ("APP_DEBUG", "yes"),
        ("APP_DB__POOL_SIZE", "0"),
    ];
    let r = from_vars_prefixed::<Settings, _, _, _>(vars, "APP_");
    assert_eq!(
        var_issues(r),
        pairs(&[
            ("APP_HOST", "missing_field"),
            ("app_port", "invalid_type"),
            ("APP_DEBUG", "invalid_type"),
            ("APP_TAGS", "missing_field"),
            ("APP_DB__POOL_SIZE", "too_small"),
            ("APP_DB__NAME", "missing_field"),
        ])
    );

    let err = from_vars_prefixed::<Settings, _, _, _>([("APP_PORT", "0")], "APP_").unwrap_err();
    let text = err.to_string();
    assert!(
        text.starts_with("invalid environment:\n  APP_HOST: "),
        "{text}"
    );
    assert!(text.contains("\n  APP_PORT: "), "{text}");
    assert!(text.contains("\n  APP_DB: "), "{text}");
    let source = err.source().unwrap();
    assert!(source.downcast_ref::<vld::error::VldError>().is_some());

    // An item issue is named by the list's variable.
    vld::schema! {
        #[derive(Debug)]
        struct Ports { ports: Vec<i64> => vld::array(vld::number().int().min(1)) }
    }
    assert_eq!(
        var_issues(from_vars_prefixed::<Ports, _, _, _>(
            [("SVC_PORTS", "0")],
            "SVC_"
        )),
        pairs(&[("SVC_PORTS", "too_small")])
    );
}

/// Given a variable that is both a value and a section,
/// when the variables are validated,
/// then it is a `Source` error naming the variable (in either order), with
/// no validation issues.
#[test]
fn value_and_section_conflict() {
    for vars in [
        [("APP_DB", "x"), ("APP_DB__NAME", "y")],
        [("APP_DB__NAME", "y"), ("APP_DB", "x")],
    ] {
        let err = from_vars_prefixed::<Settings, _, _, _>(vars, "APP_").unwrap_err();
        assert!(
            matches!(&err, VldEnvError::Source(m) if m.contains("is both a value and a section")),
            "{err}"
        );
        assert_eq!(err.var_issues().count(), 0);
        assert!(err.source().is_none());
        assert!(err.to_string().starts_with("env error: "));
    }
}

/// Given the real process environment (read only),
/// when it is validated with `from_env_prefixed` under a prefix nobody sets,
/// and with `from_env` into a schema of optional fields,
/// then the first reports every field missing under its variable name and
/// the second succeeds.
#[test]
fn process_environment() {
    let r = from_env_prefixed::<Settings>("VLD_APP_TEST_UNSET_");
    assert_eq!(
        var_issues(r),
        pairs(&[
            ("VLD_APP_TEST_UNSET_HOST", "missing_field"),
            ("VLD_APP_TEST_UNSET_PORT", "missing_field"),
            ("VLD_APP_TEST_UNSET_DEBUG", "missing_field"),
            ("VLD_APP_TEST_UNSET_TAGS", "missing_field"),
            ("VLD_APP_TEST_UNSET_DB", "missing_field"),
        ])
    );

    vld::schema! {
        #[derive(Debug)]
        struct Optional { vld_app_test_unset: Option<String> => vld::string().optional() }
    }
    assert_eq!(from_env::<Optional>().unwrap().vld_app_test_unset, None);
}
