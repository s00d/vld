//! [`FakeBroker`]: an in-memory AMQP 0-9-1 broker on a localhost socket,
//! enough for lapin to connect, open a channel, declare queues, publish
//! (optionally with publisher confirms), `basic.get` and settle messages.
//! Everything it receives is recorded in [`State`].

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

const FRAME_END: u8 = 0xCE;

/// How a delivery was settled by the client.
#[derive(Debug, Clone, PartialEq)]
pub enum Settle {
    Ack { tag: u64 },
    Nack { tag: u64, requeue: bool },
    Reject { tag: u64, requeue: bool },
}

#[derive(Debug, Clone)]
pub struct Message {
    pub body: Vec<u8>,
    pub redelivered: bool,
}

/// Everything the broker holds and saw.
#[derive(Debug, Default)]
pub struct State {
    pub queues: HashMap<String, VecDeque<Message>>,
    /// `(exchange, routing key, body)` of every publish.
    pub published: Vec<(String, String, Vec<u8>)>,
    /// Settlements in arrival order.
    pub settled: Vec<Settle>,
    /// Delivered and not yet settled: tag → (queue, message).
    pub unacked: HashMap<u64, (String, Message)>,
    /// Answer publisher confirms with `basic.nack` instead of `basic.ack`.
    pub nack_publishes: bool,
    next_tag: u64,
}

/// A running broker; connect lapin to [`FakeBroker::uri`].
pub struct FakeBroker {
    pub uri: String,
    pub state: Arc<Mutex<State>>,
}

impl FakeBroker {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let uri = format!("amqp://guest:guest@{}/%2f", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let state = shared.clone();
                std::thread::spawn(move || {
                    let _ = Session {
                        stream,
                        state,
                        confirm: false,
                        publish_seq: 0,
                    }
                    .run();
                });
            }
        });
        Self { uri, state }
    }

    pub fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }
}

// ---------------------------------------------------------------- encoding

#[derive(Default)]
struct Out(Vec<u8>);

impl Out {
    fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn u64(mut self, v: u64) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn short(mut self, s: &str) -> Self {
        self.0.push(s.len() as u8);
        self.0.extend_from_slice(s.as_bytes());
        self
    }
    fn long(self, b: &[u8]) -> Self {
        let mut s = self.u32(b.len() as u32);
        s.0.extend_from_slice(b);
        s
    }
    fn method(class: u16, method: u16) -> Self {
        Out::default().u16(class).u16(method)
    }
}

/// A field table of boolean flags (`{"publisher_confirms": true, …}`).
fn bool_table(keys: &[&str]) -> Vec<u8> {
    let mut t = Out::default();
    for k in keys {
        t = t.short(k).u8(b't').u8(1);
    }
    t.0
}

struct In<'a>(&'a [u8]);

impl In<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.0[0];
        self.0 = &self.0[1..];
        v
    }
    fn u16(&mut self) -> u16 {
        let v = u16::from_be_bytes(self.0[..2].try_into().unwrap());
        self.0 = &self.0[2..];
        v
    }
    fn u64(&mut self) -> u64 {
        let v = u64::from_be_bytes(self.0[..8].try_into().unwrap());
        self.0 = &self.0[8..];
        v
    }
    fn short(&mut self) -> String {
        let n = self.u8() as usize;
        let s = String::from_utf8(self.0[..n].to_vec()).unwrap();
        self.0 = &self.0[n..];
        s
    }
}

// ----------------------------------------------------------------- session

struct Session {
    stream: TcpStream,
    state: Arc<Mutex<State>>,
    confirm: bool,
    publish_seq: u64,
}

impl Session {
    fn send(&mut self, kind: u8, channel: u16, payload: &[u8]) -> std::io::Result<()> {
        let mut f = Out::default()
            .u8(kind)
            .u16(channel)
            .u32(payload.len() as u32)
            .0;
        f.extend_from_slice(payload);
        f.push(FRAME_END);
        self.stream.write_all(&f)
    }

    fn method(&mut self, channel: u16, m: Out) -> std::io::Result<()> {
        self.send(1, channel, &m.0)
    }

    fn read_frame(&mut self) -> std::io::Result<(u8, u16, Vec<u8>)> {
        let mut head = [0u8; 7];
        self.stream.read_exact(&mut head)?;
        let size = u32::from_be_bytes(head[3..7].try_into().unwrap()) as usize;
        let mut payload = vec![0; size + 1];
        self.stream.read_exact(&mut payload)?;
        assert_eq!(payload.pop(), Some(FRAME_END));
        Ok((head[0], u16::from_be_bytes([head[1], head[2]]), payload))
    }

