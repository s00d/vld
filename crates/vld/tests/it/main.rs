//! Core `vld` behaviour tests: models and user stories, one test binary.
//! See `tests/README.md` for the layout and the rules.

#[macro_use]
mod common;

#[cfg(feature = "crypto")]
mod model_crypto;
mod model_numbers;
mod model_scalars;
mod model_schema;
mod model_strings;

mod model_robustness;

#[cfg(feature = "crypto")]
mod corpus_crypto;
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
mod corpus_dates;
mod corpus_files_paths_net;
mod corpus_formats;
#[cfg(feature = "geo")]
mod corpus_geo;
#[cfg(feature = "openapi")]
mod corpus_json_schema;
mod corpus_numbers_strings;
mod corpus_units;

#[cfg(feature = "derive")]
mod story_api_request;
mod story_collections_objects;
#[cfg(feature = "diff")]
mod story_diff;
mod story_form;
mod story_inputs;
mod story_lenient_report;
#[cfg(all(feature = "llm", feature = "derive", feature = "regex"))]
mod story_llm;
mod story_messages_i18n;
mod story_optional_fields;
mod story_redaction;
mod story_scalars_adapters;
mod story_schema_macro;
mod story_settings_migration;
#[cfg(feature = "derive")]
mod story_tagged_enums;
mod story_untrusted_input;

// Optional modules; their tests also use the schema features listed.
#[cfg(all(
    feature = "fake",
    feature = "chrono",
    feature = "regex",
    feature = "string-advanced",
    feature = "decimal",
    feature = "net"
))]
mod fake;
#[cfg(all(
    feature = "ts",
    feature = "regex",
    feature = "string-advanced",
    feature = "derive"
))]
mod ts;
