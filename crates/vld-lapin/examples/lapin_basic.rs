//! Real RabbitMQ/Lapin example for `impl_to_lapin!`.
//!
//! Run:
//! cargo run -p vld-lapin --example lapin_basic

use vld_lapin::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct EventSchema {
        pub event: String => vld::string().min(1),
        pub retries: i64 => vld::number().int().min(0).max(10),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== vld-lapin example ===\n");

    let good = EventSchema {
        event: "user.created".into(),
        retries: 0,
    };
    let bad = EventSchema {
        event: "".into(),
        retries: 99,
    };

    println!("1) real Lapin connection (only impl_to_lapin API):");
    let conn = lapin::Connection::connect(
        "amqp://guest:guest@127.0.0.1:5672/%2f",
        lapin::ConnectionProperties::default(),
    )
    .await?;
    let channel = conn.create_channel().await?;
    impl_to_lapin!(channel);

    // Native lapin::Channel API is available through Deref.
    channel
        .basic_qos(10, lapin::options::BasicQosOptions::default())
        .await?;

    channel
        .queue_declare(
            "vld.events",
            lapin::options::QueueDeclareOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .publish(
            "",
            "vld.events",
            lapin::options::BasicPublishOptions::default(),
            lapin::BasicProperties::default(),
            &good,
        )
        .await?;
    println!("   [OK] Published to queue vld.events");

    let delivered = channel
        .basic_get(
            "vld.events",
            lapin::options::BasicGetOptions::default(),
        )
        .await?;
    if let Some(delivery) = delivered {
        let parsed: EventSchema = channel
            .ack_decode(&delivery, lapin::options::BasicAckOptions::default())
            .await?;
        println!("   [OK] Consumed and acked event: {}", parsed.event);
    } else {
        println!("   [WARN] Queue was empty");
    }

    println!("\n2) invalid payload is rejected before publish:");
    match channel
        .publish(
            "",
            "vld.events",
            lapin::options::BasicPublishOptions::default(),
            lapin::BasicProperties::default(),
            &bad,
        )
        .await
    {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   [OK] Validation failed: {e}"),
    }

    Ok(())
}
