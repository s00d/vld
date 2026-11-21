//! Corpus (differential, optional): schemas with samples, vld's verdict for
//! each, and the generated Zod 3 / Zod 4 / Valibot code run under node must
//! agree. Runs when `VLD_TS_NODE_DIR` is set (see the test's doc comment).

use super::story_typescript_module::Shape;
use serde_json::{Value, json};
use vld::schema::VldSchema;

/// One schema, its generated code and vld's verdict for each sample.
struct Case {
    zod: String,
    zod4: String,
    valibot: String,
    samples: Vec<(Value, bool)>,
}

fn case<S>(schema: S, samples: Vec<Value>) -> Case
where
    S: VldSchema + vld::openapi::JsonSchema,
{
    Case {
        zod: vld::ts::to_zod(&schema),
        zod4: vld::ts::to_zod4(&schema),
        valibot: vld::ts::to_valibot(&schema),
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
            vld::string().pattern("(?i)^abc$"),
            s(&["abc", "ABC", "abcd"]),
        ),
        case(vld::string().pattern(r"\Aa+\z"), s(&["aaa", "Aa", "aab"])),
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
                "2020-01-01T10:00:00.1234567890Z",
                "2020-01-01t10:00:00z",
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
            s(&[
                "10:00",
                "10:00:00",
                "10:00:00.5",
                "10:00:00.1234567890",
                "24:00",
                "10:60",
                "1:00",
            ]),
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
            zod: <Shape as vld::ts::ToZod>::to_zod(),
            zod4: <Shape as vld::ts::ToZod4>::to_zod4(),
            valibot: <Shape as vld::ts::ToValibot>::to_valibot(),
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

/// Runs the generated Zod 3, Zod 4 and Valibot 1 code under node when
/// `VLD_TS_NODE_DIR` points at a directory whose `node_modules` has `zod`
/// (>= 3.25, for the `zod/v3` and `zod/v4` entry points) and, optionally,
/// `valibot@1`, and compares every verdict with vld's. Skipped otherwise.
#[test]
fn generated_code_agrees_with_vld_under_node() {
    let cases = node_cases();
    let Ok(dir) = std::env::var("VLD_TS_NODE_DIR") else {
        return;
    };
    let mut js = String::from(
        "import { z as z3 } from 'zod/v3';\nimport { z as z4 } from 'zod/v4';\n\
         const v = await import('valibot').catch(() => null);\n\
         if (!v) console.log('valibot not installed: Valibot code not checked');\n\
         let bad = 0;\nconst cases = [\n",
    );
    for c in &cases {
        let samples: Vec<Value> = c.samples.iter().map(|(s, ok)| json!([s, ok])).collect();
        js.push_str(&format!(
            "  [{}, (z) => {}, (z) => {}, (v) => {}, {}],\n",
            serde_json::to_string(&c.zod).unwrap(),
            c.zod,
            c.zod4,
            c.valibot,
            Value::Array(samples)
        ));
    }
    js.push_str(
        "];\n\
         for (const [name, z3f, z4f, vf, samples] of cases) {\n\
         \x20 let s3, s4, sv;\n\
         \x20 try { s3 = z3f(z3); s4 = z4f(z4); sv = v && vf(v); } catch (e) { bad++; console.log('THROWS', name, String(e)); continue; }\n\
         \x20 for (const [s, ok] of samples) {\n\
         \x20   const o3 = s3.safeParse(s).success, o4 = s4.safeParse(s).success;\n\
         \x20   const ov = v ? v.safeParse(sv, s).success : ok;\n\
         \x20   if (o3 !== ok || o4 !== ok || ov !== ok) { bad++; console.log('MISMATCH', name, JSON.stringify(s), 'vld', ok, 'zod3', o3, 'zod4', o4, 'valibot', ov); }\n\
         \x20 }\n\
         }\n\
         console.log('cases', cases.length, 'mismatches', bad);\n\
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
