//! `vld-redis` behaviour tests: one test binary, modules are stories and a
//! model through an in-memory fake Redis server (`common::FakeRedis`, which
//! speaks the real RESP command encoding); see `TESTING.md` at the
//! repository root.

mod common;

mod model_commands;
mod story_async;
mod story_commands;
mod story_rejects;
