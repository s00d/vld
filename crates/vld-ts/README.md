# vld-ts

[![crates.io](https://img.shields.io/crates/v/vld-ts.svg)](https://crates.io/crates/vld-ts)
[![docs.rs](https://img.shields.io/docsrs/vld-ts)](https://docs.rs/vld-ts)

Generate TypeScript [Zod](https://zod.dev/) 3 or [Valibot](https://valibot.dev/) 1 schemas from [vld](https://crates.io/crates/vld) schemas, so the frontend accepts the same values as the server. No Node or TypeScript dependency: it prints code.

## Install

```toml
[dependencies]
vld = { version = "0.5", features = ["openapi"] }
vld-ts = "0.5"
```

## Example

```rust
use vld_ts::{impl_to_zod, to_zod, ToZod};

vld::schema! {
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}
impl_to_zod!(User);

assert_eq!(to_zod(&vld::string().email()), "z.string().email()");

let module = format!("import {{ z }} from \"zod\";\n\n{}", User::to_zod_export());
// export const UserSchema = z.object({ ... });
// export type User = z.infer<typeof UserSchema>;
```

Valibot: `impl_to_valibot!(User)`, `User::to_valibot()`, `User::to_valibot_export()`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/codegen/ts>
- API: <https://docs.rs/vld-ts>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
