//! Helpers shared by the redis test modules of one major, chiefly
//! [`FakeRedis`]: an in-memory Redis that parses the RESP commands the
//! connection wrappers send and answers like a server.
#![allow(dead_code)]

use super::vld_redis::VldRedisError;
use super::vld_redis::redis::{self, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

pub fn user(name: &str, email: &str) -> UserSchema {
    UserSchema {
        name: name.into(),
        email: email.into(),
    }
}

/// How a valid user is stored: the validated JSON value (keys sorted).
pub fn stored(u: &UserSchema) -> Vec<u8> {
    format!(r#"{{"email":"{}","name":"{}"}}"#, u.email, u.name).into_bytes()
}

#[derive(Debug, Clone)]
pub enum Entry {
    Str(Vec<u8>),
    Hash(BTreeMap<Vec<u8>, Vec<u8>>),
    List(VecDeque<Vec<u8>>),
    Set(BTreeSet<Vec<u8>>),
    /// `(member, score)`, kept sorted by score then member.
    ZSet(Vec<(Vec<u8>, f64)>),
}

/// In-memory Redis: keys, a log of every command received, published
/// messages, and an optional failure injected for the next command.
#[derive(Debug, Default)]
pub struct FakeRedis {
    pub data: HashMap<Vec<u8>, Entry>,
    /// Every command received, as its argument list.
    pub log: Vec<Vec<Vec<u8>>>,
    /// `(channel, message)` of every PUBLISH.
    pub published: Vec<(Vec<u8>, Vec<u8>)>,
    /// What PUBLISH answers (number of subscribers).
    pub subscribers: i64,
    /// When set, the next command fails with this I/O error text.
    pub fail_next: Option<String>,
}

/// Split a packed RESP command (`*N\r\n$len\r\narg\r\n…`) into its arguments.
pub fn parse_resp(mut b: &[u8]) -> Vec<Vec<u8>> {
    fn line<'a>(b: &mut &'a [u8]) -> &'a [u8] {
        let end = b.windows(2).position(|w| w == b"\r\n").expect("CRLF");
        let l = &b[..end];
        *b = &b[end + 2..];
        l
    }
    let head = line(&mut b);
    assert_eq!(head[0], b'*', "array header");
    let n: usize = std::str::from_utf8(&head[1..]).unwrap().parse().unwrap();
    (0..n)
        .map(|_| {
            let h = line(&mut b);
            assert_eq!(h[0], b'$', "bulk header");
            let len: usize = std::str::from_utf8(&h[1..]).unwrap().parse().unwrap();
            let arg = b[..len].to_vec();
            b = &b[len + 2..];
            arg
        })
        .collect()
}

fn bulk(b: &[u8]) -> Value {
    Value::BulkString(b.to_vec())
}

fn opt(b: Option<&Vec<u8>>) -> Value {
    b.map_or(Value::Nil, |b| bulk(b))
}

fn int(n: usize) -> Value {
    Value::Int(n as i64)
}

fn num<T: std::str::FromStr>(b: &[u8]) -> T
where
    T::Err: std::fmt::Debug,
{
    std::str::from_utf8(b).unwrap().parse().unwrap()
}

impl FakeRedis {
    pub fn new() -> Self {
        Self::default()
    }

    /// The names of the commands received, in order.
    pub fn commands(&self) -> Vec<String> {
        self.log
            .iter()
            .map(|c| String::from_utf8_lossy(&c[0]).into_owned())
            .collect()
    }

    pub fn list(&self, key: &str) -> Vec<Vec<u8>> {
        match self.data.get(key.as_bytes()) {
            Some(Entry::List(l)) => l.iter().cloned().collect(),
            None => vec![],
            other => panic!("{key} is {other:?}"),
        }
    }

    fn wrong_type() -> redis::RedisError {
        std::io::Error::other("WRONGTYPE").into()
    }

