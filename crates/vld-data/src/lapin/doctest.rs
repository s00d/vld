
vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct EventSchema {
        pub event: String => vld::string().min(1),
    }
}

async fn roundtrip(channel: &LapinChannel) -> Result<(), VldLapinError> {
    let event = EventSchema { event: "user.created".into() };
    channel
        .publish("", "events", Default::default(), Default::default(), &event)
        .await?;

    if let Some(message) = channel.basic_get("events", Default::default()).await? {
        // Ack when valid, nack when invalid: every message is settled.
        let parsed: EventSchema = channel
            .ack_or_nack_decode(&message, Default::default(), Default::default())
            .await?;
        println!("{}", parsed.event);
    }
    Ok(())
}
