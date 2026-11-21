//! Story: the report of a lenient import (`ParseResult` / `FieldResult` from
//! `impl_validate_fields!`): which fields failed, the printed report, the saved file.

use crate::common::*;

use serde_json::json;

vld::schema! {
    #[derive(Debug, Default, serde::Serialize)]
    struct Contact {
        name: String => vld::string().min(2),
        email: String => vld::string().min(6).email(),
    }
}

vld::impl_validate_fields!(Contact {
    name: String => vld::string().min(2),
    email: String => vld::string().min(6).email(),
});

/// Given a contact imported leniently, whose email breaks two rules
/// When the result is inspected, printed and saved into a directory that does not exist yet
/// Then it is not valid and has errors, lists both fields in order (name ok, email not), prints
/// one line per field with one `✖` line per issue of the bad field, and the saved file holds the
/// struct with the bad field defaulted (the directories are created).
#[test]
fn lenient_import_report_is_printed_and_saved() {
    let r = Contact::parse_lenient(&json!({"name": "Alex", "email": "ab"})).unwrap();
    assert!(!r.is_valid());
    assert!(r.has_errors());
    let state: Vec<(&str, bool)> = r
        .fields()
        .iter()
        .map(|f| (f.name.as_str(), f.is_ok()))
        .collect();
    assert_eq!(state, [("name", true), ("email", false)]);

    assert_eq!(
        r.to_string(),
        "ParseResult (1 valid, 1 errors):\n\
         \x20 ✔ name: \"Alex\"\n\
         \x20 ✖ email: String must be at least 6 characters (received: \"ab\")\n\
         ✖ email: Invalid email address (received: \"ab\")\n"
    );

    let dir = tempdir();
    let path = dir.path().join("exports").join("2026").join("contact.json");
    r.save_to_file(&path).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved, json!({"name": "Alex", "email": ""}));
}
