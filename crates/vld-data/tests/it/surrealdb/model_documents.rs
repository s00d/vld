//! Model: random nested documents through `validate_records`,
//! `validate_json` and `VldSurrealResponse`.
//!
//! proptest draws batches of `{tags: [..], addr: {city}}` documents with
//! empty and non-empty tags and cities of any length. The model predicts each
//! document's `(path, code)` issues (`.tags[i] too_small` per empty tag, then
//! `.addr.city too_small` for a city shorter than 2 chars), the index of the
//! first bad document in a batch, and the API response's field paths (the
//! same paths without the leading dot).

use super::common::*;
use proptest::prelude::*;
use serde_json::json;
use vld_data::surrealdb::prelude::*;

#[derive(Debug, Clone, serde::Serialize)]
struct Addr {
    city: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct Doc {
    tags: Vec<String>,
    addr: Addr,
}

fn doc() -> impl Strategy<Value = Doc> {
    let tag = prop_oneof![Just(String::new()), "[a-z]{1,4}"];
    (prop::collection::vec(tag, 0..4), "\\PC{0,3}").prop_map(|(tags, city)| Doc {
        tags,
        addr: Addr { city },
    })
}

fn model(d: &Doc) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = d
        .tags
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_empty())
        .map(|(i, _)| (format!(".tags[{i}]"), "too_small".into()))
        .collect();
    if d.addr.city.chars().count() < 2 {
        out.push((".addr.city".into(), "too_small".into()));
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn model_document_batches(docs in prop::collection::vec(doc(), 0..5)) {
        let predicted: Vec<_> = docs.iter().map(model).collect();
        let first_bad = predicted.iter().position(|p| !p.is_empty());

        match (validate_records::<OuterDoc, _>(&docs), first_bad) {
            (Ok(()), None) => {}
            (Err((i, err)), Some(want)) => {
                prop_assert_eq!(i, want);
                prop_assert_eq!(issues_of(&err), predicted[want].clone());
            }
            (got, want) => prop_assert!(false, "got {got:?}, model says first bad = {want:?}"),
        }

        for (d, want) in docs.iter().zip(&predicted) {
            let as_json = json!(d);
            let got = validate_json::<OuterDoc>(&as_json);
            if want.is_empty() {
                prop_assert!(got.is_ok(), "{got:?}");
                continue;
            }
            let err = got.unwrap_err();
            prop_assert_eq!(&issues_of(&err), want);
            let resp = VldSurrealResponse::from_error(&err);
            prop_assert_eq!(&resp.error, "Validation failed");
            let fields: Vec<String> = resp.fields.iter().map(|f| f.field.clone()).collect();
            let paths: Vec<String> = want.iter().map(|(p, _)| p[1..].to_string()).collect();
            prop_assert_eq!(fields, paths);
        }
    }
}
