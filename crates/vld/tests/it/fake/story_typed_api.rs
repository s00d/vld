//! Stories: a test suite seeds its fixtures with vld-fake — typed fakes of
//! `vld::schema!` structs (`impl_fake!`), raw JSON values, `$ref`s between
//! types, reproducible seeds, a custom RNG and the recursion limit.

use super::common::{fake_value, seed};
use rand::SeedableRng;
use serde_json::{Value, json};
use vld::fake::{FakeData, FakeGen, fake_value_seeded};

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct SimpleUser {
        pub name:  String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age:   i64    => vld::number().int().min(18).max(99),
    }
}
vld::fake::impl_fake!(SimpleUser);

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct RefAddress {
        pub city: String => vld::string().min(1).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct RefUser {
        pub name:    String     => vld::string().min(2).max(50),
        pub address: RefAddress => vld::nested!(RefAddress),
    }
}
vld::fake::impl_fake!(RefUser);

#[track_caller]
fn assert_user(u: &SimpleUser) {
    assert!((2..=50).contains(&u.name.chars().count()), "{u:?}");
    assert!(u.email.contains('@'), "{u:?}");
    assert!((18..=99).contains(&u.age), "{u:?}");
}

/// Given a `schema!` struct with `impl_fake!`, when a test asks for fakes
/// through every entry point, then each is a valid, typed instance; seeds
/// reproduce, different seeds differ.
#[test]
fn typed_fakes_for_a_schema_struct() {
    assert_user(&SimpleUser::fake());
    let many = SimpleUser::fake_many(10);
    assert_eq!(many.len(), 10);
    many.iter().for_each(assert_user);
    assert_user(&SimpleUser::try_fake().unwrap());

    let a = SimpleUser::fake_seeded(42);
    let b = SimpleUser::fake_seeded(42);
    assert_user(&a);
    assert_eq!((&a.name, &a.email, a.age), (&b.name, &b.email, b.age));
    assert_ne!(
        SimpleUser::fake_seeded(111).name,
        SimpleUser::fake_seeded(222).name
    );

    // The function forms over the struct's JSON Schema.
    let schema = SimpleUser::json_schema();
    assert_user(&vld::fake::fake_parsed::<SimpleUser>(&schema));
    assert_user(&vld::fake::try_fake_parsed::<SimpleUser>(&schema).unwrap());
    assert_user(&vld::fake::parse_or_panic::<SimpleUser>(&fake_value(
        &schema,
    )));
    let v = vld::fake::fake_value(&schema);
    assert_user(&SimpleUser::parse_value(&v).unwrap());
    let text = vld::fake::fake_json(&schema);
    assert_user(&SimpleUser::parse_value(&serde_json::from_str::<Value>(&text).unwrap()).unwrap());
    let values = vld::fake::fake_many(&schema, 7);
    assert_eq!(values.len(), 7);
    for v in &values {
        assert_user(&SimpleUser::parse_value(v).unwrap());
    }
    assert_eq!(
        fake_value_seeded(&schema, 12345),
        fake_value_seeded(&schema, 12345)
    );
    assert_ne!(
        fake_value_seeded(&schema, 11111),
        fake_value_seeded(&schema, 22222)
    );
}

/// Given an unsatisfiable schema (a fake cannot pass it), when a test asks for
/// typed fakes, then `try_*` report the issues and the panicking forms panic
/// with the generated value.
#[test]
fn unsatisfiable_schemas_are_reported() {
    vld::schema! {
        #[derive(Debug)]
        pub struct Never {
            pub n: i64 => vld::number().int().min(5).max(1),
        }
    }
    vld::fake::impl_fake!(Never);
    let err = Never::try_fake().unwrap_err();
    assert_eq!(
        err.issues[0]
            .path
            .iter()
            .map(ToString::to_string)
            .collect::<String>(),
        ".n"
    );
    assert!(vld::fake::try_fake_parsed::<Never>(&Never::json_schema()).is_err());
    let panic = std::panic::catch_unwind(Never::fake).unwrap_err();
    let text = panic.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(text.contains("generated value failed validation"), "{text}");
    assert!(std::panic::catch_unwind(|| Never::fake_seeded(1)).is_err());
    assert!(std::panic::catch_unwind(|| Never::fake_many(1)).is_err());
}

