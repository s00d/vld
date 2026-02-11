use serde_json::json;
use vld::prelude::*;

#[test]
fn lazy_basic() {
    let schema = vld::lazy(|| vld::string().min(3));
    assert_eq!(schema.parse(r#""hello""#).unwrap(), "hello");
    assert!(schema.parse(r#""hi""#).is_err());
}

#[test]
fn lazy_recursive_tree() {
    fn tree_schema() -> vld::object::ZObject {
        vld::object()
            .field("value", vld::number().int())
            .field("children", vld::array(vld::lazy(tree_schema)))
    }

    let input = r#"{
        "value": 1,
        "children": [
            {"value": 2, "children": []},
            {"value": 3, "children": [
                {"value": 4, "children": []}
            ]}
        ]
    }"#;

    let result = tree_schema().parse(input).unwrap();
    assert_eq!(result["value"], json!(1));
    let children = result["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[1]["children"][0]["value"], json!(4));
}

#[test]
fn lazy_called_each_time() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    let schema = vld::lazy(|| {
        CALL_COUNT.fetch_add(1, Ordering::Relaxed);
        vld::string()
    });

    let before = CALL_COUNT.load(Ordering::Relaxed);
    let _ = schema.parse(r#""a""#);
    let _ = schema.parse(r#""b""#);
    let after = CALL_COUNT.load(Ordering::Relaxed);
    assert_eq!(after - before, 2);
}