    /// Execute one command, as a server would.
    pub fn exec(&mut self, args: Vec<Vec<u8>>) -> redis::RedisResult<Value> {
        self.log.push(args.clone());
        if let Some(msg) = self.fail_next.take() {
            return Err(std::io::Error::other(msg).into());
        }
        let name = String::from_utf8_lossy(&args[0]).to_ascii_uppercase();
        let a = &args[1..];
        macro_rules! entry {
            ($key:expr, $variant:ident, $new:expr) => {
                match self
                    .data
                    .entry($key.clone())
                    .or_insert_with(|| Entry::$variant($new))
                {
                    Entry::$variant(v) => v,
                    _ => return Err(Self::wrong_type()),
                }
            };
        }
        macro_rules! read {
            ($key:expr, $variant:ident) => {
                match self.data.get($key) {
                    Some(Entry::$variant(v)) => Some(v),
                    None => None,
                    _ => return Err(Self::wrong_type()),
                }
            };
        }
        Ok(match name.as_str() {
            "SET" => {
                self.data.insert(a[0].clone(), Entry::Str(a[1].clone()));
                Value::Okay
            }
            "GET" => opt(read!(&a[0], Str)),
            "MSET" => {
                for kv in a.chunks(2) {
                    self.data.insert(kv[0].clone(), Entry::Str(kv[1].clone()));
                }
                Value::Okay
            }
            "MGET" => Value::Array(
                a.iter()
                    .map(|k| match self.data.get(k) {
                        Some(Entry::Str(v)) => bulk(v),
                        _ => Value::Nil,
                    })
                    .collect(),
            ),
            "DEL" => int(a.iter().filter(|k| self.data.remove(*k).is_some()).count()),
            "HSET" => {
                let h = entry!(a[0], Hash, BTreeMap::new());
                int(usize::from(h.insert(a[1].clone(), a[2].clone()).is_none()))
            }
            "HGET" => opt(read!(&a[0], Hash).and_then(|h| h.get(&a[1]))),
            "LPUSH" | "RPUSH" => {
                let l = entry!(a[0], List, VecDeque::new());
                if name == "LPUSH" {
                    l.push_front(a[1].clone());
                } else {
                    l.push_back(a[1].clone());
                }
                int(l.len())
            }
            "LPOP" | "RPOP" => {
                let popped = match self.data.get_mut(&a[0]) {
                    Some(Entry::List(l)) if name == "LPOP" => l.pop_front(),
                    Some(Entry::List(l)) => l.pop_back(),
                    None => None,
                    _ => return Err(Self::wrong_type()),
                };
                opt(popped.as_ref())
            }
            "SADD" => {
                let s = entry!(a[0], Set, BTreeSet::new());
                int(usize::from(s.insert(a[1].clone())))
            }
            "SMEMBERS" => Value::Array(
                read!(&a[0], Set)
                    .into_iter()
                    .flatten()
                    .map(|m| bulk(m))
                    .collect(),
            ),
            "ZADD" => {
                let z = entry!(a[0], ZSet, Vec::new());
                let score: f64 = num(&a[1]);
                let added = match z.iter_mut().find(|(m, _)| *m == a[2]) {
                    Some(e) => {
                        e.1 = score;
                        0
                    }
                    None => {
                        z.push((a[2].clone(), score));
                        1
                    }
                };
                z.sort_by(|x, y| x.1.total_cmp(&y.1).then_with(|| x.0.cmp(&y.0)));
                int(added)
            }
            "ZRANGE" => {
                let z = read!(&a[0], ZSet).cloned().unwrap_or_default();
                let len = z.len() as isize;
                let idx = |i: isize| if i < 0 { len + i } else { i };
                let (start, stop) = (idx(num(&a[1])).max(0), idx(num(&a[2])).min(len - 1));
                Value::Array(
                    (start..=stop)
                        .filter(|_| start <= stop)
                        .map(|i| bulk(&z[i as usize].0))
                        .collect(),
                )
            }
            "PUBLISH" => {
                self.published.push((a[0].clone(), a[1].clone()));
                Value::Int(self.subscribers)
            }
            other => panic!("FakeRedis: unsupported command {other}"),
        })
    }
}

impl redis::ConnectionLike for FakeRedis {
    fn req_packed_command(&mut self, cmd: &[u8]) -> redis::RedisResult<Value> {
        self.exec(parse_resp(cmd))
    }
    fn req_packed_commands(
        &mut self,
        _cmd: &[u8],
        _offset: usize,
        _count: usize,
    ) -> redis::RedisResult<Vec<Value>> {
        panic!("FakeRedis: the wrappers send no pipelines")
    }
    fn get_db(&self) -> i64 {
        0
    }
    fn check_connection(&mut self) -> bool {
        true
    }
    fn is_open(&self) -> bool {
        true
    }
}

#[cfg(feature = "redis-aio")]
impl redis::aio::ConnectionLike for FakeRedis {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a redis::Cmd) -> redis::RedisFuture<'a, Value> {
        let r = self.exec(parse_resp(&cmd.get_packed_command()));
        Box::pin(async move { r })
    }
    fn req_packed_commands<'a>(
        &'a mut self,
        _cmd: &'a redis::Pipeline,
        _offset: usize,
        _count: usize,
    ) -> redis::RedisFuture<'a, Vec<Value>> {
        panic!("FakeRedis: the wrappers send no pipelines")
    }
    fn get_db(&self) -> i64 {
        0
    }
}

/// `(path, code)` of every issue of a `Validation` error (panics otherwise).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldRedisError>) -> Vec<(String, String)> {
    match r {
        Err(VldRedisError::Validation(e)) => e
            .issues
            .iter()
            .map(|i| {
                (
                    i.path.iter().map(ToString::to_string).collect(),
                    i.code.key().to_string(),
                )
            })
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(path, code)` pairs for comparing with [`issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}