    /// Content header + body frames following a `basic.publish`.
    fn read_content(&mut self) -> std::io::Result<Vec<u8>> {
        let (kind, _, header) = self.read_frame()?;
        assert_eq!(kind, 2, "content header");
        let size = u64::from_be_bytes(header[4..12].try_into().unwrap()) as usize;
        let mut body = Vec::with_capacity(size);
        while body.len() < size {
            let (kind, _, chunk) = self.read_frame()?;
            assert_eq!(kind, 3, "content body");
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    fn run(mut self) -> std::io::Result<()> {
        let mut proto = [0u8; 8];
        self.stream.read_exact(&mut proto)?;
        assert_eq!(&proto, b"AMQP\x00\x00\x09\x01");
        let caps = bool_table(&[
            "publisher_confirms",
            "basic.nack",
            "consumer_cancel_notify",
            "connection.blocked",
        ]);
        let props = Out::default().short("capabilities").u8(b'F').long(&caps).0;
        let start = Out::method(10, 10).u8(0).u8(9).long(&props);
        self.method(0, start.long(b"PLAIN").long(b"en_US"))?;

        loop {
            let (kind, channel, payload) = self.read_frame()?;
            if kind != 1 {
                continue; // heartbeats
            }
            let mut a = In(&payload);
            let (class, method) = (a.u16(), a.u16());
            match (class, method) {
                (10, 11) => self.method(0, Out::method(10, 30).u16(2047).u32(131_072).u16(0))?,
                (10, 31) => {}
                (10, 40) => self.method(0, Out::method(10, 41).short(""))?,
                (10, 50) => {
                    self.method(0, Out::method(10, 51))?;
                    return Ok(());
                }
                (10, 51) => return Ok(()),
                (20, 10) => self.method(channel, Out::method(20, 11).long(b""))?,
                (20, 40) => self.method(channel, Out::method(20, 41))?,
                (20, 41) => {}
                (85, 10) => {
                    self.confirm = true;
                    if a.u8() & 1 == 0 {
                        self.method(channel, Out::method(85, 11))?;
                    }
                }
                (50, 10) => {
                    a.u16();
                    let queue = a.short();
                    let nowait = a.u8() & 0b1_0000 != 0;
                    let count = {
                        let mut s = self.state.lock().unwrap();
                        s.queues.entry(queue.clone()).or_default().len() as u32
                    };
                    if !nowait {
                        let ok = Out::method(50, 11).short(&queue).u32(count).u32(0);
                        self.method(channel, ok)?;
                    }
                }
                (60, 40) => {
                    a.u16();
                    let (exchange, key) = (a.short(), a.short());
                    let body = self.read_content()?;
                    let nack = {
                        let mut s = self.state.lock().unwrap();
                        s.published
                            .push((exchange.clone(), key.clone(), body.clone()));
                        if exchange.is_empty() {
                            if let Some(q) = s.queues.get_mut(&key) {
                                q.push_back(Message {
                                    body,
                                    redelivered: false,
                                });
                            }
                        }
                        s.nack_publishes
                    };
                    if self.confirm {
                        self.publish_seq += 1;
                        let m = if nack { 120 } else { 80 };
                        self.method(channel, Out::method(60, m).u64(self.publish_seq).u8(0))?;
                    }
                }
                (60, 70) => {
                    a.u16();
                    let queue = a.short();
                    let no_ack = a.u8() & 1 != 0;
                    let got = {
                        let mut s = self.state.lock().unwrap();
                        let msg = s.queues.get_mut(&queue).and_then(VecDeque::pop_front);
                        let left = s.queues.get(&queue).map_or(0, VecDeque::len) as u32;
                        msg.map(|m| {
                            s.next_tag += 1;
                            let tag = s.next_tag;
                            if !no_ack {
                                s.unacked.insert(tag, (queue.clone(), m.clone()));
                            }
                            (tag, m, left)
                        })
                    };
                    match got {
                        None => self.method(channel, Out::method(60, 72).short(""))?,
                        Some((tag, m, left)) => {
                            let ok = Out::method(60, 71)
                                .u64(tag)
                                .u8(u8::from(m.redelivered))
                                .short("")
                                .short(&queue)
                                .u32(left);
                            self.method(channel, ok)?;
                            let header = Out::default()
                                .u16(60)
                                .u16(0)
                                .u64(m.body.len() as u64)
                                .u16(0);
                            self.send(2, channel, &header.0)?;
                            if !m.body.is_empty() {
                                self.send(3, channel, &m.body)?;
                            }
                        }
                    }
                }
                (60, 80) | (60, 120) | (60, 90) => {
                    let tag = a.u64();
                    let bits = a.u8();
                    let mut s = self.state.lock().unwrap();
                    let (settle, requeue) = match method {
                        80 => (Settle::Ack { tag }, false),
                        120 => {
                            let requeue = bits & 0b10 != 0;
                            (Settle::Nack { tag, requeue }, requeue)
                        }
                        _ => {
                            let requeue = bits & 1 != 0;
                            (Settle::Reject { tag, requeue }, requeue)
                        }
                    };
                    s.settled.push(settle);
                    if let Some((queue, mut m)) = s.unacked.remove(&tag) {
                        if requeue {
                            m.redelivered = true;
                            s.queues.entry(queue).or_default().push_front(m);
                        }
                    }
                }
                other => panic!("FakeBroker: unsupported method {other:?}"),
            }
        }
    }
}
