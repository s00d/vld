vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct EventSchema {
        pub event: String => vld::string().min(1),
        pub retries: i64 => vld::number().int().min(0).max(10),
    }
}

#[test]
fn impl_to_lapin_api_compiles() {
    async fn compile_check(
        channel: vld_lapin::LapinChannel,
        delivery: lapin::message::Delivery,
        get_msg: lapin::message::BasicGetMessage,
        evt: EventSchema,
    ) -> Result<(), vld_lapin::VldLapinError> {
        // Native lapin method through Deref.
        channel
            .basic_qos(10, lapin::options::BasicQosOptions::default())
            .await?;

        channel
            .publish(
                "",
                "events.user",
                lapin::options::BasicPublishOptions::default(),
                lapin::BasicProperties::default(),
                &evt,
            )
            .await?;
        let _decoded_bytes: EventSchema = channel.decode_bytes(br#"{"event":"x","retries":1}"#)?;
        let _decoded: EventSchema = channel.decode_delivery(&delivery)?;
        let _decoded_get: EventSchema = channel.decode_get(&get_msg)?;
        let _acked: EventSchema = channel
            .ack_decode(&delivery, lapin::options::BasicAckOptions::default())
            .await?;
        let _acked_get: EventSchema = channel
            .ack_decode_get(&get_msg, lapin::options::BasicAckOptions::default())
            .await?;
        let _nacked: EventSchema = channel
            .nack_decode(&delivery, lapin::options::BasicNackOptions::default())
            .await?;
        let _nacked_get: EventSchema = channel
            .nack_decode_get(&get_msg, lapin::options::BasicNackOptions::default())
            .await?;
        let _rejected: EventSchema = channel
            .reject_decode(&delivery, lapin::options::BasicRejectOptions::default())
            .await?;
        let _rejected_get: EventSchema = channel
            .reject_decode_get(&get_msg, lapin::options::BasicRejectOptions::default())
            .await?;
        Ok(())
    }

    let _ = compile_check;
}
