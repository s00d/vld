//! [`FakeNats`]: an in-memory NATS server on a localhost socket speaking the
//! text protocol async-nats uses without headers: `INFO`, `CONNECT`,
//! `PING` / `PONG`, `SUB`, `UNSUB`, `PUB` and `MSG`, with `*` / `>` subject
//! wildcards. Every `PUB` is recorded in [`FakeNats::published`].

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// One subscription: the subject pattern, its sid and the subscriber's socket.
struct Sub {
    conn: usize,
    sid: String,
    pattern: String,
    out: TcpStream,
}

#[derive(Default)]
struct State {
    subs: Vec<Sub>,
    /// `(subject, reply, payload)` of every `PUB`, in arrival order.
    published: Vec<(String, Option<String>, Vec<u8>)>,
}

/// A running server; connect async-nats to [`FakeNats::url`].
pub struct FakeNats {
    pub url: String,
    state: Arc<Mutex<State>>,
}

impl FakeNats {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("nats://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        std::thread::spawn(move || {
            for (conn, stream) in listener.incoming().enumerate() {
                let Ok(stream) = stream else { return };
                let state = shared.clone();
                std::thread::spawn(move || {
                    let _ = serve(conn, stream, &state);
                });
            }
        });
        Self { url, state }
    }

    /// `(subject, reply, payload)` of every `PUB` the server received.
    pub fn published(&self) -> Vec<(String, Option<String>, Vec<u8>)> {
        self.state.lock().unwrap().published.clone()
    }
}

/// `subject` matches `pattern` token by token (`*` one token, `>` the rest).
fn matches(pattern: &str, subject: &str) -> bool {
    let mut s = subject.split('.');
    for p in pattern.split('.') {
        match (p, s.next()) {
            (">", Some(_)) => return true,
            ("*", Some(_)) => {}
            (p, Some(t)) if p == t => {}
            _ => return false,
        }
    }
    s.next().is_none()
}

/// Every write happens under the state lock, so frames written by different
/// connection threads never interleave.
fn send(state: &Mutex<State>, out: &mut TcpStream, frame: &[u8]) -> std::io::Result<()> {
    let _guard = state.lock().unwrap();
    out.write_all(frame)
}

fn serve(conn: usize, stream: TcpStream, state: &Mutex<State>) -> std::io::Result<()> {
    let mut out = stream.try_clone()?;
    send(
        state,
        &mut out,
        b"INFO {\"server_id\":\"fake\",\"version\":\"2.10.0\",\"proto\":1,\"headers\":false,\"max_payload\":1048576}\r\n",
    )?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let args: Vec<&str> = line.split_whitespace().collect();
        match args.as_slice() {
            ["PING"] => send(state, &mut out, b"PONG\r\n")?,
            ["SUB", pattern, .., sid] => state.lock().unwrap().subs.push(Sub {
                conn,
                sid: sid.to_string(),
                pattern: pattern.to_string(),
                out: out.try_clone()?,
            }),
            ["UNSUB", sid, ..] => state
                .lock()
                .unwrap()
                .subs
                .retain(|s| !(s.conn == conn && s.sid == *sid)),
            ["PUB", subject, rest @ ..] => {
                let (reply, len) = match rest {
                    [len] => (None, len),
                    [reply, len] => (Some(reply.to_string()), len),
                    _ => panic!("bad PUB: {line}"),
                };
                let mut payload = vec![0; len.parse::<usize>().unwrap() + 2];
                reader.read_exact(&mut payload)?;
                payload.truncate(payload.len() - 2);
                deliver(state, subject, reply, payload)?;
            }
            _ => {} // CONNECT, PONG
        }
    }
    let mut st = state.lock().unwrap();
    st.subs.retain(|s| s.conn != conn);
    Ok(())
}

/// Record a `PUB` and send it as `MSG` to every matching subscription.
fn deliver(
    state: &Mutex<State>,
    subject: &str,
    reply: Option<String>,
    payload: Vec<u8>,
) -> std::io::Result<()> {
    let mut st = state.lock().unwrap();
    for sub in st.subs.iter_mut().filter(|s| matches(&s.pattern, subject)) {
        let head = match &reply {
            Some(r) => format!("MSG {subject} {} {r} {}\r\n", sub.sid, payload.len()),
            None => format!("MSG {subject} {} {}\r\n", sub.sid, payload.len()),
        };
        let mut frame = head.into_bytes();
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(b"\r\n");
        sub.out.write_all(&frame)?;
    }
    st.published.push((subject.to_string(), reply, payload));
    Ok(())
}
