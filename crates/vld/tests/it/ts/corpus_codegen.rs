//! Corpus: JSON Schemas (hand written and emitted by vld) and the code each
//! generator must produce for them — exactly, or the fragments that matter.

use super::common::{Target, Target::*, generate as emit};
use serde_json::{Value, json};
use vld::openapi::JsonSchema;
use vld::schema::VldSchema as _;

enum Want {
    Eq(&'static str),
    Starts(&'static str),
    Ends(&'static str),
    Has(&'static str),
    Lacks(&'static str),
}
use Want::*;

fn obj(tag: Value) -> Value {
    json!({"type": "object", "properties": {"t": tag}, "required": ["t"]})
}

fn rows() -> Vec<(Value, Vec<(Target, Want)>)> {
    let animal = vld::discriminated_union("type")
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
        );
    let disc = json!({"propertyName": "t"});
    let mut described = obj(json!({"const": "b"}));
    described["description"] = json!("B");
    let p = |p: &str| json!({"type": "string", "pattern": p});
    let mut rows = vec![
        // strings, numbers, nullable, unions
        (
            vld::string().min(2).max(50).email().json_schema(),
            vec![
                (Zod, Eq("z.string().min(2).max(50).email()")),
                (Zod4, Eq("z.email().min(2).max(50)")),
                (
                    Valibot,
                    Eq("v.pipe(v.string(), v.minLength(2), v.maxLength(50), v.email())"),
                ),
                (Ts, Eq("string")),
            ],
        ),
        (
            vld::string().min(2).email().json_schema(),
            vec![(Valibot, Eq("v.pipe(v.string(), v.minLength(2), v.email())"))],
        ),
        (
            vld::number().int().min(0).max(100).json_schema(),
            vec![
                (Zod, Eq("z.number().int().min(0).max(100)")),
                (Zod4, Eq("z.int().min(0).max(100)")),
                (Ts, Eq("number")),
            ],
        ),
        (
            vld::number().int().min(1).json_schema(),
            vec![(Zod4, Eq("z.int().min(1)"))],
        ),
        (
            vld::string().nullable().json_schema(),
            vec![
                (Zod, Eq("z.string().nullable()")),
                (Ts, Eq("string | null")),
            ],
        ),
        (
            vld::union(vld::string(), vld::number()).json_schema(),
            vec![(Zod, Eq("z.union([z.string(), z.number()])"))],
        ),
        (vld::boolean().json_schema(), vec![(Ts, Eq("boolean"))]),
        (vld::string().json_schema(), vec![(Ts, Eq("string"))]),
        // escaping
        (
            vld::enumeration(&["a\"b", "c\\d"]).json_schema(),
            vec![
                (
                    Zod,
                    Eq(r#"z.union([z.literal("a\"b"), z.literal("c\\d")])"#),
                ),
                (
                    Valibot,
                    Eq(r#"v.union([v.literal("a\"b"), v.literal("c\\d")])"#),
                ),
            ],
        ),
        (
            vld::enumeration(&["a\"b", "c"]).json_schema(),
            vec![(Ts, Eq("\"a\\\"b\" | \"c\""))],
        ),
        (
            json!({"type": "string", "pattern": "^a/b\"c$"}),
            vec![
                (Zod, Eq(r#"z.string().regex(new RegExp("^a/b\"c$"))"#)),
                (
                    Valibot,
                    Eq(r#"v.pipe(v.string(), v.regex(new RegExp("^a/b\"c$")))"#),
                ),
            ],
        ),
        (
            vld::string()
                .describe("line1\nback\\slash \"q\"")
                .json_schema(),
            vec![(
                Zod,
                Eq(r#"z.string().describe("line1\nback\\slash \"q\"")"#),
            )],
        ),
        (
            json!({"type": "object", "required": ["first-name", "ok_1"], "properties": {"first-name": {"type": "string"}, "ok_1": {"type": "string"}}}),
            vec![
                (Zod, Has("  \"first-name\": z.string()")),
                (Zod, Has("  ok_1: z.string()")),
                (Valibot, Has("  \"first-name\": v.string()")),
            ],
        ),
        // defaults take null too, and make a field optional on input
        (
            vld::number().int().with_default(5).json_schema(),
            vec![
                (
                    Zod,
                    Eq("z.number().int().nullish().transform((v) => v ?? 5)"),
                ),
                (Valibot, Eq("v.nullish(v.pipe(v.number(), v.integer()), 5)")),
            ],
        ),
        (
            vld::string()
                .min(1)
                .with_default("user".to_string())
                .json_schema(),
            vec![
                (
                    Zod,
                    Eq(r#"z.string().min(1).nullish().transform((v) => v ?? "user")"#),
                ),
                (
                    Valibot,
                    Eq(r#"v.nullish(v.pipe(v.string(), v.minLength(1)), "user")"#),
                ),
            ],
        ),
        (
            json!({"type": "object", "required": [], "properties": {"role": {"type": "string", "default": "user"}}}),
            vec![
                (
                    Zod,
                    Has("role: z.string().nullish().transform((v) => v ?? \"user\")\n"),
                ),
                (Valibot, Has("role: v.nullish(v.string(), \"user\")\n")),
            ],
        ),
        // descriptions on every kind
        (
            vld::string().nullable().describe("N").json_schema(),
            vec![(Zod, Eq("z.string().nullable().describe(\"N\")"))],
        ),
        (
            json!({"$ref": "#/components/schemas/A", "description": "R"}),
            vec![
                (Zod, Eq("z.lazy(() => ASchema).describe(\"R\")")),
                (
                    Valibot,
                    Eq("v.pipe(v.lazy(() => ASchema), v.description(\"R\"))"),
                ),
            ],
        ),
        (
            vld::enumeration(&["a"]).describe("E").json_schema(),
            vec![(Zod, Eq("z.literal(\"a\").describe(\"E\")"))],
        ),
        (
            json!({"allOf": [{"type": "string"}, {"type": "number"}], "description": "I"}),
            vec![(
                Zod,
                Eq("z.intersection(z.string(), z.number()).describe(\"I\")"),
            )],
        ),
        (
            vld::boolean().describe("B").json_schema(),
            vec![(Zod, Eq("z.boolean().describe(\"B\")"))],
        ),
        (
            json!({"type": "null", "description": "Z"}),
            vec![(Zod, Eq("z.null().describe(\"Z\")"))],
        ),
        // anyOf like oneOf; type arrays
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            vec![
                (Zod, Eq("z.string().nullable()")),
                (Valibot, Eq("v.nullable(v.string())")),
            ],
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
            vec![(Zod, Eq("z.union([z.string(), z.number()])"))],
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}], "description": "d"}),
            vec![(Zod, Eq(r#"z.string().nullable().describe("d")"#))],
        ),
        (
            json!({"anyOf": [{"type": "null"}]}),
            vec![(Zod, Eq("z.null()")), (Valibot, Eq("v.null()"))],
        ),
        (
            json!({"oneOf": []}),
            vec![(Zod, Eq("z.never()")), (Ts, Eq("never"))],
        ),
        (
            json!({"type": ["string", "null"], "minLength": 1}),
            vec![
                (Zod, Eq("z.string().min(1).nullable()")),
                (
                    Valibot,
                    Eq("v.nullable(v.pipe(v.string(), v.minLength(1)))"),
                ),
            ],
        ),
        (
            json!({"type": ["string", "integer"]}),
            vec![(Zod, Eq("z.union([z.string(), z.number().int()])"))],
        ),
        (
            json!({"type": ["string", "number"], "format": "decimal"}),
            vec![(Zod, Eq("z.union([z.string(), z.number()])"))],
        ),
        (
            json!({"type": ["integer", "null"]}),
            vec![(Ts, Eq("number | null"))],
        ),
        (json!({"type": ["null"]}), vec![(Zod, Eq("z.null()"))]),
        (
            json!({"type": "weird"}),
            vec![(Zod, Eq("z.unknown()")), (Ts, Eq("unknown"))],
        ),
        (
            json!({}),
            vec![
                (Zod, Eq("z.unknown()")),
                (Valibot, Eq("v.unknown()")),
                (Ts, Eq("unknown")),
            ],
        ),
        (
            json!(true),
            vec![(Zod, Eq("z.unknown()")), (Ts, Eq("unknown"))],
        ),
        (
            json!(false),
            vec![(Zod, Eq("z.never()")), (Ts, Eq("never"))],
        ),
        (
            json!({"const": 3}),
            vec![(Ts, Eq("3")), (Zod, Eq("z.literal(3)"))],
        ),
        (
            json!({"const": null}),
            vec![(Zod, Eq("z.null()")), (Ts, Eq("null"))],
        ),
        (
            json!({"const": [1]}),
            vec![(Zod, Eq("z.unknown()")), (Ts, Eq("unknown"))],
        ),
        // unknown keys
        (
            json!({"type": "object", "required": ["a"], "properties": {"a": {"type": "string"}}, "additionalProperties": {"type": "number"}}),
            vec![
                (Zod, Ends(".catchall(z.number())")),
                (Zod4, Ends(".catchall(z.number())")),
                (Valibot, Starts("v.objectWithRest(")),
                (Valibot, Ends(", v.number())")),
            ],
        ),
        (
            json!({"type": "object", "required": ["a"], "properties": {"a": {"type": "string"}}, "additionalProperties": true}),
            vec![
                (Zod, Eq("z.object({\n  a: z.string()\n})")),
                (Valibot, Eq("v.object({\n  a: v.string()\n})")),
            ],
        ),
        (
            vld::object()
                .field("a", vld::string())
                .strict()
                .json_schema(),
            vec![
                (Zod, Ends(".strict()")),
                (Valibot, Starts("v.strictObject(")),
            ],
        ),
        (
            json!({"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"], "additionalProperties": false}),
            vec![(Zod4, Eq("z.strictObject({\n  a: z.string()\n})"))],
        ),
        (
            json!({"type": "object"}),
            vec![
                (Zod, Eq("z.object({}).passthrough()")),
                (Zod4, Eq("z.looseObject({})")),
                (Valibot, Eq("v.objectWithRest({}, v.unknown())")),
                (Ts, Eq("Record<string, unknown>")),
            ],
        ),
        (
            json!({"type": "object", "additionalProperties": false}),
            vec![
                (Zod, Eq("z.object({}).strict()")),
                (Zod4, Eq("z.strictObject({})")),
                (Valibot, Eq("v.strictObject({})")),
                (Ts, Eq("Record<string, never>")),
            ],
        ),
        // records
        (
            json!({"type": "object", "additionalProperties": {"type": "integer"}}),
            vec![
                (Zod4, Eq("z.record(z.string(), z.int())")),
                (Zod, Eq("z.record(z.string(), z.number().int())")),
            ],
        ),
        (
            json!({"type": "object", "additionalProperties": {"type": "string"}}),
            vec![(Ts, Eq("Record<string, string>"))],
        ),
        (
            json!({"type": "object", "propertyNames": {"minLength": 2}}),
            vec![
                (Zod, Eq("z.record(z.string().min(2), z.unknown())")),
                (
                    Valibot,
                    Eq("v.record(v.pipe(v.string(), v.minLength(2)), v.unknown())"),
                ),
            ],
        ),
        (
            json!({"type": "object", "propertyNames": {"type": "string", "maxLength": 3}, "additionalProperties": {"type": "boolean"}}),
            vec![(Zod, Eq("z.record(z.string().max(3), z.boolean())"))],
        ),
        (
            json!({"type": "object", "properties": {"a b": {"type": "string"}}, "additionalProperties": {"type": "number"}}),
            vec![(Ts, Eq("{\n  \"a b\"?: string;\n} & Record<string, number>"))],
        ),
        (
            json!({"type": "object", "minProperties": 1, "maxProperties": 2, "additionalProperties": {"type": "number"}}),
            vec![
                (
                    Zod,
                    Has(".refine((o) => Object.keys(o).length >= 1, \"Must have >= 1 keys\")"),
                ),
                (
                    Zod4,
                    Has(
                        ".refine((o) => Object.keys(o).length <= 2, { error: \"Must have <= 2 keys\" })",
                    ),
                ),
                (Valibot, Has("v.minEntries(1), v.maxEntries(2)")),
            ],
        ),
        // arrays and tuples
        (
            vld::array(vld::string())
                .min_len(1)
                .max_len(3)
                .json_schema(),
            vec![(
                Valibot,
                Eq("v.pipe(v.array(v.string()), v.minLength(1), v.maxLength(3))"),
            )],
        ),
        (
            vld::array(vld::number()).len(6).json_schema(),
            vec![
                (Zod, Eq("z.array(z.number()).length(6)")),
                (Valibot, Eq("v.pipe(v.array(v.number()), v.length(6))")),
            ],
        ),
        (
            vld::array(vld::number())
                .min_len(1)
                .max_len(2)
                .json_schema(),
            vec![(Zod, Eq("z.array(z.number()).min(1).max(2)"))],
        ),
        (
            json!({"type": "array"}),
            vec![(Zod, Eq("z.array(z.unknown())")), (Ts, Eq("unknown[]"))],
        ),
        (
            vld::array(vld::union(vld::string(), vld::number())).json_schema(),
            vec![(Ts, Eq("(string | number)[]"))],
        ),
        (
            json!({"type": "array", "prefixItems": [{"type": "number"}], "items": false, "minItems": 1, "maxItems": 1}),
            vec![
                (Zod, Eq("z.tuple([z.number()])")),
                (Valibot, Eq("v.strictTuple([v.number()])")),
            ],
        ),
        (
            (vld::string(), vld::number()).json_schema(),
            vec![
                (Zod, Eq("z.tuple([z.string(), z.number()])")),
                (Zod4, Eq("z.tuple([z.string(), z.number()])")),
                (Valibot, Eq("v.strictTuple([v.string(), v.number()])")),
            ],
        ),
        (
            vld::map(vld::string(), vld::number()).json_schema(),
            vec![(Zod, Eq("z.array(z.tuple([z.string(), z.number()]))"))],
        ),
        (
            json!({"type": "array", "prefixItems": [{"type": "string"}], "items": {"type": "number"}}),
            vec![
                (Zod, Eq("z.tuple([z.string()]).rest(z.number())")),
                (Zod4, Eq("z.tuple([z.string()], z.number())")),
                (Valibot, Eq("v.tupleWithRest([v.string()], v.number())")),
                (Ts, Eq("[string, ...number[]]")),
            ],
        ),
        (
            json!({"type": "array", "prefixItems": [{"type": "string"}]}),
            vec![
                (Zod, Eq("z.tuple([z.string()]).rest(z.unknown())")),
                (Ts, Eq("[string, ...unknown[]]")),
            ],
        ),
        (
            json!({"type": "array", "prefixItems": [{"type": "string"}, {"type": "boolean"}], "items": false}),
            vec![(Ts, Eq("[string, boolean]"))],
        ),
        (
            json!({"type": "array", "prefixItems": [{"type": "string"}], "items": {"anyOf": [{"type": "number"}, {"type": "null"}]}}),
            vec![(Ts, Eq("[string, ...(number | null)[]]"))],
        ),
        (
            vld::array(vld::number())
                .contains(3)
                .min_contains(2)
                .max_contains(3)
                .json_schema(),
            vec![(
                Zod,
                Has(
                    "refine((a) => (n => n >= 2 && n <= 3)(a.filter((x) => x === 3).length), \"Must contain 3\")",
                ),
            )],
        ),
        (
            json!({"type": "array", "contains": {"const": {"a": 1}}}),
            vec![(
                Zod,
                Has("a.filter((x) => JSON.stringify(x) === \"{\\\"a\\\":1}\").length >= 1"),
            )],
        ),
        (
            vld::array(vld::number()).unique().json_schema(),
            vec![(
                Zod,
                Has(
                    "refine((a) => new Set(a.map((x) => JSON.stringify(x))).size === a.length, \"Items must be unique\")",
                ),
            )],
        ),
        (
            json!({"type": "array", "items": {"type": "boolean"}, "x-minUniqueItems": 2, "x-maxUniqueItems": 2}),
            vec![(Valibot, Has(".size >= 2")), (Valibot, Has(".size <= 2"))],
        ),
        // refs, never, intersections
        (
            json!({"$ref": "#/components/schemas/Address"}),
            vec![
                (Ts, Eq("Address")),
                (Zod, Eq("z.lazy(() => AddressSchema)")),
            ],
        ),
        (
            json!({"$ref": "#/$defs/A"}),
            vec![(Zod4, Eq("z.lazy(() => ASchema)")), (Ts, Eq("A"))],
        ),
        (
            json!({"$ref": "#/definitions/A"}),
            vec![(Valibot, Eq("v.lazy(() => ASchema)"))],
        ),
        (
            json!({"$ref": "https://x/y"}),
            vec![(Zod, Eq("z.unknown()")), (Ts, Eq("unknown"))],
        ),
        (
            vld::never().json_schema(),
            vec![
                (Zod, Eq("z.never()")),
                (Valibot, Eq("v.never()")),
                (Ts, Eq("never")),
            ],
        ),
        (
            json!({"not": true}),
            vec![(Zod, Eq("z.never()")), (Ts, Eq("never"))],
        ),
        (
            json!({"not": {"type": "string"}}),
            vec![(Zod, Eq("z.unknown()"))],
        ),
        (
            json!({"allOf": [{"type": "string"}, {"minLength": 1}, {"type": "number"}]}),
            vec![
                (
                    Zod,
                    Eq("z.intersection(z.intersection(z.string(), z.unknown()), z.number())"),
                ),
                (
                    Valibot,
                    Eq("v.intersect([v.string(), v.unknown(), v.number()])"),
                ),
            ],
        ),
        (
            json!({"allOf": []}),
            vec![(Zod, Eq("z.unknown()")), (Ts, Eq("unknown"))],
        ),
        (
            json!({"allOf": [{"type": "string"}]}),
            vec![(Zod, Eq("z.string()"))],
        ),
        (
            json!({"allOf": [{"$ref": "#/components/schemas/A"}, {"anyOf": [{"$ref": "#/components/schemas/B"}, {"type": "null"}]}]}),
            vec![(Ts, Eq("A & (B | null)"))],
        ),
        (
            json!({"type": "array", "items": {"allOf": [{"$ref": "#/components/schemas/A"}, {"$ref": "#/components/schemas/B"}]}}),
            vec![(Ts, Eq("(A & B)[]"))],
        ),
        // vld's extra patterns / formats / multipleOf sit in `allOf` next to `type`
        (
            vld::string()
                .min(3)
                .starts_with("a")
                .ends_with("z")
                .json_schema(),
            vec![
                (
                    Zod,
                    Eq(r#"z.string().min(3).regex(new RegExp("^a")).regex(new RegExp("z$"))"#),
                ),
                (
                    Valibot,
                    Eq(
                        r#"v.pipe(v.string(), v.minLength(3), v.regex(new RegExp("^a")), v.regex(new RegExp("z$")))"#,
                    ),
                ),
            ],
        ),
        (
            vld::number().multiple_of(3.).multiple_of(5.).json_schema(),
            vec![(Zod, Eq("z.number().multipleOf(3).multipleOf(5)"))],
        ),
        (
            json!({"type": "object", "properties": {}, "allOf": [{"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]}]}),
            vec![(Zod, Starts("z.intersection(z.object({}), z.object("))],
        ),
        (
            json!({"type": "string", "allOf": [{"pattern": "a"}, {"pattern": "b"}]}),
            vec![(Ts, Eq("string"))],
        ),
        (
            json!({"type": "string", "allOf": [{"format": "email"}]}),
            vec![(
                Zod4,
                Eq(
                    "z.string().refine((s) => z.email().safeParse(s).success, { error: \"Invalid email\" })",
                ),
            )],
        ),
        (
            json!({"type": "string", "allOf": [{"format": "email"}], "description": "d", "default": "a@b"}),
            vec![(
                Zod,
                Eq("z.string().email().describe(\"d\").nullish().transform((v) => v ?? \"a@b\")"),
            )],
        ),
        // checks come after chained methods (a Zod 3 refinement has no `.min()`)
        (
            vld::string().lowercase().min(2).describe("d").json_schema(),
            vec![
                (
                    Zod,
                    Eq(
                        r#"z.string().min(2).describe("d").refine((s) => s === s.toLowerCase(), "Must be lowercase")"#,
                    ),
                ),
                (
                    Zod4,
                    Eq(
                        r#"z.string().min(2).describe("d").refine((s) => s === s.toLowerCase(), { error: "Must be lowercase" })"#,
                    ),
                ),
                (
                    Valibot,
                    Eq(
                        r#"v.pipe(v.string(), v.minLength(2), v.description("d"), v.check((s) => s === s.toLowerCase(), "Must be lowercase"))"#,
                    ),
                ),
            ],
        ),
        // Zod 4 formats
        (
            json!({"type": "string", "format": "ip"}),
            vec![
                (
                    Zod4,
                    Eq(
                        "z.string().refine((s) => z.ipv4().safeParse(s).success || z.ipv6().safeParse(s).success, { error: \"Invalid IP\" })",
                    ),
                ),
                (Zod, Eq("z.string().ip()")),
            ],
        ),
        (
            json!({"type": "string", "format": "email", "minLength": 3, "pattern": "^a"}),
            vec![(Zod4, Eq("z.email().min(3).regex(new RegExp(\"^a\"))"))],
        ),
        (
            vld::string().uuid().json_schema(),
            vec![(Zod4, Eq("z.guid()")), (Zod, Eq("z.string().uuid()"))],
        ),
        (
            vld::string().uuid_v4().json_schema(),
            vec![(
                Zod4,
                Eq("z.guid().regex(/^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-4/)"),
            )],
        ),
        // `z.iso.datetime()` would reject vld's `t` / `z` and take `HH:MM`.
        (
            vld::string().iso_datetime().json_schema(),
            vec![
                (Zod4, Starts(r"z.string().regex(/^\d{4}-\d{2}-\d{2}[Tt]")),
                (Zod4, Lacks("z.iso")),
            ],
        ),
        (
            vld::string().iso_date().json_schema(),
            vec![(Zod4, Starts("z.iso.date()"))],
        ),
        // `z.iso.time()` takes any number of fraction digits; vld up to 9.
        (
            vld::string().iso_time().json_schema(),
            vec![(
                Zod4,
                Eq(r"z.iso.time().regex(/^([01]\d|2[0-3]):[0-5]\d(:[0-5]\d(\.\d{1,9})?)?$/)"),
            )],
        ),
        (
            vld::string().ipv4().json_schema(),
            vec![
                (Zod4, Eq("z.ipv4()")),
                (Zod, Eq("z.string().ip({ version: \"v4\" })")),
                (Valibot, Eq("v.pipe(v.string(), v.ipv4())")),
            ],
        ),
        (
            vld::string().ipv6().json_schema(),
            vec![
                (Zod4, Eq("z.ipv6()")),
                (Valibot, Eq("v.pipe(v.string(), v.ipv6())")),
            ],
        ),
        (
            vld::string().url().json_schema(),
            vec![
                (Zod4, Eq(r"z.url().regex(/^https?:\/\/\S+$/)")),
                (Zod, Eq(r"z.string().url().regex(/^https?:\/\/\S+$/)")),
            ],
        ),
        (
            vld::array(vld::string()).unique().json_schema(),
            vec![(Zod4, Has("{ error: \"Items must be unique\" }"))],
        ),
        (
            json!({"type": "string", "format": "cidr"}),
            vec![
                (Zod4, Has("z.ipv4().safeParse(a).success")),
                (
                    Valibot,
                    Has("v.safeParse(v.pipe(v.string(), v.ipv6()), a).success"),
                ),
            ],
        ),
        (
            json!({"type": "string", "format": "int32"}),
            vec![(Zod, Eq("z.string()"))],
        ),
        // dates
        (
            json!({"type": "string", "format": "date", "formatMinimum": "2020-01-01", "x-past": true}),
            vec![
                (
                    Zod,
                    Has(r#".refine((s) => s >= "2020-01-01", "Must be >= 2020-01-01")"#),
                ),
                (Zod, Has("Date.parse(s) < Date.now()")),
            ],
        ),
        (
            json!({"type": "string", "format": "date-time", "formatMaximum": "2020-01-01T00:00:00Z", "x-future": true}),
            vec![
                (
                    Zod,
                    Has(r#"(s) => Date.parse(s) <= Date.parse("2020-01-01T00:00:00Z")"#),
                ),
                (Zod, Has("Date.parse(s) > Date.now()")),
            ],
        ),
        // vld's naive-allowed date-time: siblings apply to every branch
        (
            json!({"type": "string", "anyOf": [{"format": "date-time"}, {"pattern": "^a$"}], "x-past": true}),
            vec![
                (Zod, Starts("z.union([z.string().regex(")),
                (Zod, Lacks("unknown")),
            ],
        ),
        (
            json!({"type": "string", "anyOf": [{"format": "date"}, {"pattern": "^b$"}]}),
            vec![(Ts, Eq("string"))],
        ),
        // discriminated unions
        (
            animal.json_schema(),
            vec![
                (Zod, Starts(r#"z.discriminatedUnion("type", ["#)),
                (Valibot, Starts(r#"v.variant("type", ["#)),
            ],
        ),
        (
            json!({"oneOf": [obj(json!({"const": "a"})), obj(json!({"const": "a"}))], "discriminator": disc}),
            vec![(Zod, Starts("z.union(["))],
        ),
        (
            json!({"oneOf": [obj(json!({"const": "a"})), {"type": "string"}], "discriminator": disc}),
            vec![(Valibot, Starts("v.union(["))],
        ),
        (
            json!({"anyOf": [obj(json!({"const": "a"})), obj(json!({"type": "string"}))]}),
            vec![(Zod, Starts("z.union(["))],
        ),
        (
            json!({"oneOf": [obj(json!({"enum": ["a"]})), obj(json!({"const": "b"})), {"type": "null"}]}),
            vec![
                (Zod, Starts(r#"z.discriminatedUnion("t", ["#)),
                (Zod, Ends(".nullable()")),
            ],
        ),
        (
            json!({"oneOf": [obj(json!({"const": "a"})), described]}),
            vec![
                (Zod, Starts("z.discriminatedUnion(")),
                (Valibot, Starts("v.union([")),
            ],
        ),
        (
            json!({"oneOf": [obj(json!({"const": "a", "default": "a"})), obj(json!({"const": "b"}))]}),
            vec![(Zod, Starts("z.union(["))],
        ),
        (
            json!({"oneOf": [obj(json!({"const": "a"})), obj(json!({"const": "b"}))], "discriminator": {"propertyName": "x"}}),
            vec![(Zod, Starts("z.union(["))],
        ),
        // field order: `required` gives the order of the required fields
        (
            json!({"type": "object", "properties": {"z": {"type": "number"}, "m": {"type": "number"}, "a": {"type": "number"}}, "required": ["z", "a", "gone"]}),
            vec![
                (
                    Valibot,
                    Eq(
                        "v.object({\n  z: v.number(),\n  m: v.optional(v.number()),\n  a: v.number()\n})",
                    ),
                ),
                (Ts, Eq("{\n  z: number;\n  m?: number;\n  a: number;\n}")),
            ],
        ),
        (
            json!({"type": "object", "properties": {"o": {"type": "object", "properties": {"x": {"type": "number"}}, "required": ["x"]}}, "required": ["o"]}),
            vec![(Ts, Eq("{\n  o: {\n    x: number;\n  };\n}"))],
        ),
        (
            json!({"type": "object", "properties": {"$x": {"type": "number"}, "_1": {"type": "number"}, "1a": {"type": "number"}}}),
            vec![
                (Zod, Has("  $x: ")),
                (Zod, Has("  _1: ")),
                (Zod, Has("  \"1a\": ")),
            ],
        ),
        (
            vld::object()
                .field("o", vld::string().optional())
                .field("n", vld::string().nullable())
                .field("h", vld::string().nullish())
                .json_schema(),
            vec![
                (Zod, Has("o: z.string().optional()")),
                (Zod, Has("n: z.string().nullable()")),
                (Zod, Lacks("n: z.string().nullable().optional()")),
                (Zod, Has("h: z.string().nullable().optional()")),
                (Ts, Has("o?: string;")),
                (Ts, Has("n: string | null;")),
                (Ts, Has("h?: string | null;")),
            ],
        ),
        // numbers in constraints
        (
            json!({"type": "number", "minimum": 0.5, "exclusiveMinimum": -1, "exclusiveMaximum": 1e20, "multipleOf": 0.25}),
            vec![
                (
                    Zod,
                    Eq("z.number().min(0.5).gt(-1).lt(100000000000000000000).multipleOf(0.25)"),
                ),
                (
                    Valibot,
                    Eq(
                        "v.pipe(v.number(), v.minValue(0.5), v.gtValue(-1), v.ltValue(100000000000000000000), v.multipleOf(0.25))",
                    ),
                ),
            ],
        ),
        (
            json!({"type": "number", "maximum": 1e15}),
            vec![(Zod, Eq("z.number().max(1000000000000000)"))],
        ),
    ];

    // Rust regexes translated for JS; non-portable ones become a note.
    rows.extend([
        (p("(?i)^abc$"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("^abc$", "i"))"#))]),
        (p("(?im)(?s)x"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("x", "ims"))"#))]),
        (p("(?ii)x"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("x", "i"))"#))]),
        (p(r"\Aab\z"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("^ab$"))"#))]),
        (p(r"[\A]"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("(?P<y>[0-9]{4})"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("(?<y>[0-9]{4})"))"#))]),
        (p("(?<y>a)(?:b)"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("(?<y>a)(?:b)"))"#))]),
        (p(r"\d\D\w\W\s\S\b\B\n\t\r\f\v\x41\u0041\.\-"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("\\d\\D\\w\\W\\s\\S\\b\\B\\n\\t\\r\\f\\v\\x41\\u0041\\.\\-"))"#))]),
        (p("[^]a]"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("[a-z&&[^x]]"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("[a--b]"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("[a~~b]"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("[a-b]"), vec![(Zod, Eq(r#"z.string().regex(new RegExp("[a-b]"))"#))]),
        (p("(?=a)"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("[a"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("a\\"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("(?"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p(r"\b{start}"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p(r"\<a"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("(?x)a"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("(?i"), vec![(Zod, Starts("z.string() /* pattern not portable"))]),
        (p("a*/b["), vec![(Zod, Lacks("*/b")), (Zod, Has(r"a*\/b["))]),
    ]);
    rows.extend([
        (
            p(r"[\z]"),
            vec![(Zod, Starts("z.string() /* pattern not portable"))],
        ),
        (
            p("[(?]"),
            vec![(Zod, Eq(r#"z.string().regex(new RegExp("[(?]"))"#))],
        ),
        (
            p("(a)b"),
            vec![(Zod, Eq(r#"z.string().regex(new RegExp("(a)b"))"#))],
        ),
        (
            p("(?P=n)"),
            vec![(Zod, Starts("z.string() /* pattern not portable"))],
        ),
        // keys checked by a non-string schema; a lone intersection member
        (
            json!({"type": "object", "propertyNames": {"type": "integer"}}),
            vec![(Zod, Eq("z.record(z.number().int(), z.unknown())"))],
        ),
        (
            json!({"allOf": [{"anyOf": [{"$ref": "#/components/schemas/A"}, {"type": "null"}]}]}),
            vec![(Ts, Eq("A | null"))],
        ),
        // a branch's own `type` wins over the sibling one
        (
            json!({"type": "string", "anyOf": [{"type": "number"}, {"minLength": 1}]}),
            vec![(Ts, Eq("number | string"))],
        ),
    ]);
    for bad in [
        "a(?i)b",
        r"\p{L}",
        "[[:alpha:]]",
        "[a&&b]",
        "[]a]",
        r"\x{41}",
    ] {
        rows.push((
            p(bad),
            vec![
                (Zod, Starts("z.string() /* pattern not portable")),
                (Zod, Lacks(".regex(")),
            ],
        ));
    }
    rows
}

/// Given each row's JSON Schema, when code is generated for the row's
/// targets, then it is (or contains / lacks) the expected code.
#[test]
fn every_schema_generates_the_expected_code() {
    let mut failures = Vec::new();
    for (schema, wants) in rows() {
        for (t, want) in wants {
            let got = emit(t, &schema);
            let ok = match want {
                Eq(w) => got == w,
                Starts(w) => got.starts_with(w),
                Ends(w) => got.ends_with(w),
                Has(w) => got.contains(w),
                Lacks(w) => !got.contains(w),
            };
            if !ok {
                failures.push(format!("{t:?} for {schema}:\n  got  {got}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    // The typed entry points are the value forms on `json_schema()`.
    let s = vld::string().min(2).email();
    let js = s.json_schema();
    assert_eq!(vld::ts::to_zod(&s), emit(Zod, &js));
    assert_eq!(vld::ts::to_zod4(&s), emit(Zod4, &js));
    assert_eq!(vld::ts::to_valibot(&s), emit(Valibot, &js));
    assert_eq!(vld::ts::to_ts_type(&s), emit(Ts, &js));
    assert_eq!(vld::ts::to_openapi(&s), js);
    // vld applies `with_default` to `null` too, as the generated code does.
    let d = vld::string().min(1).with_default("user".to_string());
    assert_eq!(d.parse_value(&Value::Null).unwrap(), "user");
}

/// Given every `format` the generators know, when Zod 3, Zod 4 and Valibot
/// code is generated for a string with that format, then it matches the
/// snapshot (one check per format, mirroring vld's predicate).
#[test]
fn every_format_has_its_check() {
    let formats = [
        "email",
        "uri",
        "url",
        "url-strict",
        "uuid",
        "uuid-v1",
        "uuid-v4",
        "uuid-v7",
        "ipv4",
        "ipv6",
        "ip",
        "cidr",
        "date",
        "date-time",
        "time",
        "hostname",
        "mac",
        "hex",
        "slug",
        "semver",
        "jwt",
        "ascii",
        "alpha",
        "alphanumeric",
        "lowercase",
        "uppercase",
        "locale",
        "country-code",
        "currency-code",
        "cron",
        "credit-card",
        "phone",
        "phone-e164",
        "color",
        "byte",
        "base64url",
        "json",
        "cuid2",
        "ulid",
        "nanoid",
        "emoji",
        "decimal",
    ];
    let mut out = String::new();
    for f in formats {
        let schema = json!({"type": "string", "format": f});
        let fragment = json!({"type": "string", "allOf": [{"format": f}]});
        for t in [Zod, Zod4, Valibot] {
            out.push_str(&format!("{f} {t:?}: {}\n", emit(t, &schema)));
        }
        out.push_str(&format!("{f} Zod4 fragment: {}\n\n", emit(Zod4, &fragment)));
    }
    insta::assert_snapshot!("formats", out);
}
