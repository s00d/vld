# vld-clap

[Clap](https://docs.rs/clap) integration for the **vld** validation library.

Validate CLI arguments **after** clap has parsed them — email format, URL, ranges, string lengths, regex patterns, and everything else `vld` supports. Validation rules go directly on the Cli struct via `#[derive(Validate)]`.

## Why

Clap handles argument parsing (types, defaults, required/optional), but doesn't validate semantic constraints like "must be a valid email" or "port must be 1–65535". `vld-clap` fills that gap — no separate schema struct needed.

## Installation

```toml
[dependencies]
vld-clap = "0.1"
vld = { version = "0.1", features = ["derive"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
```

## Quick Start

```rust
use clap::Parser;
use vld::Validate;
use vld_clap::prelude::*;

#[derive(Parser, Debug, serde::Serialize, Validate)]
#[command(name = "myapp")]
struct Cli {
    #[arg(long)]
    #[vld(vld::string().email())]
    email: String,

    #[arg(long, default_value_t = 8080)]
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,

    #[arg(long)]
    #[vld(vld::string().min(2).max(50))]
    name: String,
}

fn main() {
    let cli = Cli::parse();
    validate_or_exit(&cli);
    println!("email={}, port={}", cli.email, cli.port);
}
```

## API

| Function | Description |
|----------|-------------|
| `validate(args)` | Validate a struct with `#[derive(Validate)]` + `Serialize` |
| `validate_or_exit(args)` | Validate; print error & exit(2) on failure |
| `validate_with_schema::<S, T>(args)` | Validate `T: Serialize` against a separate schema `S` |
| `validate_json::<S>(json)` | Validate raw JSON |

## Error Output

```
Invalid arguments:
       --email: Invalid email address
       --name: String must be at least 2 characters
       --port: Number must be at most 65535
```

## Examples

```bash
# Valid
cargo run -p vld-clap --example clap_basic -- \
  --email alice@example.com --name Alice --port 3000 --workers 8

# Invalid (exits with code 2)
cargo run -p vld-clap --example clap_basic -- \
  --email bad --name X --port 99999 --workers 0
```

## License

MIT
