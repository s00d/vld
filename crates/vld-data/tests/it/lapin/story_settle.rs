//! Stories: poison messages and the settle helpers (`ack_decode`,
//! `ack_or_nack_decode`, `nack_decode`, `reject_decode`, and their `_get`
//! forms): what the broker sees for valid and invalid deliveries.

use super::common::broker::Settle;
use super::common::*;
use super::lapin::{self, options::BasicNackOptions};
use super::vld_lapin::prelude::*;

const BAD: &[u8] = br#"{"event":"","retries":0}"#;

async fn queue_with(ch: &LapinChannel, bodies: &[&[u8]]) {
    ch.queue_declare("jobs", Default::default(), Default::default())
        .await
        .unwrap();
    for b in bodies {
        publish_raw(ch, "jobs", b).await;
    }
}

/// Given a poison message (schema-invalid) at the head of the queue,
/// when it is consumed with `ack_decode`,
/// then the validation error is returned and the delivery stays unacked on
/// the broker (nothing settled).
#[tokio::test(flavor = "multi_thread")]
async fn ack_decode_leaves_a_poison_message_unacked() {
    let s = setup().await;
    let ch = &s.channel;
    queue_with(ch, &[BAD]).await;
    let msg = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        issues(ch.ack_decode::<Evt>(&msg, Default::default()).await),
        pairs(&[(".event", "too_small")])
    );
    sync(ch).await;
    let st = s.broker.state();
    assert!(st.settled.is_empty());
    assert_eq!(st.unacked.keys().copied().collect::<Vec<_>>(), [1]);
}

/// Given a poison message, a non-JSON message and a valid one,
/// when each is consumed with `ack_or_nack_decode` (nack without requeue),
/// then the bad ones are nacked and their errors returned, the valid one is
/// acked and returned, and nothing is left in the queue.
#[tokio::test(flavor = "multi_thread")]
async fn ack_or_nack_decode_settles_every_message() {
    let s = setup().await;
    let ch = &s.channel;
    let good = evt("ok", 2);
    queue_with(ch, &[BAD, b"not json", &payload(&good)]).await;
    let mut got = vec![];
    while let Some(msg) = ch.basic_get("jobs", Default::default()).await.unwrap() {
        got.push(
            ch.ack_or_nack_decode::<Evt>(&msg, Default::default(), Default::default())
                .await,
        );
    }
    assert_eq!(got.len(), 3);
    assert_eq!(issues(got.remove(0)), pairs(&[(".event", "too_small")]));
    assert!(matches!(
        got.remove(0),
        Err(VldLapinError::Deserialization(_))
    ));
    assert_eq!(got.remove(0).unwrap(), good);
    sync(ch).await;
    assert_eq!(
        s.broker.state().settled,
        [
            Settle::Nack {
                tag: 1,
                requeue: false
            },
            Settle::Nack {
                tag: 2,
                requeue: false
            },
            Settle::Ack { tag: 3 }
        ]
    );
}

/// Given a poison message and a non-JSON one,
/// when they are consumed with `nack_decode` (requeue) and `reject_decode`,
/// then the first is nacked with requeue and comes back redelivered, the
/// second is rejected; both errors are returned.
#[tokio::test(flavor = "multi_thread")]
async fn nack_and_reject_settle_only_invalid_messages() {
    let s = setup().await;
    let ch = &s.channel;
    queue_with(ch, &[BAD, b"{"]).await;

    let msg = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    let requeue = BasicNackOptions {
        requeue: true,
        ..Default::default()
    };
    assert_eq!(
        issues(ch.nack_decode::<Evt>(&msg, requeue).await),
        pairs(&[(".event", "too_small")])
    );
    sync(ch).await;
    let again = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    assert!(again.delivery.redelivered);
    assert_eq!(again.data, BAD);

    let msg = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    let err = ch
        .reject_decode::<Evt>(&msg, Default::default())
        .await
        .unwrap_err();
    assert!(matches!(err, VldLapinError::Deserialization(_)), "{err:?}");
    sync(ch).await;
    assert_eq!(
        s.broker.state().settled,
        [
            Settle::Nack {
                tag: 1,
                requeue: true
            },
            Settle::Reject {
                tag: 3,
                requeue: false
            }
        ]
    );
}

