//! Model: random command sequences against the fake server.
//!
//! proptest draws sequences of writes (`set`, `hset`, `lpush`, `rpush`,
//! `sadd`) of users that are valid or break the schema in one or both
//! fields, reads (`get`, `hget`, `lpop`, `rpop`, `smembers`) and raw writes
//! of corrupt data by another client. A model of Redis (strings, a hash, a
//! deque, a set) plus the schema predicts every result: invalid writes fail
//! with their exact issues and change nothing, reads give back exactly the
//! users written, and corrupt data reads as the matching error.

use crate::common::*;
use proptest::prelude::*;
use std::collections::{BTreeSet, HashMap, VecDeque};
use vld_redis::redis::Commands;
use vld_redis::{RedisConn, VldRedisError};

#[derive(Debug, Clone)]
enum Op {
    Set(u8, UserSchema),
    Get(u8),
    HSet(u8, UserSchema),
    HGet(u8),
    LPush(UserSchema),
    RPush(UserSchema),
    LPop,
    RPop,
    SAdd(UserSchema),
    SMembers,
    /// Another client writes raw bytes under a string key.
    Corrupt(u8, &'static str),
}

/// The result of one op as the tests compare it.
#[derive(Debug, Clone, PartialEq)]
enum Out {
    Done,
    Count(usize),
    One(Option<UserSchema>),
    Many(Vec<UserSchema>),
    Invalid(Vec<(String, String)>),
    NotJson,
}

fn user_strategy() -> impl Strategy<Value = UserSchema> {
    let name = prop_oneof![Just(String::new()), "[A-Z][a-z]{0,5}"];
    let email = prop_oneof![
        "[a-z]{1,5}@example\\.com",
        Just("nope".to_string()),
        Just(String::new())
    ];
    (name, email).prop_map(|(name, email)| UserSchema { name, email })
}

fn op() -> impl Strategy<Value = Op> {
    let key = 0u8..3;
    prop_oneof![
        (key.clone(), user_strategy()).prop_map(|(k, u)| Op::Set(k, u)),
        key.clone().prop_map(Op::Get),
        (key.clone(), user_strategy()).prop_map(|(k, u)| Op::HSet(k, u)),
        key.clone().prop_map(Op::HGet),
        user_strategy().prop_map(Op::LPush),
        user_strategy().prop_map(Op::RPush),
        Just(Op::LPop),
        Just(Op::RPop),
        user_strategy().prop_map(Op::SAdd),
        Just(Op::SMembers),
        (
            key,
            prop_oneof![Just("not json"), Just(r#"{"name":"","email":"a@b.co"}"#)]
        )
            .prop_map(|(k, raw)| Op::Corrupt(k, raw)),
    ]
}

/// The schema's verdict on a user: its `(path, code)` issues.
fn verdict(u: &UserSchema) -> Vec<(String, String)> {
    let mut out = vec![];
    if u.name.is_empty() {
        out.push((".name".into(), "too_small".into()));
    }
    if !u.email.ends_with("@example.com") {
        out.push((".email".into(), "invalid_string".into()));
    }
    out
}

/// What a string key holds in the model.
#[derive(Debug, Clone)]
enum Slot {
    User(UserSchema),
    Raw(&'static str),
}

#[derive(Default)]
struct Model {
    strings: HashMap<u8, Slot>,
    hash: HashMap<u8, UserSchema>,
    list: VecDeque<UserSchema>,
    set: Vec<UserSchema>,
}

impl Model {
    fn apply(&mut self, op: &Op) -> Out {
        let write = |u: &UserSchema| {
            let v = verdict(u);
            if v.is_empty() {
                None
            } else {
                Some(Out::Invalid(v))
            }
        };
        match op {
            Op::Set(k, u) => write(u).unwrap_or_else(|| {
                self.strings.insert(*k, Slot::User(u.clone()));
                Out::Done
            }),
            Op::Get(k) => match self.strings.get(k) {
                None => Out::One(None),
                Some(Slot::User(u)) => Out::One(Some(u.clone())),
                Some(Slot::Raw(r)) if r.starts_with('{') => {
                    Out::Invalid(vec![(".name".into(), "too_small".into())])
                }
                Some(Slot::Raw(_)) => Out::NotJson,
            },
            Op::HSet(f, u) => write(u).unwrap_or_else(|| {
                self.hash.insert(*f, u.clone());
                Out::Done
            }),
            Op::HGet(f) => Out::One(self.hash.get(f).cloned()),
            Op::LPush(u) => write(u).unwrap_or_else(|| {
                self.list.push_front(u.clone());
                Out::Count(self.list.len())
            }),
            Op::RPush(u) => write(u).unwrap_or_else(|| {
                self.list.push_back(u.clone());
                Out::Count(self.list.len())
            }),
            Op::LPop => Out::One(self.list.pop_front()),
            Op::RPop => Out::One(self.list.pop_back()),
            Op::SAdd(u) => write(u).unwrap_or_else(|| {
                if self.set.contains(u) {
                    Out::Count(0)
                } else {
                    self.set.push(u.clone());
                    Out::Count(1)
                }
            }),
            Op::SMembers => Out::Many(self.set.clone()),
            Op::Corrupt(k, raw) => {
                self.strings.insert(*k, Slot::Raw(raw));
                Out::Done
            }
        }
    }
}

fn observe<T>(r: Result<T, VldRedisError>, ok: impl FnOnce(T) -> Out) -> Out {
    match r {
        Ok(v) => ok(v),
        Err(VldRedisError::Validation(_)) => Out::Invalid(issues(r.map(|_| ()))),
        Err(VldRedisError::Deserialization(_)) => Out::NotJson,
        Err(e) => panic!("unexpected error {e:?}"),
    }
}

fn run(conn: &mut RedisConn<FakeRedis>, op: &Op) -> Out {
    let key = |k: &u8| format!("user:{k}");
    match op {
        Op::Set(k, u) => observe(conn.set(key(k), u), |()| Out::Done),
        Op::Get(k) => observe(conn.get(key(k)), Out::One),
        Op::HSet(f, u) => observe(conn.hset("h", f, u), |()| Out::Done),
        Op::HGet(f) => observe(conn.hget("h", f), Out::One),
        Op::LPush(u) => observe(conn.lpush("l", u), Out::Count),
        Op::RPush(u) => observe(conn.rpush("l", u), Out::Count),
        Op::LPop => observe(conn.lpop("l"), Out::One),
        Op::RPop => observe(conn.rpop("l"), Out::One),
        Op::SAdd(u) => observe(conn.sadd("s", u), Out::Count),
        Op::SMembers => observe(conn.smembers("s"), Out::Many),
        Op::Corrupt(k, raw) => {
            let _: () = Commands::set(&mut **conn, key(k), *raw).unwrap();
            Out::Done
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn model_command_sequences(ops in prop::collection::vec(op(), 1..40)) {
        let mut conn = RedisConn::new(FakeRedis::new());
        let mut model = Model::default();
        for op in &ops {
            let sent_before = conn.log.len();
            let want = model.apply(op);
            let mut got = run(&mut conn, op);
            if let (Out::Many(g), Out::Many(w)) = (&mut got, &want) {
                // Redis sets are unordered: compare as sets.
                let g: BTreeSet<_> = g.iter().map(|u| (u.name.clone(), u.email.clone())).collect();
                let w: BTreeSet<_> = w.iter().map(|u| (u.name.clone(), u.email.clone())).collect();
                prop_assert_eq!(g, w, "{:?}", op);
                continue;
            }
            prop_assert_eq!(&got, &want, "{:?}", op);
            let is_write = matches!(op, Op::Set(..) | Op::HSet(..) | Op::LPush(_) | Op::RPush(_) | Op::SAdd(_));
            if is_write && matches!(want, Out::Invalid(_)) {
                prop_assert_eq!(conn.log.len(), sent_before, "invalid write was sent: {:?}", op);
            }
        }
    }
}
