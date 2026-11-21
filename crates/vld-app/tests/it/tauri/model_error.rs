//! Model test: random validation errors through the crate's entry points.
//!
//! proptest builds a `VldError` (0–5 issues; paths of fields and indexes,
//! root included; codes with parameters, user codes; arbitrary messages) and
//! feeds it through `validate` (the structured `VldTauriError` the frontend
//! gets) and `VldPayload` deserialization (the inline error string). A small
//! model predicts, independently of the crate: every issue's path (`a.b[0]`,
//! no leading dot), message, code key and params map, the error category,
//! counts, `Display` and the inline `path: message; …` text.

use super::common::*;

use std::collections::BTreeMap;

use proptest::prelude::*;
use serde_json::json;
use vld::error::{IssueCode, PathSegment, SizeOrigin, StringValidation, ValidationIssue, VldError};
use vld_app::tauri::{VldPayload, VldTauriError, validate};

/// An issue code with the key and params the frontend must see for it.
#[derive(Debug, Clone)]
struct Code {
    code: IssueCode,
    key: String,
    params: BTreeMap<String, String>,
}

fn code() -> impl Strategy<Value = Code> {
    let kinds = ["string", "number", "object", "array"];
    prop_oneof![
        (0u32..1000, any::<bool>()).prop_map(|(n, inclusive)| Code {
            code: IssueCode::TooSmall {
                origin: SizeOrigin::String,
                minimum: json!(n),
                inclusive,
            },
            key: "too_small".into(),
            params: params(&[
                ("origin", "string"),
                ("minimum", &n.to_string()),
                ("inclusive", &inclusive.to_string()),
            ]),
        }),
        (0u32..1000, any::<bool>()).prop_map(|(n, inclusive)| Code {
            code: IssueCode::TooBig {
                origin: SizeOrigin::Array,
                maximum: json!(n),
                inclusive,
            },
            key: "too_big".into(),
            params: params(&[
                ("origin", "array"),
                ("maximum", &n.to_string()),
                ("inclusive", &inclusive.to_string()),
            ]),
        }),
        (0usize..4, 0usize..4).prop_map(move |(e, r)| Code {
            code: IssueCode::InvalidType {
                expected: kinds[e].into(),
                received: kinds[r].into(),
            },
            key: "invalid_type".into(),
            params: params(&[("expected", kinds[e]), ("received", kinds[r])]),
        }),
        (0usize..20).prop_map(|n| Code {
            code: IssueCode::InvalidLength {
                origin: SizeOrigin::String,
                expected: n,
            },
            key: "invalid_length".into(),
            params: params(&[("origin", "string"), ("expected", &n.to_string())]),
        }),
        Just(Code {
            code: IssueCode::InvalidString {
                validation: StringValidation::Email,
            },
            key: "invalid_string".into(),
            params: params(&[("validation", "email")]),
        }),
        prop::collection::vec("[a-z]{1,4}", 1..4).prop_map(|opts| Code {
            params: params(&[(
                "options",
                &opts
                    .iter()
                    .map(|o| format!("\"{o}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
            )]),
            code: IssueCode::InvalidEnumValue { options: opts },
            key: "invalid_enum_value".into(),
        }),
        Just(Code {
            code: IssueCode::MissingField,
            key: "missing_field".into(),
            params: params(&[]),
        }),
        Just(Code {
            code: IssueCode::NotInt,
            key: "not_int".into(),
            params: params(&[]),
        }),
        "[a-z][a-z_]{0,8}".prop_map(|c| Code {
            params: params(&[("code", &c)]),
            key: c.clone(),
            code: IssueCode::Custom { code: c },
        }),
    ]
}

fn segment() -> impl Strategy<Value = PathSegment> {
    prop_oneof![
        prop::sample::select(vec!["name", "user", "e_mail", "items", "x"])
            .prop_map(|f| PathSegment::Field(f.into())),
        (0usize..30).prop_map(PathSegment::Index),
    ]
}

/// Model of the frontend path: `.field` / `[i]` per segment, without the
/// leading dot.
fn model_path(path: &[PathSegment]) -> String {
    let s: String = path
        .iter()
        .map(|seg| match seg {
            PathSegment::Field(f) => format!(".{f}"),
            PathSegment::Index(i) => format!("[{i}]"),
        })
        .collect();
    s.strip_prefix('.').map(str::to_string).unwrap_or(s)
}

fn issues() -> impl Strategy<Value = Vec<(Vec<PathSegment>, Code, String)>> {
    prop::collection::vec(
        (prop::collection::vec(segment(), 0..4), code(), "\\PC{0,12}"),
        0..6,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn frontend_error_matches_the_model(generated in issues()) {
        let err = VldError {
            issues: generated
                .iter()
                .map(|(path, c, message)| ValidationIssue {
                    code: c.code.clone(),
                    message: message.clone(),
                    path: path.clone(),
                    received: None,
                })
                .collect(),
        };

        // Structured error, through `validate` and through `From`.
        let got = validate::<Echo>(failing(&err)).unwrap_err();
        let from: VldTauriError = err.clone().into();
        prop_assert_eq!(&from.issues, &got.issues);
        prop_assert_eq!(got.error.as_str(), "Validation failed");
        prop_assert_eq!(got.issue_count(), generated.len());
        prop_assert_eq!(got.has_issues(), !generated.is_empty());
        prop_assert_eq!(got.to_string(), format!("Validation failed: {} issue(s)", generated.len()));
        for (issue, (path, c, message)) in got.issues.iter().zip(&generated) {
            prop_assert_eq!(&issue.path, &model_path(path));
            prop_assert_eq!(&issue.message, message);
            prop_assert_eq!(&issue.code, &c.key);
            prop_assert_eq!(&issue.params, &c.params);
        }

        // Inline string, through `VldPayload` deserialization.
        let inline = serde_json::from_value::<VldPayload<Echo>>(failing(&err))
            .unwrap_err()
            .to_string();
        let parts: Vec<String> = generated
            .iter()
            .map(|(path, _, message)| match model_path(path) {
                p if p.is_empty() => message.clone(),
                p => format!("{p}: {message}"),
            })
            .collect();
        prop_assert_eq!(inline, format!("Validation failed: {}", parts.join("; ")));
    }
}
