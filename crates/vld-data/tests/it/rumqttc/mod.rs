//! `vld_data::rumqttc` behaviour tests: an `AsyncClient` built on a flume
//! channel (`AsyncClient::from_senders`) whose requests the test reads, and
//! incoming `Publish` packets built with `Publish::new`.

mod story_publish;
