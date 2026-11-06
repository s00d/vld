# vld-clap

[![crates.io](https://img.shields.io/crates/v/vld-clap.svg)](https://crates.io/crates/vld-clap)
[![docs.rs](https://img.shields.io/docsrs/vld-clap)](https://docs.rs/vld-clap)

Validate [clap](https://crates.io/crates/clap) 4 arguments with [vld](https://crates.io/crates/vld) rules on the same struct (`#[derive(Validate)]`), with errors that name the real flags (`--out`, `<INPUT>`).

## Install

```toml
[dependencies]
vld = { version = "0.5", features = ["derive"] }
vld-clap = "0.5"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
```

## Example

```rust
use clap::Parser;
use vld::Validate;
use vld_clap::prelude::*;

#[derive(Parser, Debug, serde::Serialize, Validate)]
struct Cli {
    #[arg(long)]
    #[vld(vld::string().email())]
    email: String,

    #[arg(long, default_value_t = 8080)]
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
}

fn main() {
    let cli = Cli::parse();
    validate_or_exit(&cli); // prints "--port: Value must be at least 1" etc., exits with 2
    println!("{cli:?}");
}
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/config-cli/clap>
- API: <https://docs.rs/vld-clap>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
