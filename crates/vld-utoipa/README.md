[![Crates.io](https://img.shields.io/crates/v/vld-utoipa?style=for-the-badge)](https://crates.io/crates/vld-utoipa)
[![docs.rs](https://img.shields.io/docsrs/vld-utoipa?style=for-the-badge)](https://docs.rs/vld-utoipa)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-utoipa

Bridge between [vld](https://crates.io/crates/vld) validation library and
[utoipa](https://crates.io/crates/utoipa) OpenAPI documentation.

Define validation rules once with `vld` and call **`impl_to_schema!` once** — the same workflow
for JSON bodies and for query/path parameters, with no duplicate schema definitions.

## Installation

```toml
[dependencies]
vld = { version = "0.4", features = ["openapi"] }
vld-utoipa = "0.4"
utoipa = "5"
```

## One macro, same as derive

| Struct role | Attribute on struct | Bridge |
|-------------|---------------------|--------|
| JSON body / response | _(none)_ | `impl_to_schema!(CreateUser)` |
| Query `?q=…` | `#[into_params(parameter_in = Query)]` | `impl_to_schema!(SearchQuery)` |
| Path `/users/{id}` | `#[into_params(parameter_in = Path)]` | `impl_to_schema!(UserPath)` |

```rust
use vld::prelude::*;
use vld_utoipa::impl_to_schema;

// Body
vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}
impl_to_schema!(CreateUser);

// Query — same attribute utoipa users already know
vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct SearchQuery {
        pub q: String => vld::string().min(1).max(200),
    }
}
impl_to_schema!(SearchQuery);

// Path
vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Path)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().positive(),
    }
}
impl_to_schema!(UserPath);
```

```rust
#[utoipa::path(
    post,
    path = "/users/{id}/search",
    params(UserPath, SearchQuery),
    request_body = CreateUser,
)]
async fn handler(
    VldPath(path): VldPath<UserPath>,
    VldQuery(query): VldQuery<SearchQuery>,
    VldJson(body): VldJson<CreateUser>,
) { /* ... */ }
```

`minLength`, `minimum`, and other vld rules appear in OpenAPI automatically — no
`#[param(min_length = …)]` duplication.

## Using with `#[derive(Validate)]`

`impl_to_schema!` also works with `#[derive(Validate)]` from `vld-derive`.
This lets you use standard Rust struct syntax with serde attributes like
`#[serde(rename_all = "camelCase")]` and still get OpenAPI schema generation.

```toml
[dependencies]
vld = { version = "0.4", features = ["derive", "openapi"] }
vld-utoipa = "0.4"
utoipa = "5"
```

```rust
use vld::Validate;
use vld_utoipa::impl_to_schema;

#[derive(Debug, serde::Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct UpdateLocationRequest {
    #[vld(vld::string().min(1).max(255))]
    name: String,
    #[vld(vld::string())]
    street_address: String,
    #[vld(vld::number().int().non_negative().min(1).max(9999))]
    street_number: i64,
    #[vld(vld::string().optional())]
    street_number_addition: Option<String>,
    #[vld(vld::boolean())]
    is_active: bool,
}

impl_to_schema!(UpdateLocationRequest);
// OpenAPI schema properties use camelCase:
// "streetAddress", "streetNumber", "streetNumberAddition", "isActive"
```

Query/path with derive — add `#[into_params(parameter_in = Query)]` on the struct:

```rust
#[derive(Debug, serde::Deserialize, Validate)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    #[vld(vld::string().min(1).max(200))]
    q: String,
}

impl_to_schema!(SearchQuery);
```

## Separate structs per HTTP role

OpenAPI treats body fields and query parameters as different contracts. Use one vld struct per
role (as above). Shared field rules can live in a nested `vld::schema!` type composed into both.

## Nested Schemas (auto-registration)

When you use `vld::nested!(Type)`, the nested type is automatically registered in
utoipa's `components/schemas`. No need to list it manually in `#[openapi(components(schemas(...)))]`.

```rust
use vld::prelude::*;
use vld_utoipa::impl_to_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}

impl_to_schema!(Address);

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2),
        pub address: Address => vld::nested!(Address),
    }
}

impl_to_schema!(CreateUser);

// In OpenAPI spec:
// - CreateUser.address → { "$ref": "#/components/schemas/Address" }
// - Address schema is auto-registered in components
```

## Custom Schema Name

```rust
impl_to_schema!(CreateUser, "CreateUserRequest");
```

## Converting Arbitrary JSON Schema

```rust
use vld_utoipa::json_schema_to_schema;

let json_schema = serde_json::json!({
    "type": "object",
    "required": ["name"],
    "properties": {
        "name": { "type": "string", "minLength": 1 }
    }
});

let utoipa_schema = json_schema_to_schema(&json_schema);
```

## Supported JSON Schema Features

- Primitive types: `string`, `number`, `integer`, `boolean`, `null`
- Object with `properties` and `required`
- Array with `items`, `minItems`, `maxItems`
- `oneOf`, `allOf` composites
- `enum` values
- String: `minLength`, `maxLength`, `pattern`, `format`
- Number: `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`
- `$ref` references
- `description`, `default`, `example`, `title`

## Migration from older APIs

The unified API is **`#[into_params(parameter_in = …)]` on the struct** + **`impl_to_schema!(T)`**.
Older patterns still compile but emit deprecation warnings.

| Old (deprecated) | New (recommended) |
|------------------|-------------------|
| `impl_into_params!(T)` | `#[into_params(parameter_in = Query)]` + `impl_to_schema!(T)` |
| `impl_into_params!(T, Query)` | `#[into_params(parameter_in = Query)]` + `impl_to_schema!(T)` |
| `impl_into_params!(T, Path)` | `#[into_params(parameter_in = Path)]` + `impl_to_schema!(T)` |
| `impl_to_schema_query!(T)` | `#[into_params(parameter_in = Query)]` + `impl_to_schema!(T)` |
| `impl_to_schema_path!(T)` | `#[into_params(parameter_in = Path)]` + `impl_to_schema!(T)` |
| `impl_to_schema!(T, query)` | `#[into_params(parameter_in = Query)]` + `impl_to_schema!(T)` |
| `impl_to_schema!(T, path)` | `#[into_params(parameter_in = Path)]` + `impl_to_schema!(T)` |
| `impl_to_schema!(T, "Name")` | unchanged — custom OpenAPI component name |

Resolution order for parameter location: utoipa provider → legacy macro override →
`#[into_params(parameter_in = …)]` on the struct.

## Running the Example

```bash
cargo run -p vld-utoipa --example utoipa_basic
```

## License

MIT
