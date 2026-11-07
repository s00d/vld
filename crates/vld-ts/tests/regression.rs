//! Regression tests for the live end-to-end findings (0.5.0).

use vld::schema::VldSchema;

/// vld applies `with_default` to `null` too; the generated code must as well
/// (checked against zod 3 / valibot 1 with node).
#[test]
fn default_also_replaces_null() {
    let schema = vld::string().min(1).with_default("user".to_string());
    assert_eq!(
        schema.parse_value(&serde_json::Value::Null).unwrap(),
        "user"
    );
    assert_eq!(
        vld_ts::to_zod(&schema),
        r#"z.string().min(1).nullish().transform((v) => v ?? "user")"#
    );
    assert_eq!(
        vld_ts::to_valibot(&schema),
        r#"v.nullish(v.pipe(v.string(), v.minLength(1)), "user")"#
    );
}

// ---------------------------------------------------------------------------
// 0.5.0 re-audit: allOf siblings, regex translation, never, tuples, formats.
// ---------------------------------------------------------------------------

use serde_json::{Value, json};
use vld_ts::{__to_valibot_from_value as valibot_of, __to_zod_from_value as zod_of};

#[test]
fn all_of_next_to_type_keeps_the_typed_base() {
    let s = vld::string().min(3).starts_with("a").ends_with("z");
    assert_eq!(
        vld_ts::to_zod(&s),
        r#"z.string().min(3).regex(new RegExp("^a")).regex(new RegExp("z$"))"#
    );
    assert_eq!(
        vld_ts::to_valibot(&s),
        r#"v.pipe(v.string(), v.minLength(3), v.regex(new RegExp("^a")), v.regex(new RegExp("z$")))"#
    );
    let n = vld::number().multiple_of(3.).multiple_of(5.);
    assert_eq!(vld_ts::to_zod(&n), "z.number().multipleOf(3).multipleOf(5)");
    // Structural parts still intersect with the typed base.
    let mixed = json!({"type": "object", "properties": {}, "allOf": [{"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]}]});
    assert!(zod_of(&mixed).starts_with("z.intersection(z.object({}), z.object("));
}

#[test]
fn rust_regex_is_translated_for_js() {
    let p = |p: &str| zod_of(&json!({"type": "string", "pattern": p}));
    assert_eq!(
        p("(?i)^abc$"),
        r#"z.string().regex(new RegExp("^abc$", "i"))"#
    );
    assert_eq!(p(r"\Aab\z"), r#"z.string().regex(new RegExp("^ab$"))"#);
    assert_eq!(
        p("(?P<y>[0-9]{4})"),
        r#"z.string().regex(new RegExp("(?<y>[0-9]{4})"))"#
    );
    // Not portable: no check, a note instead (never a JS syntax error).
    for bad in [
        "a(?i)b",
        r"\p{L}",
        "[[:alpha:]]",
        "[a&&b]",
        "[]a]",
        r"\x{41}",
    ] {
        let z = p(bad);
        assert!(
            z.starts_with("z.string() /* pattern not portable"),
            "{bad}: {z}"
        );
        assert!(!z.contains(".regex("), "{z}");
    }
    let tricky = p("a*/b[");
    assert!(
        !tricky.contains("*/b"),
        "comment must not end early: {tricky}"
    );
}

#[test]
fn never_is_never() {
    assert_eq!(vld_ts::to_zod(&vld::never()), "z.never()");
    assert_eq!(vld_ts::to_valibot(&vld::never()), "v.never()");
}

#[test]
fn tuples_and_maps() {
    let t = (vld::string(), vld::number());
    assert_eq!(vld_ts::to_zod(&t), "z.tuple([z.string(), z.number()])");
    assert_eq!(
        vld_ts::to_valibot(&t),
        "v.strictTuple([v.string(), v.number()])"
    );
    let m = vld::map(vld::string(), vld::number());
    assert_eq!(
        vld_ts::to_zod(&m),
        "z.array(z.tuple([z.string(), z.number()]))"
    );
    let rest =
        json!({"type": "array", "prefixItems": [{"type": "string"}], "items": {"type": "number"}});
    assert_eq!(zod_of(&rest), "z.tuple([z.string()]).rest(z.number())");
    assert_eq!(
        valibot_of(&rest),
        "v.tupleWithRest([v.string()], v.number())"
    );
}

#[test]
fn refinements_come_after_chained_methods() {
    // A Zod refinement returns ZodEffects (no `.min()`): checks go last.
    let z = vld_ts::to_zod(&vld::string().lowercase().min(2).describe("d"));
    assert_eq!(
        z,
        r#"z.string().min(2).describe("d").refine((s) => s === s.toLowerCase(), "Must be lowercase")"#
    );
}

/// One schema, its generated code and vld's verdict for each sample.
struct Case {
    zod: String,
    valibot: String,
    samples: Vec<(Value, bool)>,
}

fn case<S>(schema: S, samples: Vec<Value>) -> Case
where
    S: VldSchema + vld::openapi::JsonSchema,
{
    Case {
        zod: vld_ts::to_zod(&schema),
        valibot: vld_ts::to_valibot(&schema),
        samples: samples
            .into_iter()
            .map(|s| {
                let ok = schema.parse_value(&s).is_ok();
                (s, ok)
            })
            .collect(),
    }
}

fn node_cases() -> Vec<Case> {
    let s = |v: &[&str]| v.iter().map(|x| json!(x)).collect::<Vec<_>>();
    let long_label = "a".repeat(64);
    vec![
        case(
            vld::string().min(3).starts_with("a").ends_with("z"),
            s(&["abz", "abcz", "az", "bcz", "abc"]),
        ),
        case(
            vld::number().multiple_of(3.).multiple_of(5.),
            vec![json!(15), json!(30), json!(9), json!(10)],
        ),
        case(vld::never(), vec![json!(1), json!(null), json!("x")]),
        case(
            vld::string().regex(regex_lite::Regex::new("(?i)^abc$").unwrap()),
            s(&["abc", "ABC", "abcd"]),
        ),
        case(
            vld::string().regex(regex_lite::Regex::new(r"\Aa+\z").unwrap()),
            s(&["aaa", "Aa", "aab"]),
        ),
        case(
            (vld::string(), vld::number().int()),
            vec![
                json!(["a", 1]),
                json!(["a"]),
                json!(["a", 1, 2]),
                json!([1, "a"]),
            ],
        ),
        case(
            vld::map(vld::string(), vld::boolean()),
            vec![
                json!([["a", true]]),
                json!([["a"]]),
                json!([["a", 1]]),
                json!([]),
            ],
        ),
        case(
            vld::string().iso_datetime(),
            s(&[
                "2020-01-01T10:00:00Z",
                "2020-01-01T10:00:00+03:00",
                "2020-01-01T10:00:00-05:30",
                "2020-01-01T10:00",
                "2020-01-01T10:00:00",
                "2020-01-01T10:00:00.123456789Z",
                "2020-02-30T10:00:00Z",
                "2020-01-01T10:00:00+0300",
                "2020-01-01 10:00:00Z",
                "2020-01-01T25:00:00Z",
            ]),
        ),
        case(
            vld::string().iso_date(),
            s(&[
                "2020-02-29",
                "2021-02-29",
                "2020-04-31",
                "2020-13-01",
                "20-01-01",
            ]),
        ),
        case(
            vld::string().iso_time(),
            s(&["10:00", "10:00:00", "10:00:00.5", "24:00", "10:60", "1:00"]),
        ),
        case(vld::string().email(), s(&["a@b.co", "a@@b.co", "x"])),
        case(
            vld::string().url(),
            s(&[
                "https://a.b/c",
                "http://x",
                "ftp://a.b",
                "mailto:a@b.c",
                "https://",
            ]),
        ),
        case(
            vld::string().uuid_v4(),
            s(&[
                "9f3c4e7a-1b2d-4c3e-8f9a-0b1c2d3e4f5a",
                "9f3c4e7a-1b2d-1c3e-8f9a-0b1c2d3e4f5a",
                "nope",
            ]),
        ),
        case(
            vld::string().ipv4(),
            s(&["1.2.3.4", "256.1.1.1", "01.2.3.4", "::1"]),
        ),
        case(
            vld::string().cidr(),
            s(&[
                "10.0.0.0/8",
                "10.0.0.0/33",
                "::1/128",
                "::1/129",
                "10.0.0.0/08",
                "10.0.0.0",
            ]),
        ),
        case(
            vld::string().hostname(),
            s(&["a.b", "localhost", "-a.b", "a..b", "a_b.c", &long_label]),
        ),
        case(
            vld::string().mac(),
            s(&["aa:bb:cc:dd:ee:ff", "aa:bb:cc:dd:ee", "gg:bb:cc:dd:ee:ff"]),
        ),
        case(vld::string().hex(), s(&["deadBEEF", "", "xyz"])),
        case(vld::string().slug(), s(&["a-b-c", "a--b", "-a", "A", "a-"])),
        case(
            vld::string().semver(),
            s(&["1.2.3", "v1.2.3", " 1.2.3-rc.1 ", "1.2", "1.2.x", "1.2.3+b"]),
        ),
        case(
            vld::string().jwt(),
            s(&["a.b.c", "a.b", "a.b.c=", "a-_.b.c"]),
        ),
        case(vld::string().ascii(), s(&["abc~", "é"])),
        case(vld::string().alpha(), s(&["abc", "ab1"])),
        case(vld::string().alphanumeric(), s(&["ab1", "ab-1"])),
        case(vld::string().lowercase(), s(&["abc", "aBc", "1-2"])),
        case(vld::string().uppercase(), s(&["ABC", "aBC"])),
        case(
            vld::string().locale(),
            s(&["en", "en-US", "EN", "en_US", "en-us"]),
        ),
        case(vld::string().country_code(), s(&["US", "us", "USA"])),
        case(vld::string().currency_code(), s(&["USD", "usd", "US"])),
        case(
            vld::string().cron(),
            s(&["*/5 * * * *", "0 0 * * * *", "* * *", "a * * * *"]),
        ),
        case(
            vld::string().credit_card(),
            s(&[
                "4111 1111 1111 1111",
                "4111-1111-1111-1112",
                "4111",
                "4111x1111111111111",
            ]),
        ),
        case(
            vld::string().phone(),
            s(&[
                "+1 (555) 123-4567",
                "+12345",
                "123456789",
                "+1234567890123456",
            ]),
        ),
        case(
            vld::string().color(),
            s(&[
                "#aabbcc",
                "#AABBCCDD",
                "#abc",
                "rgb(1, 2, 255)",
                "RGB(1,2,3)",
                "rgb(1,2,256)",
                "hsl(360, 100%, 0%)",
                "hsl(361,0%,0%)",
                "hsl(1, 50 %, 1%)",
            ]),
        ),
        case(
            vld::string().base64(),
            s(&["YWJj", "YWI=", "YQ==", "YWJ", "", "Y*=="]),
        ),
        case(
            vld::string().base64url(),
            s(&["YWJj", "YWI", "YW-_", "a", "YW+/"]),
        ),
        case(vld::string().json(), s(&["{\"a\":1}", "[1", "1", ""])),
        case(vld::string().cuid2(), s(&["ab1", "1ab", "a", "aB"])),
        case(
            vld::string().ulid(),
            s(&[
                "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "01ARZ3NDEKTSV4RRFFQ69G5FAI",
                "01arz3ndektsv4rrffq69g5fav",
            ]),
        ),
        case(
            vld::string().nanoid(),
            s(&["V1StGXR8_Z5jdHi6B-myT", "a b", ""]),
        ),
        case(vld::string().emoji(), s(&["😀", "😀a", ""])),
        case(
            vld::string().email().starts_with("ab"),
            s(&["ab@x.io", "b@x.io"]),
        ),
        case(
            vld::array(vld::number())
                .contains(3)
                .min_contains(2)
                .max_contains(3),
            vec![
                json!([3, 3]),
                json!([3]),
                json!([3, 3, 3, 3]),
                json!([1, 3, 3, 3]),
            ],
        ),
        case(
            vld::array(vld::number()).unique(),
            vec![json!([1, 2]), json!([1, 1])],
        ),
        case(animal(), animal_samples()),
        Case {
            zod: <Shape as vld_ts::ToZod>::to_zod(),
            valibot: <Shape as vld_ts::ToValibot>::to_valibot(),
            samples: shape_samples()
                .into_iter()
                .map(|s| {
                    let ok = <Shape as vld::schema::VldParse>::vld_parse_value(&s).is_ok();
                    (s, ok)
                })
                .collect(),
        },
        case(
            vld::array(vld::number()).len(3),
            vec![json!([1, 2, 3]), json!([1, 2]), json!([1, 2, 3, 4])],
        ),
        case(
            vld::record_with_keys(vld::string().min(2), vld::number())
                .min_keys(1)
                .max_keys(2),
            vec![
                json!({"ab": 1}),
                json!({"a": 1}),
                json!({}),
                json!({"ab": 1, "cd": 2, "ef": 3}),
            ],
        ),
    ]
}

/// Runs the generated Zod 3 / Valibot 1 code under node when
/// `VLD_TS_NODE_DIR` points at a directory with `zod@3` and `valibot@1` in
/// `node_modules`, and compares every verdict with vld's. Skipped otherwise.
#[test]
fn generated_code_agrees_with_vld_under_node() {
    let cases = node_cases();
    let Ok(dir) = std::env::var("VLD_TS_NODE_DIR") else {
        return;
    };
    let mut js = String::from(
        "import { z } from 'zod';\nimport * as v from 'valibot';\nlet bad = 0;\nconst cases = [\n",
    );
    for c in &cases {
        let samples: Vec<Value> = c.samples.iter().map(|(s, ok)| json!([s, ok])).collect();
        js.push_str(&format!(
            "  [{}, () => {}, () => {}, {}],\n",
            serde_json::to_string(&c.zod).unwrap(),
            c.zod,
            c.valibot,
            Value::Array(samples)
        ));
    }
    js.push_str(
        "];\n\
         for (const [name, zf, vf, samples] of cases) {\n\
         \x20 let zs, vs;\n\
         \x20 try { zs = zf(); vs = vf(); } catch (e) { bad++; console.log('THROWS', name, String(e)); continue; }\n\
         \x20 for (const [s, ok] of samples) {\n\
         \x20   const zo = zs.safeParse(s).success, vo = v.safeParse(vs, s).success;\n\
         \x20   if (zo !== ok || vo !== ok) { bad++; console.log('MISMATCH', name, JSON.stringify(s), 'vld', ok, 'zod', zo, 'valibot', vo); }\n\
         \x20 }\n\
         }\n\
         console.log('mismatches', bad);\n\
         process.exit(bad ? 1 : 0);\n",
    );
    let path = std::path::Path::new(&dir).join("vld_ts_cross_check.mjs");
    std::fs::write(&path, js).unwrap();
    let out = std::process::Command::new("node")
        .arg(&path)
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Keywords the 0.5 core emits for dates and sets.
#[test]
fn date_bounds_and_unique_counts_become_checks() {
    let d =
        json!({"type": "string", "format": "date", "formatMinimum": "2020-01-01", "x-past": true});
    let z = zod_of(&d);
    assert!(
        z.contains(r#".refine((s) => s >= "2020-01-01", "Must be >= 2020-01-01")"#),
        "{z}"
    );
    assert!(z.contains("Date.parse(s) < Date.now()"), "{z}");
    let set = json!({"type": "array", "items": {"type": "boolean"}, "x-minUniqueItems": 2, "x-maxUniqueItems": 2});
    let v = valibot_of(&set);
    assert!(v.contains(".size >= 2"), "{v}");
    assert!(v.contains(".size <= 2"), "{v}");
    // decimal: `type: ["string", "number"]`.
    let dec = json!({"type": ["string", "number"], "format": "decimal"});
    assert_eq!(zod_of(&dec), "z.union([z.string(), z.number()])");
}

/// vld's naive-allowed datetime: `{type: string, anyOf: [{format}, {pattern}]}`
/// — the sibling `type` / `x-past` apply to every branch.
#[test]
fn any_of_branches_keep_sibling_keywords() {
    let s = json!({
        "type": "string",
        "anyOf": [{"format": "date-time"}, {"pattern": "^a$"}],
        "x-past": true
    });
    let z = zod_of(&s);
    assert!(z.starts_with("z.union([z.string().regex("), "{z}");
    assert!(!z.contains("unknown"), "{z}");
    assert_eq!(z.matches("Date.now()").count(), 2, "{z}");
    // A nullable union is unchanged.
    let n = json!({"anyOf": [{"type": "string"}, {"type": "null"}], "description": "d"});
    assert_eq!(zod_of(&n), r#"z.string().nullable().describe("d")"#);
}

// ---------------------------------------------------------------------------
// User report: discriminated unions, field order, fixed-length arrays.
// ---------------------------------------------------------------------------

fn animal() -> vld::combinators::ZDiscriminatedUnion {
    vld::discriminated_union("type")
        .variant_str(
            "dog",
            vld::object()
                .field("type", vld::literal("dog"))
                .field("bark", vld::boolean()),
        )
        .variant_str(
            "cat",
            vld::object()
                .field("type", vld::literal("cat"))
                .field("lives", vld::number().int()),
        )
}

fn animal_samples() -> Vec<Value> {
    vec![
        json!({"type": "dog", "bark": true}),
        json!({"type": "cat", "lives": 9}),
        json!({"type": "cat", "bark": true}),
        json!({"type": "fish"}),
        json!({"bark": true}),
    ]
}

#[derive(serde::Serialize, serde::Deserialize, vld::Validate)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)]
enum Shape {
    Empty,
    Circle {
        #[vld(vld::number().min(0.0))]
        radius: f64,
    },
    Rect {
        #[vld(vld::number())]
        w: f64,
        #[vld(vld::number())]
        h: f64,
    },
}

vld_ts::impl_to_zod!(Shape);
vld_ts::impl_to_valibot!(Shape);

fn shape_samples() -> Vec<Value> {
    vec![
        json!({"kind": "empty"}),
        json!({"kind": "circle", "radius": 1}),
        json!({"kind": "circle", "radius": -1}),
        json!({"kind": "rect", "w": 1, "h": 2}),
        json!({"kind": "rect", "w": 1}),
        json!({"kind": "square"}),
    ]
}

#[test]
fn tagged_unions_are_discriminated() {
    use vld_ts::{ToValibot, ToZod};
    let z = Shape::to_zod();
    assert!(
        z.starts_with(r#"z.discriminatedUnion("kind", [z.object({"#),
        "{z}"
    );
    let v = Shape::to_valibot();
    assert!(v.starts_with(r#"v.variant("kind", [v.object({"#), "{v}");
    // The tag comes first in each variant, the fields in declaration order.
    assert!(
        z.contains("kind: z.literal(\"rect\"),\n  w: z.number(),\n  h: z.number()"),
        "{z}"
    );
    // vld's `discriminated_union` (a bare `anyOf`): the tag is inferred.
    let z = vld_ts::to_zod(&animal());
    assert!(z.starts_with(r#"z.discriminatedUnion("type", ["#), "{z}");
    let v = vld_ts::to_valibot(&animal());
    assert!(v.starts_with(r#"v.variant("type", ["#), "{v}");
}

#[test]
fn unions_that_are_not_discriminated_stay_unions() {
    let obj = |tag: Value| json!({"type": "object", "properties": {"t": tag}, "required": ["t"]});
    let disc = json!({"propertyName": "t"});
    // A repeated tag value.
    let s = json!({"oneOf": [obj(json!({"const": "a"})), obj(json!({"const": "a"}))], "discriminator": disc});
    assert!(zod_of(&s).starts_with("z.union(["), "{}", zod_of(&s));
    // A non-object option.
    let s =
        json!({"oneOf": [obj(json!({"const": "a"})), {"type": "string"}], "discriminator": disc});
    assert!(valibot_of(&s).starts_with("v.union(["));
    // A non-literal tag.
    let s = json!({"anyOf": [obj(json!({"const": "a"})), obj(json!({"type": "string"}))]});
    assert!(zod_of(&s).starts_with("z.union(["));
    // A single-value enum is a literal too; `null` stays outside.
    let s = json!({"oneOf": [obj(json!({"enum": ["a"]})), obj(json!({"const": "b"})), {"type": "null"}]});
    let z = zod_of(&s);
    assert!(
        z.starts_with(r#"z.discriminatedUnion("t", ["#) && z.ends_with(".nullable()"),
        "{z}"
    );
    // A described option is a `v.pipe`, which `v.variant` does not take.
    let mut described = obj(json!({"const": "b"}));
    described["description"] = json!("B");
    let s = json!({"oneOf": [obj(json!({"const": "a"})), described]});
    assert!(zod_of(&s).starts_with("z.discriminatedUnion("));
    assert!(valibot_of(&s).starts_with("v.union(["));
}

vld::schema! {
    pub struct Ordered {
        pub kind: String => vld::string(),
        pub zeta: String => vld::string(),
        pub alpha: f64 => vld::number(),
    }
}
vld_ts::impl_to_zod!(Ordered);

#[test]
fn fields_keep_schema_order() {
    use vld_ts::ToZod;
    let z = Ordered::to_zod();
    let pos = |k: &str| z.find(k).unwrap_or_else(|| panic!("{k} in {z}"));
    assert!(
        pos("kind") < pos("zeta") && pos("zeta") < pos("alpha"),
        "{z}"
    );
    // `required` gives the order of the required fields; optional ones keep
    // their place in the (sorted) map.
    let s = json!({
        "type": "object",
        "properties": {"z": {"type": "number"}, "m": {"type": "number"}, "a": {"type": "number"}},
        "required": ["z", "a", "gone"]
    });
    assert_eq!(
        valibot_of(&s),
        "v.object({\n  z: v.number(),\n  m: v.optional(v.number()),\n  a: v.number()\n})"
    );
}

#[test]
fn fixed_length_arrays() {
    let a = vld::array(vld::number()).len(6);
    assert_eq!(vld_ts::to_zod(&a), "z.array(z.number()).length(6)");
    assert_eq!(
        vld_ts::to_valibot(&a),
        "v.pipe(v.array(v.number()), v.length(6))"
    );
    // Different bounds are unchanged; tuples keep their form.
    let b = vld::array(vld::number()).min_len(1).max_len(2);
    assert_eq!(vld_ts::to_zod(&b), "z.array(z.number()).min(1).max(2)");
    let t = json!({"type": "array", "prefixItems": [{"type": "number"}], "items": false, "minItems": 1, "maxItems": 1});
    assert_eq!(zod_of(&t), "z.tuple([z.number()])");
    assert_eq!(valibot_of(&t), "v.strictTuple([v.number()])");
}
