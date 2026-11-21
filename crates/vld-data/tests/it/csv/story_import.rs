//! Stories: a CSV file imported into a vld type, every failing row reported
//! with its line, cells and `(path, code)` issues.

use crate::issues::{of, pairs};
use csv::ReaderBuilder;
use vld_data::csv::prelude::*;

vld::schema! {
    #[derive(Debug, PartialEq)]
    pub struct User {
        pub name: String => vld::string().min(2),
        pub zip: String => vld::string().len(5),
        pub age: i64 => vld::number().int().min(18),
        pub admin: bool => vld::boolean(),
        pub score: f64 => vld::number().blank_as(0.0),
    }
}

fn reader(data: &str) -> csv::Reader<&[u8]> {
    csv::Reader::from_reader(data.as_bytes())
}

fn user(name: &str, zip: &str, age: i64, admin: bool, score: f64) -> User {
    User {
        name: name.into(),
        zip: zip.into(),
        age,
        admin,
        score,
    }
}

/// `(line, (path, code)...)` of validation failures; panics on a CSV error.
#[track_caller]
fn failures(errors: &[RowError]) -> Vec<(u64, Vec<(String, String)>)> {
    errors
        .iter()
        .map(|e| match &e.error {
            RowErrorKind::Validation(v) => (e.line, of(v)),
            other => panic!(
                "line {}: expected a validation error, got {other:?}",
                e.line
            ),
        })
        .collect()
}

/// Given a CSV export with good and bad rows (quoted cells, a numeric-looking
/// string column, a blank optional score),
/// when it is imported with `validate_all`,
/// then a clean file gives every row, and a dirty one gives every failing
/// row with its file line, cells and issues.
#[test]
fn every_failing_row_is_reported_with_its_line() {
    let clean = "name,zip,age,admin,score\n\
                 \"Smith, Al\",01234,30,true,1.5\n\
                 Bo,\"12345\",18,false,\n";
    assert_eq!(
        validate_all::<User, _>(reader(clean)).unwrap(),
        [
            user("Smith, Al", "01234", 30, true, 1.5),
            user("Bo", "12345", 18, false, 0.0),
        ]
    );

    let dirty = "name,zip,age,admin,score\n\
                 Al,01234,30,true,1\n\
                 A,123,17,yes,2\n\
                 \"multi\nline\",12345,40,false,x\n\
                 Cy,99999,20,true,3\n";
    let errors = validate_all::<User, _>(reader(dirty)).unwrap_err();
    assert_eq!(
        failures(&errors),
        [
            (
                3,
                pairs(&[
                    (".name", "too_small"),
                    (".zip", "invalid_length"),
                    (".age", "too_small"),
                    (".admin", "invalid_type"),
                ])
            ),
            (4, pairs(&[(".score", "invalid_type")])),
        ]
    );
    assert_eq!(
        errors[0].record.as_deref(),
        Some(&["A", "123", "17", "yes", "2"].map(String::from)[..])
    );
    assert_eq!(errors[1].record.as_ref().unwrap()[0], "multi\nline");
    assert!(
        errors[0]
            .to_string()
            .starts_with("line 3: validation error")
    );
}

/// Given rows with missing and extra cells,
/// when they are read with `rows`,
/// then a strict reader reports a CSV error per short / long row and goes
/// on, a flexible one reports the missing columns as `missing_field` (a
/// `blank_as` column takes its default), and an
/// extra header column the schema does not know is ignored.
#[test]
fn missing_and_extra_columns() {
    let data = "name,zip,age,admin,score\n\
                Al,01234,30,true,1\n\
                Bo,12345\n\
                Cy,12345,30,true,1,extra\n\
                Di,12345,30,false,2\n";

    let strict: Vec<_> = rows::<User, _>(reader(data)).collect();
    assert_eq!(strict.len(), 4);
    assert_eq!(strict[0].as_ref().unwrap().name, "Al");
    for (row, line) in [(&strict[1], 3), (&strict[2], 4)] {
        let e = row.as_ref().unwrap_err();
        assert_eq!(e.line, line);
        assert!(matches!(e.error, RowErrorKind::Csv(_)), "{e:?}");
        assert!(e.record.is_none());
        assert!(
            e.to_string()
                .starts_with(&format!("line {line}: CSV error"))
        );
    }
    assert_eq!(strict[3].as_ref().unwrap().name, "Di");

    let flexible = ReaderBuilder::new()
        .flexible(true)
        .from_reader(data.as_bytes());
    let errors = validate_all::<User, _>(flexible).unwrap_err();
    assert_eq!(
        failures(&errors),
        [(
            3,
            pairs(&[(".age", "missing_field"), (".admin", "missing_field"),])
        )]
    );

    let extra = "name,zip,age,admin,score,note\nAl,01234,30,true,1,hi\n";
    assert_eq!(
        validate_all::<User, _>(reader(extra)).unwrap(),
        [user("Al", "01234", 30, true, 1.0)]
    );
}

/// Given a reader configured without a header row, invalid UTF-8, or a
/// header-only file,
/// when it is read,
/// then no header is one `Validation` error at line 1, invalid UTF-8 is a
/// CSV error at its line, and a header-only file is an empty import.
#[test]
fn reader_edge_cases() {
    let headerless = ReaderBuilder::new()
        .has_headers(false)
        .from_reader("Al,01234,30,true,1\n".as_bytes());
    let all: Vec<_> = rows::<User, _>(headerless).collect();
    assert_eq!(all.len(), 1);
    let e = all[0].as_ref().unwrap_err();
    assert_eq!(e.line, 1);
    assert!(
        matches!(&e.error, RowErrorKind::Validation(v) if of(v) == pairs(&[("", "parse_error")]))
    );

    let mut bytes = b"name,zip,age,admin,score\nAl,01234,30,true,1\n".to_vec();
    bytes.extend_from_slice(b"B\xff,12345,30,true,1\n");
    let errors = validate_all::<User, _>(csv::Reader::from_reader(&bytes[..])).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 3);
    assert!(matches!(errors[0].error, RowErrorKind::Csv(_)));

    assert!(
        validate_all::<User, _>(reader("name,zip,age,admin,score\n"))
            .unwrap()
            .is_empty()
    );
}
