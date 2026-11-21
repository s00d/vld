//! Stories: events validated before an `INSERT` and after a `SELECT`.

use crate::issues::{of, pairs};
use clickhouse::{Client, Row, test};
use vld_data::clickhouse::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct EventSchema {
        pub name: String => vld::string().min(1),
        pub value: i64 => vld::number().int().min(0),
    }
}

#[derive(Debug, Clone, PartialEq, Row, serde::Serialize, serde::Deserialize)]
struct Event {
    name: String,
    value: i64,
}

fn event(name: &str, value: i64) -> Event {
    Event {
        name: name.into(),
        value,
    }
}

#[track_caller]
fn issues_of(e: &VldClickhouseError) -> Vec<(String, String)> {
    match e {
        VldClickhouseError::Validation(e) => of(e),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Given an `INSERT` into `events`,
/// when valid and invalid events are written with `insert_validated`,
/// then the invalid one is refused with its issues, the `INSERT` stays
/// usable, and the server receives exactly the valid rows.
#[tokio::test]
async fn only_valid_events_are_inserted() {
    let mock = test::Mock::new();
    let client = Client::default().with_mock(&mock);
    let recording = mock.add(test::handlers::record());

    let mut insert = client.insert::<Event>("events").await.unwrap();
    insert_validated::<EventSchema, _>(&mut insert, &event("click", 1))
        .await
        .unwrap();
    let err = insert_validated::<EventSchema, _>(&mut insert, &event("", -1))
        .await
        .unwrap_err();
    assert_eq!(
        issues_of(&err),
        pairs(&[(".name", "too_small"), (".value", "too_small")])
    );
    insert_validated::<EventSchema, _>(&mut insert, &event("view", 0))
        .await
        .unwrap();
    insert.end().await.unwrap();

    let rows: Vec<Event> = recording.collect().await;
    assert_eq!(rows, [event("click", 1), event("view", 0)]);
}

/// Given `SELECT` results,
/// when they are fetched with `fetch_all_validated`,
/// then valid rows are returned, the first invalid row is `InvalidRow` with
/// its index and issues, and a server failure is a `Clickhouse` error.
#[tokio::test]
async fn fetched_events_are_rechecked() {
    let mock = test::Mock::new();
    let client = Client::default().with_mock(&mock);
    let query = || client.query("SELECT ?fields FROM events");

    mock.add(test::handlers::provide([event("a", 1), event("b", 2)]));
    let rows: Vec<Event> = fetch_all_validated::<EventSchema, _>(query())
        .await
        .unwrap();
    assert_eq!(rows, [event("a", 1), event("b", 2)]);

    mock.add(test::handlers::provide([
        event("a", 1),
        event("b", -2),
        event("", 3),
    ]));
    let err = fetch_all_validated::<EventSchema, Event>(query())
        .await
        .unwrap_err();
    match &err {
        VldClickhouseError::InvalidRow { index, error } => {
            assert_eq!(*index, 1);
            assert_eq!(issues_of(error), pairs(&[(".value", "too_small")]));
        }
        other => panic!("expected InvalidRow, got {other:?}"),
    }
    assert!(err.to_string().starts_with("Row 1: Validation error"));

    mock.add(test::handlers::failure(test::status::FORBIDDEN));
    let err = fetch_all_validated::<EventSchema, Event>(query())
        .await
        .unwrap_err();
    assert!(matches!(err, VldClickhouseError::Clickhouse(_)), "{err:?}");
}

/// Given a batch about to be inserted,
/// when it is checked with `validate_rows`,
/// then the first bad row's index and issues are reported and an empty
/// batch passes.
#[test]
fn a_batch_reports_its_first_bad_row() {
    validate_rows::<EventSchema, Event>(&[]).unwrap();
    validate_row::<EventSchema, _>(&event("a", 0)).unwrap();
    let (i, e) =
        validate_rows::<EventSchema, _>(&[event("a", 0), event("b", 1), event("", 2)]).unwrap_err();
    assert_eq!(i, 2);
    assert_eq!(issues_of(&e), pairs(&[(".name", "too_small")]));
}
