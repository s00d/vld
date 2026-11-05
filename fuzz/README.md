# vld fuzz targets

`cargo-fuzz` targets (libFuzzer, nightly only). The crate is not part of the
workspace and is not run in CI.

| Target | Checks |
|---|---|
| `iso_formats` | `string().iso_date/iso_time/iso_datetime()` never panic; an accepted datetime has a valid date |
| `string_formats` | email / url invariants; `ipv4` / `ipv6` agree with `std::net`; `cidr` shape |
| `url_decode` | `vld-http-common` query / form / cookie / percent decoding never panic |
| `error_truncation` | the `received` echo is a ≤100-char, char-boundary prefix of the input |

```sh
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run iso_formats            # Ctrl-C to stop
cargo +nightly fuzz run url_decode -- -max_total_time=60
cargo +nightly fuzz list
```

Crashes land in `fuzz/artifacts/<target>/`; replay one with
`cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>`.
`cargo check` (stable) is enough to keep the targets compiling.
