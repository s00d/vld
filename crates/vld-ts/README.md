[![Crates.io](https://img.shields.io/crates/v/vld-ts?style=for-the-badge)](https://crates.io/crates/vld-ts)
[![docs.rs](https://img.shields.io/docsrs/vld-ts?style=for-the-badge)](https://docs.rs/vld-ts)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-ts

Generate TypeScript [Zod](https://zod.dev/) and [Valibot](https://valibot.dev/) schemas directly from [vld](https://crates.io/crates/vld) schemas and types.

## Overview

`vld-ts` is **vld-first**: use your `vld` schemas/types as the source of truth and generate frontend Zod/Valibot from them.

## Installation

```toml
[dependencies]
vld-ts = "0.4"
```

## Quick start

```rust
use vld_ts::{
    impl_to_openapi, impl_to_valibot, impl_to_zod, openapi_refs, to_openapi, to_valibot, to_zod,
    ToOpenApi, ToRefs, ToValibot, ToZod,
};

vld::schema! {
    pub struct Address {
        pub city: String => vld::string().min(1),
    }
}

vld::schema! {
    pub struct Order {
        pub shipping: Address => vld::nested!(Address),
    }
}

impl_to_zod!(Order);
impl_to_valibot!(Order);
impl_to_openapi!(Order);
impl_to_openapi!(Address);

// From schema instance
let zod = to_zod(&vld::string().min(2).email());
assert!(zod.contains(".email()"));

// From vld type (single schema expression, no imports/exports)
let order_zod = Order::to_zod();
assert!(order_zod.starts_with("z.object("));
assert!(order_zod.contains("shipping: z.lazy(() => AddressSchema)"));
assert!(!order_zod.contains("import { z } from \"zod\""));


// Valibot from schema instance
let single_valibot = to_valibot(&vld::string().min(2).email());
assert!(single_valibot.contains("v.string()"));
assert!(single_valibot.contains("v.email()"));

// Valibot from vld type (single schema expression, no imports/exports)
let order_valibot = Order::to_valibot();
assert!(order_valibot.starts_with("v.object"));
assert!(order_valibot.contains("shipping: v.lazy(() => AddressSchema)"));
assert!(!order_valibot.contains("import * as v from \"valibot\""));


// OpenAPI schema object from schema instance
let email_schema = to_openapi(&vld::string().email());
assert_eq!(email_schema["type"], "string");

// OpenAPI schema object from vld type
let openapi_schema = Address::to_openapi();
assert_eq!(openapi_schema["type"], "object");
let refs = openapi_refs(&Order::to_openapi());
assert!(refs.contains(&"#/components/schemas/Address".to_string()));
let refs2 = Order::to_refs();
assert!(refs2.contains(&"#/components/schemas/Address".to_string()));
```

### Named exports / modules

`to_zod()` / `to_valibot()` return a bare expression. For a TypeScript module, use
`to_zod_export()` / `to_valibot_export()` (or the free functions `zod_export` /
`valibot_export`) and add the import yourself. Nested `$ref`s are emitted as
`z.lazy(() => <TypeName>Schema)`, so export every nested type too.

```rust
use vld_ts::{impl_to_zod, ToZod};

vld::schema! {
    pub struct Address {
        pub city: String => vld::string().min(1),
    }
}
vld::schema! {
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub address: Address => vld::nested!(Address),
    }
}

impl_to_zod!(Address);
impl_to_zod!(User, "UserBody"); // optional custom export name

let module = format!(
    "import {{ z }} from \"zod\";\n\n{}\n{}",
    Address::to_zod_export(),
    User::to_zod_export(),
);
assert!(module.contains("export const UserBodySchema = z.object("));
```

Generated:

```typescript
import { z } from "zod";

export const AddressSchema = z.object({
  city: z.string().min(1)
});
export type Address = z.infer<typeof AddressSchema>;

export const UserBodySchema = z.object({
  name: z.string().min(2).max(50),
  address: z.lazy(() => AddressSchema)
});
export type UserBody = z.infer<typeof UserBodySchema>;
```

Valibot (`impl_to_valibot!` + `to_valibot_export()`) produces the same shape with
`v.InferOutput<typeof …>`:

```typescript
import * as v from "valibot";

export const UserSchema = v.object({
  name: v.pipe(v.string(), v.minLength(2), v.maxLength(50)),
  email: v.pipe(v.string(), v.email())
});
export type User = v.InferOutput<typeof UserSchema>;
```

Zod output targets the **Zod 3** API (`z.string().email()`, `.ip({ version })`).

## Supported output features

| JSON Schema                            | Zod output                                  | Valibot output                     |
|----------------------------------------|---------------------------------------------|------------------------------------|
| `type: "string"` / `"number"`          | `z.string()` / `z.number()`                 | `v.string()` / `v.number()`        |
| `type: "integer"`                      | `z.number().int()`                          | `v.pipe(v.number(), v.integer())`  |
| `type: "boolean"` / `"null"`           | `z.boolean()` / `z.null()`                  | `v.boolean()` / `v.null()`         |
| `type: ["string", "null"]`             | `z.string().nullable()`                     | `v.nullable(v.string())`           |
| `type: "array"`                        | `z.array(...)`                              | `v.array(...)`                     |
| `type: "object"`                       | `z.object({...})` (unknown keys stripped)   | `v.object({...})`                  |
| `additionalProperties: false`          | `.strict()`                                 | `v.strictObject({...})`            |
| `additionalProperties: {schema}`       | `.catchall(...)` / `z.record(...)`          | `v.objectWithRest` / `v.record`    |
| `oneOf` / `anyOf`                      | `z.union([...])`                            | `v.union([...])`                   |
| `oneOf`/`anyOf` with `null`            | `.nullable()`                               | `v.nullable(...)`                  |
| `allOf`                                | `z.intersection(...)`                       | `v.intersect([...])`               |
| `enum` / `const`                       | `z.literal(...)` / `z.union()`              | `v.literal(...)` / `v.union()`     |
| `format` (email, uuid, date, …)        | `.email()`, `.uuid()`, `.date()`, …         | `v.email()`, `v.uuid()`, …         |
| `minLength`/`maxLength`, `minItems`/`maxItems` | `.min()` / `.max()`                 | `v.minLength()` / `v.maxLength()`  |
| `minimum`/`maximum`/`multipleOf`       | `.min()` / `.max()` / `.multipleOf()`       | `v.minValue()` / …                 |
| `pattern`                              | `.regex(new RegExp("..."))`                 | `v.regex(new RegExp("..."))`       |
| `description` (any schema)             | `.describe("...")`                          | `v.description("...")`             |
| `default` (also applied to `null`)     | `.nullish().transform((v) => v ?? value)`   | `v.nullish(schema, value)`         |
| `$ref` to named schemas                | `z.lazy(() => NameSchema)`                  | `v.lazy(() => NameSchema)`         |

Strings, enum literals and object keys that are not JS identifiers are emitted as
escaped/quoted JSON string literals.

## Examples

```bash
cargo run -p vld-ts --example generate_zod
```

## License

MIT
