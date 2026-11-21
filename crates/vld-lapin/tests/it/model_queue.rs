//! Model: random batches of producer messages through one queue.
//!
//! proptest draws a sequence of sends: valid events and invalid ones through
//! the validating `publish`, and raw payloads from another producer (valid,
//! schema-invalid, not JSON). The queue is then drained with
//! `ack_or_nack_decode`. The model predicts which publishes are rejected
//! (with their exact issues) and never reach the broker, the decode result of
//! each delivery in FIFO order, and the broker's settlements: ack for a valid
//! delivery, nack (no requeue) for every other one.

use crate::common::broker::Settle;
use crate::common::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use vld_lapin::prelude::*;

#[derive(Debug, Clone)]
enum Send {
    Publish(Evt),
    Raw(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Event(Evt),
    Invalid(Vec<(String, String)>),
    NotJson,
}

fn event() -> impl Strategy<Value = Evt> {
    (prop_oneof![Just(String::new()), "[a-z.]{1,8}"], -2i64..13)
        .prop_map(|(event, retries)| Evt { event, retries })
}

fn send() -> impl Strategy<Value = Send> {
    prop_oneof![
        event().prop_map(Send::Publish),
        event().prop_map(|e| Send::Raw(payload(&e))),
        Just(Send::Raw(b"not json".to_vec())),
        Just(Send::Raw(br#"{"event":"x"}"#.to_vec())),
    ]
}

/// The schema's issues for an event payload.
fn verdict(e: &Evt) -> Vec<(String, String)> {
    let mut out = vec![];
    if e.event.is_empty() {
        out.push((".event".into(), "too_small".into()));
    }
    if e.retries < 0 {
        out.push((".retries".into(), "too_small".into()));
    }
    if e.retries > 10 {
        out.push((".retries".into(), "too_big".into()));
    }
    out
}

fn outcome(body: &[u8], sent: Option<&Evt>) -> Out {
    match sent {
        Some(e) if verdict(e).is_empty() => Out::Event(e.clone()),
        Some(e) => Out::Invalid(verdict(e)),
        None if body == b"not json" => Out::NotJson,
        None => Out::Invalid(vec![(".retries".into(), "missing_field".into())]),
    }
}

fn observe(r: Result<Evt, VldLapinError>) -> Out {
    match r {
        Ok(e) => Out::Event(e),
        Err(VldLapinError::Deserialization(_)) => Out::NotJson,
        r => Out::Invalid(issues(r)),
    }
}

async fn check(sends: Vec<Send>) -> Result<(), TestCaseError> {
    let s = setup().await;
    let ch = &s.channel;
    ch.queue_declare("q", Default::default(), Default::default())
        .await
        .unwrap();

    let mut queued: Vec<Out> = vec![];
    for send in &sends {
        match send {
            Send::Publish(e) => {
                let r = ch
                    .publish("", "q", Default::default(), Default::default(), e)
                    .await;
                let v = verdict(e);
                if v.is_empty() {
                    prop_assert!(r.is_ok(), "{r:?}");
                    queued.push(Out::Event(e.clone()));
                } else {
                    prop_assert_eq!(issues(r), v);
                }
            }
            Send::Raw(body) => {
                publish_raw(ch, "q", body).await;
                let parsed: Option<Evt> = serde_json::from_slice::<serde_json::Value>(body)
                    .ok()
                    .filter(|v| v.get("retries").is_some())
                    .map(|v| evt(v["event"].as_str().unwrap(), v["retries"].as_i64().unwrap()));
                queued.push(outcome(body, parsed.as_ref()));
            }
        }
    }

    let mut got = vec![];
    while let Some(msg) = ch.basic_get("q", Default::default()).await.unwrap() {
        got.push(observe(
            ch.ack_or_nack_decode(&msg, Default::default(), Default::default())
                .await,
        ));
    }
    prop_assert_eq!(&got, &queued);
    sync(ch).await;
    let want: Vec<Settle> = queued
        .iter()
        .zip(1u64..)
        .map(|(o, tag)| match o {
            Out::Event(_) => Settle::Ack { tag },
            _ => Settle::Nack {
                tag,
                requeue: false,
            },
        })
        .collect();
    let st = s.broker.state();
    prop_assert_eq!(&st.settled, &want);
    prop_assert_eq!(st.published.len(), queued.len());
    Ok(())
}

#[test]
fn model_queue_batches() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    TestRunner::new(Config::with_cases(48))
        .run(&prop::collection::vec(send(), 0..12), |sends| {
            rt.block_on(check(sends))
        })
        .unwrap();
}