/// Given a struct with a `vld::nested!` field (`#/components/schemas/X`), when
/// typed fakes are generated, then the nested definitions are embedded and the
/// `$ref` resolves to a valid nested value; raw schemas resolve `$defs`,
/// `definitions` and `components.schemas` refs too.
#[test]
fn refs_between_types_resolve() {
    assert!(RefUser::json_schema()["properties"]["address"]["$ref"].is_string());
    for _ in 0..20 {
        let u = RefUser::fake_seeded(seed::next());
        assert!(!u.address.city.is_empty());
    }
    assert!(!RefUser::fake().address.city.is_empty());
    assert_eq!(RefUser::fake_many(3).len(), 3);
    assert!(RefUser::try_fake().is_ok());

    // with_nested_defs embeds the nested schemas under $defs (and only then).
    let embedded = vld::fake::with_nested_defs(
        json!({"type": "object"}),
        vec![("A", || json!({"type": "string"}))],
    );
    assert_eq!(embedded["$defs"]["A"], json!({"type": "string"}));
    assert_eq!(
        vld::fake::with_nested_defs(json!({"type": "object"}), vec![]),
        json!({"type": "object"})
    );
    assert_eq!(
        vld::fake::with_nested_defs(json!(true), vec![("A", || json!({}))]),
        json!(true)
    );

    let target = json!({"type": "object", "required": ["n"], "properties": {"n": {"type": "integer", "minimum": 1, "maximum": 3}}});
    for (root, r) in [
        ("$defs", "#/$defs/T"),
        ("definitions", "#/definitions/T"),
        ("$defs", "#/components/schemas/T"),
    ] {
        let mut schema =
            json!({"type": "object", "required": ["t"], "properties": {"t": {"$ref": r}}});
        schema[root] = json!({ "T": target });
        let v = fake_value(&schema);
        let n = v["t"]["n"].as_i64().expect("ref resolved to object");
        assert!((1..=3).contains(&n), "{v}");
    }
    let doc = json!({"components": {"schemas": {"T": target}}, "$ref": "#/components/schemas/T"});
    assert!(fake_value(&doc)["n"].is_i64());
}

/// Given a custom seeded RNG and a 20-level deep schema, when a `FakeGen`
/// generates values, then it honours the schema, matches `fake_value_seeded`
/// for the same seed, and stops recursing after 13 levels (the 14th is
/// `null`) while 5 levels are generated in full.
#[test]
fn a_custom_generator_and_the_depth_limit() {
    let schema = json!({"type": "string", "minLength": 5, "maxLength": 10});
    let mut generator = FakeGen::with_rng(rand::rngs::StdRng::seed_from_u64(42));
    let v = generator.value(&schema);
    assert!((5..=10).contains(&v.as_str().unwrap().len()), "{v}");
    let mut generator = FakeGen::with_rng(rand::rngs::StdRng::seed_from_u64(42));
    assert_eq!(generator.value(&schema), fake_value_seeded(&schema, 42));
    assert!(FakeGen::new().value(&schema).is_string());
    assert!(FakeGen::default().value(&schema).is_string());

    fn deep(depth: usize) -> Value {
        if depth == 0 {
            json!({"type": "string"})
        } else {
            json!({"type": "object", "required": ["child"], "properties": {"child": deep(depth - 1)}})
        }
    }
    /// Objects along the `child` chain, and the value where the chain ends.
    fn chain(mut v: &Value) -> (usize, &Value) {
        let mut objects = 0;
        while let Some(child) = v.get("child") {
            objects += 1;
            v = child;
        }
        (objects, v)
    }
    let full = fake_value(&deep(5));
    let (objects, leaf) = chain(&full);
    assert_eq!(objects, 5, "{full}");
    assert!(leaf.is_string(), "{full}");
    let cut = fake_value(&deep(20));
    let (objects, leaf) = chain(&cut);
    assert_eq!(objects, 13, "{cut}");
    assert!(leaf.is_null(), "{cut}");
    // The generator is reusable after hitting the limit.
    let mut generator = FakeGen::with_rng(rand::rngs::StdRng::seed_from_u64(7));
    let _ = generator.value(&deep(20));
    assert_eq!(chain(&generator.value(&deep(3))).0, 3);
}

/// Given a realistic profile schema of untyped fields, when one fake is
/// generated, then every field is present and the field names pick realistic
/// generators.
#[test]
fn a_realistic_profile() {
    let schema = json!({
        "type": "object",
        "required": [
            "id", "username", "email", "first_name", "last_name",
            "phone", "company", "job_title", "city", "country",
            "website", "bio", "tags"
        ],
        "properties": {
            "id": {"type": "string"}, "username": {"type": "string"},
            "email": {"type": "string"}, "first_name": {"type": "string"},
            "last_name": {"type": "string"}, "phone": {"type": "string"},
            "company": {"type": "string"}, "job_title": {"type": "string"},
            "city": {"type": "string"}, "country": {"type": "string"},
            "website": {"type": "string"}, "bio": {"type": "string"},
            "tags": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 5}
        }
    });
    let v = fake_value(&schema);
    assert_eq!(v.as_object().unwrap().len(), 13);
    assert!(v["id"].as_str().unwrap().contains('-'));
    assert!(v["email"].as_str().unwrap().contains('@'));
    assert!(v["phone"].as_str().unwrap().starts_with('+'));
    assert!(v["website"].as_str().unwrap().starts_with("http"));
    assert!(v["bio"].as_str().unwrap().ends_with('.'));
    assert!(!v["tags"].as_array().unwrap().is_empty());
}
