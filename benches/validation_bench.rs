use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vld::prelude::*;

// -----------------------------------------------------------------------
// Existing benchmarks
// -----------------------------------------------------------------------

fn bench_string_parse(c: &mut Criterion) {
    let schema = vld::string().min(3).max(50).email();
    c.bench_function("string_email_valid", |b| {
        b.iter(|| schema.parse(black_box(r#""test@example.com""#)))
    });
    c.bench_function("string_email_invalid", |b| {
        b.iter(|| schema.parse(black_box(r#""nope""#)))
    });
}

fn bench_number_parse(c: &mut Criterion) {
    let schema = vld::number().min(0.0).max(100.0);
    c.bench_function("number_valid", |b| {
        b.iter(|| schema.parse(black_box("42.5")))
    });
    let int_schema = vld::number().int().min(0).max(1000);
    c.bench_function("int_valid", |b| {
        b.iter(|| int_schema.parse(black_box("500")))
    });
}

fn bench_object_parse(c: &mut Criterion) {
    let schema = vld::object()
        .field("name", vld::string().min(1))
        .field("email", vld::string().email())
        .field("age", vld::number().int().min(0));

    let input = r#"{"name": "Alex", "email": "alex@example.com", "age": 30}"#;
    c.bench_function("object_3_fields", |b| {
        b.iter(|| schema.parse(black_box(input)))
    });
}

fn bench_array_parse(c: &mut Criterion) {
    let schema = vld::array(vld::number().int().positive());
    let input = "[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]";
    c.bench_function("array_10_ints", |b| {
        b.iter(|| schema.parse(black_box(input)))
    });
}

fn bench_schema_macro(c: &mut Criterion) {
    vld::schema! {
        #[derive(Debug)]
        struct BenchUser {
            name: String => vld::string().min(2).max(50),
            email: String => vld::string().email(),
            age: Option<i64> => vld::number().int().gte(18).optional(),
        }
    }

    let input = r#"{"name": "Alex", "email": "alex@example.com", "age": 30}"#;
    c.bench_function("schema_macro_valid", |b| {
        b.iter(|| BenchUser::parse(black_box(input)))
    });

    let bad_input = r#"{"name": "A", "email": "bad", "age": 5}"#;
    c.bench_function("schema_macro_invalid", |b| {
        b.iter(|| BenchUser::parse(black_box(bad_input)))
    });
}

// -----------------------------------------------------------------------
// NEW: Nested object (3 levels deep)
// -----------------------------------------------------------------------

fn bench_nested_object(c: &mut Criterion) {
    vld::schema! {
        #[derive(Debug)]
        struct BenchAddress {
            street: String => vld::string().min(1).max(200),
            city: String   => vld::string().min(1).max(100),
            zip: String    => vld::string().len(5),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        struct BenchCompany {
            name: String         => vld::string().min(1).max(100),
            address: BenchAddress => vld::nested(BenchAddress::parse_value),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        struct BenchEmployee {
            first_name: String      => vld::string().min(1).max(50),
            last_name: String       => vld::string().min(1).max(50),
            email: String           => vld::string().email(),
            age: i64                => vld::number().int().min(18).max(120),
            company: BenchCompany   => vld::nested(BenchCompany::parse_value),
        }
    }

    let input = r#"{
        "first_name": "John",
        "last_name": "Doe",
        "email": "john@acme.com",
        "age": 35,
        "company": {
            "name": "Acme Inc.",
            "address": {
                "street": "123 Main St",
                "city": "Metropolis",
                "zip": "12345"
            }
        }
    }"#;

    c.bench_function("nested_3_levels_valid", |b| {
        b.iter(|| BenchEmployee::parse(black_box(input)))
    });

    let bad_input = r#"{
        "first_name": "",
        "last_name": "D",
        "email": "bad",
        "age": 10,
        "company": {
            "name": "",
            "address": { "street": "", "city": "", "zip": "1" }
        }
    }"#;

    c.bench_function("nested_3_levels_invalid", |b| {
        b.iter(|| BenchEmployee::parse(black_box(bad_input)))
    });
}

// -----------------------------------------------------------------------
// NEW: Large array (1000 elements)
// -----------------------------------------------------------------------

fn bench_large_array(c: &mut Criterion) {
    let schema = vld::array(vld::number().int().min(0).max(10000));

    // Build a JSON array of 1000 ints
    let nums: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    let input = format!("[{}]", nums.join(","));

    c.bench_function("array_1000_ints_valid", |b| {
        let val: serde_json::Value = serde_json::from_str(&input).unwrap();
        b.iter(|| schema.parse_value(black_box(&val)))
    });

    // All strings — all fail
    let bad: Vec<String> = (0..1000).map(|i| format!("\"s{}\"", i)).collect();
    let bad_input = format!("[{}]", bad.join(","));

    c.bench_function("array_1000_strings_invalid", |b| {
        let val: serde_json::Value = serde_json::from_str(&bad_input).unwrap();
        b.iter(|| schema.parse_value(black_box(&val)))
    });

    // Mixed schema: array of objects
    let obj_schema = vld::array(
        vld::object()
            .field("id", vld::number().int())
            .field("name", vld::string().min(1)),
    );
    let objs: Vec<String> = (0..1000)
        .map(|i| format!(r#"{{"id":{},"name":"user{}"}}"#, i, i))
        .collect();
    let obj_input = format!("[{}]", objs.join(","));

    c.bench_function("array_1000_objects_valid", |b| {
        let val: serde_json::Value = serde_json::from_str(&obj_input).unwrap();
        b.iter(|| obj_schema.parse_value(black_box(&val)))
    });
}

// -----------------------------------------------------------------------
// NEW: Discriminated union
// -----------------------------------------------------------------------

fn bench_discriminated_union(c: &mut Criterion) {
    let schema = vld::discriminated_union("type")
        .variant(
            "email",
            vld::object()
                .field("type", vld::literal("email"))
                .field("address", vld::string().email()),
        )
        .variant(
            "sms",
            vld::object()
                .field("type", vld::literal("sms"))
                .field("phone", vld::string().min(10)),
        )
        .variant(
            "push",
            vld::object()
                .field("type", vld::literal("push"))
                .field("token", vld::string().min(20)),
        );

    c.bench_function("discrim_union_first_variant", |b| {
        b.iter(|| schema.parse(black_box(r#"{"type":"email","address":"x@y.com"}"#)))
    });

    c.bench_function("discrim_union_last_variant", |b| {
        b.iter(|| {
            schema.parse(black_box(
                r#"{"type":"push","token":"abcdefghijklmnopqrstu"}"#,
            ))
        })
    });

    c.bench_function("discrim_union_unknown_variant", |b| {
        b.iter(|| schema.parse(black_box(r#"{"type":"pigeon","msg":"coo"}"#)))
    });

    // Plain union (non-discriminated) for comparison
    let plain_union = vld::union(vld::string(), vld::union(vld::number(), vld::boolean()));
    c.bench_function("union_3_types_string", |b| {
        b.iter(|| plain_union.parse(black_box(r#""hello""#)))
    });
    c.bench_function("union_3_types_bool", |b| {
        b.iter(|| plain_union.parse(black_box("true")))
    });
}

// -----------------------------------------------------------------------
// NEW: Coercion benchmarks
// -----------------------------------------------------------------------

fn bench_coercion(c: &mut Criterion) {
    // String coercion: number → string
    let str_coerce = vld::string().coerce();
    c.bench_function("coerce_number_to_string", |b| {
        b.iter(|| str_coerce.parse(black_box("42")))
    });
    c.bench_function("coerce_bool_to_string", |b| {
        b.iter(|| str_coerce.parse(black_box("true")))
    });
    c.bench_function("coerce_string_passthrough", |b| {
        b.iter(|| str_coerce.parse(black_box(r#""hello""#)))
    });

    // Number coercion: string → number
    let num_coerce = vld::number().coerce();
    c.bench_function("coerce_string_to_number", |b| {
        b.iter(|| num_coerce.parse(black_box(r#""42.5""#)))
    });
    c.bench_function("coerce_number_passthrough", |b| {
        b.iter(|| num_coerce.parse(black_box("42.5")))
    });

    // Boolean coercion: string → bool
    let bool_coerce = vld::boolean().coerce();
    c.bench_function("coerce_string_to_bool", |b| {
        b.iter(|| bool_coerce.parse(black_box(r#""true""#)))
    });
    c.bench_function("coerce_bool_passthrough", |b| {
        b.iter(|| bool_coerce.parse(black_box("true")))
    });
}

// -----------------------------------------------------------------------
// NEW: Conditional validation (when)
// -----------------------------------------------------------------------

fn bench_conditional(c: &mut Criterion) {
    let schema = vld::object()
        .field("role", vld::string())
        .field_optional("admin_key", vld::string())
        .when("role", "admin", "admin_key", vld::string().min(10));

    c.bench_function("conditional_when_matched", |b| {
        b.iter(|| {
            schema.parse(black_box(
                r#"{"role":"admin","admin_key":"super-secret-admin-key"}"#,
            ))
        })
    });

    c.bench_function("conditional_when_skipped", |b| {
        b.iter(|| schema.parse(black_box(r#"{"role":"user"}"#)))
    });
}

// -----------------------------------------------------------------------
// NEW: Error formatting
// -----------------------------------------------------------------------

fn bench_error_formatting(c: &mut Criterion) {
    vld::schema! {
        #[derive(Debug)]
        struct ErrorUser {
            name: String => vld::string().min(2).max(50),
            email: String => vld::string().email(),
            age: i64 => vld::number().int().min(0).max(150),
        }
    }

    let bad = r#"{"name":"","email":"bad","age":-1}"#;
    let err = ErrorUser::parse(bad).unwrap_err();

    c.bench_function("error_display", |b| {
        b.iter(|| format!("{}", black_box(&err)))
    });
    c.bench_function("error_prettify", |b| {
        b.iter(|| vld::format::prettify_error(black_box(&err)))
    });
    c.bench_function("error_flatten", |b| {
        b.iter(|| vld::format::flatten_error(black_box(&err)))
    });
}

// -----------------------------------------------------------------------
// Register all benchmark groups
// -----------------------------------------------------------------------

criterion_group!(
    benches,
    // Original
    bench_string_parse,
    bench_number_parse,
    bench_object_parse,
    bench_array_parse,
    bench_schema_macro,
    // New
    bench_nested_object,
    bench_large_array,
    bench_discriminated_union,
    bench_coercion,
    bench_conditional,
    bench_error_formatting,
);
criterion_main!(benches);