/// Given valid messages,
/// when they are consumed with `nack_decode` / `reject_decode` and their
/// `_get` forms,
/// then the events are returned and the deliveries left unsettled, so the
/// caller acks them after processing; `ack_decode_get` acks a valid one.
#[tokio::test(flavor = "multi_thread")]
async fn valid_messages_are_left_for_the_caller() {
    use lapin::options::BasicRejectOptions;
    let s = setup().await;
    let ch = &s.channel;
    let events: Vec<Evt> = (0..5).map(|i| evt("e", i)).collect();
    let bodies: Vec<Vec<u8>> = events.iter().map(payload).collect();
    queue_with(ch, &bodies.iter().map(Vec::as_slice).collect::<Vec<_>>()).await;

    let mut msgs = vec![];
    while let Some(m) = ch.basic_get("jobs", Default::default()).await.unwrap() {
        msgs.push(m);
    }
    let nack = BasicNackOptions::default();
    let reject = BasicRejectOptions::default();
    assert_eq!(
        ch.nack_decode::<Evt>(&msgs[0], nack).await.unwrap(),
        events[0]
    );
    assert_eq!(
        ch.reject_decode::<Evt>(&msgs[1], reject).await.unwrap(),
        events[1]
    );
    assert_eq!(
        ch.nack_decode_get::<Evt>(&msgs[2], nack).await.unwrap(),
        events[2]
    );
    assert_eq!(
        ch.reject_decode_get::<Evt>(&msgs[3], reject).await.unwrap(),
        events[3]
    );
    sync(ch).await;
    assert!(s.broker.state().settled.is_empty());

    assert_eq!(
        ch.ack_decode_get::<Evt>(&msgs[4], Default::default())
            .await
            .unwrap(),
        events[4]
    );
    assert_eq!(
        ch.ack_decode::<Evt>(&msgs[0], Default::default())
            .await
            .unwrap(),
        events[0]
    );
    sync(ch).await;
    assert_eq!(
        s.broker.state().settled,
        [Settle::Ack { tag: 5 }, Settle::Ack { tag: 1 }]
    );
}

/// Given invalid messages fetched with `basic_get`,
/// when they go through `nack_decode_get` and `reject_decode_get`,
/// then they are nacked / rejected and the validation errors returned.
#[tokio::test(flavor = "multi_thread")]
async fn get_forms_settle_invalid_messages() {
    use lapin::options::BasicRejectOptions;
    let s = setup().await;
    let ch = &s.channel;
    queue_with(ch, &[BAD, BAD]).await;
    let a = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    let b = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    assert!(
        ch.nack_decode_get::<Evt>(&a, BasicNackOptions::default())
            .await
            .is_err()
    );
    assert!(
        ch.reject_decode_get::<Evt>(&b, BasicRejectOptions { requeue: true })
            .await
            .is_err()
    );
    assert!(
        ch.ack_decode_get::<Evt>(&b, Default::default())
            .await
            .is_err()
    );
    sync(ch).await;
    assert_eq!(
        s.broker.state().settled,
        [
            Settle::Nack {
                tag: 1,
                requeue: false
            },
            Settle::Reject {
                tag: 2,
                requeue: true
            }
        ]
    );
}

/// Given deliveries the application already acked natively,
/// when they go through the settle helpers again (an invalid one through
/// `nack_decode`, a valid one through `ack_or_nack_decode`),
/// then the settle failure is returned instead of the decode result: lapin 2
/// fails the second settle (`Lapin`), lapin 3 / 4 report it sent nothing
/// (`NotSettled`). Either way nothing more reaches the broker.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::let_unit_value)] // `ack` gives `()` on lapin 2, `bool` on 3/4
async fn settling_twice() {
    let s = setup().await;
    let ch = &s.channel;
    let good = evt("ok", 0);
    queue_with(ch, &[BAD, &payload(&good)]).await;
    let bad = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    let ok = ch
        .basic_get("jobs", Default::default())
        .await
        .unwrap()
        .unwrap();
    for m in [&bad, &ok] {
        let _ = m.ack(Default::default()).await.unwrap();
    }

    let nacked = ch.nack_decode::<Evt>(&bad, Default::default()).await;
    let acked = ch
        .ack_or_nack_decode::<Evt>(&ok, Default::default(), Default::default())
        .await;
    super::compat::check_settled_twice(nacked, acked);
    sync(ch).await;
    assert_eq!(
        s.broker.state().settled,
        [Settle::Ack { tag: 1 }, Settle::Ack { tag: 2 }]
    );
}
