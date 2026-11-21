//! Embedded error-message catalogs (feature `i18n`).
//!
//! The features pick the catalogs from `locales/` that are compiled in: `i18n` embeds
//! `en`, `ru`, `de`, `es` (the languages vld shipped before 0.8); each `i18n-<lang>` adds
//! `locales/<lang>.json`; `i18n-all` enables all of them.

use std::path::PathBuf;

/// Embedded with plain `i18n`.
const BASE: &[&str] = &["de", "en", "es", "ru"];

fn main() {
    println!("cargo:rerun-if-changed=locales");
    if std::env::var_os("CARGO_FEATURE_I18N").is_none() {
        return;
    }
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("locales");
    let mut selected: Vec<String> = BASE.iter().map(|l| l.to_string()).collect();
    for entry in std::fs::read_dir(&dir).expect("locales/") {
        let path = entry.expect("locales/ entry").path();
        let Some(code) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().is_some_and(|x| x == "json")
            && !selected.iter().any(|l| l == code)
            && std::env::var_os(format!("CARGO_FEATURE_I18N_{}", code.to_ascii_uppercase()))
                .is_some()
        {
            selected.push(code.to_string());
        }
    }
    selected.sort();

    // `en` is the always-compiled `EN_JSON` of the including module.
    let entries: Vec<String> = selected
        .iter()
        .map(|code| {
            if code == "en" {
                "(\"en\", EN_JSON)".to_string()
            } else {
                let path = dir.join(format!("{code}.json"));
                format!("({code:?}, include_str!({:?}))", path.display().to_string())
            }
        })
        .collect();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("vld_catalogs.rs");
    std::fs::write(out, format!("&[{}]\n", entries.join(",\n"))).expect("write vld_catalogs.rs");
}
