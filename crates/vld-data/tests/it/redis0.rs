//! `vld_data::redis0`: the shared redis tests (`redis/`) compiled against
//! redis `>=0.32, <1`, through an in-memory fake Redis server
//! (`common::FakeRedis`, which speaks the real RESP command encoding).

use vld_data::redis0 as vld_redis;

#[allow(clippy::duplicate_mod)]
#[path = "redis/common.rs"]
mod common;
#[allow(clippy::duplicate_mod)]
#[path = "redis/model_commands.rs"]
mod model_commands;
#[allow(clippy::duplicate_mod)]
#[path = "redis/story_async.rs"]
mod story_async;
#[allow(clippy::duplicate_mod)]
#[path = "redis/story_commands.rs"]
mod story_commands;
#[allow(clippy::duplicate_mod)]
#[path = "redis/story_rejects.rs"]
mod story_rejects;
