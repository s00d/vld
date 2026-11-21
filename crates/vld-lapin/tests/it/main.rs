//! `vld-lapin` behaviour tests: one test binary, modules are stories and a
//! model through lapin connected to an in-memory AMQP broker
//! (`common::broker::FakeBroker`); see `TESTING.md` at the repository root.

mod common;

mod model_queue;
mod story_publish;
mod story_settle;
