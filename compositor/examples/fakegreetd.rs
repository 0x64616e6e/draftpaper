//! A stand-in for greetd, for the login screen's headless test (compositor/tests/greeter.sh): greetd's
//! IPC (a native-endian u32 length, then JSON) on the socket given, one account, `drafttest` with the
//! password `pencil`, and the session the greeter starts written to the file given as one JSON line.
//!
//!   fakegreetd SOCKET STARTED-FILE

use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
};

use serde_json::{json, Value};

fn read(s: &mut UnixStream) -> Option<Value> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len).ok()?;
    let mut body = vec![0u8; u32::from_ne_bytes(len) as usize];
    s.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn write(s: &mut UnixStream, v: Value) {
    let body = v.to_string();
    let _ = s.write_all(&(body.len() as u32).to_ne_bytes());
    let _ = s.write_all(body.as_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(socket), Some(started)) = (args.first(), args.get(1)) else {
        eprintln!("fakegreetd SOCKET STARTED-FILE");
        std::process::exit(2);
    };
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).expect("bind");
    for stream in listener.incoming() {
        let Ok(mut s) = stream else { continue };
        let mut user = String::new();
        let mut authed = false;
        while let Some(req) = read(&mut s) {
            let reply = match req["type"].as_str().unwrap_or("") {
                "create_session" => {
                    user = req["username"].as_str().unwrap_or("").to_string();
                    authed = false;
                    json!({ "type": "auth_message", "auth_message_type": "secret", "auth_message": "Password: " })
                }
                "post_auth_message_response" if user == "drafttest" && req["response"] == "pencil" => {
                    authed = true;
                    json!({ "type": "success" })
                }
                "post_auth_message_response" => {
                    user.clear();
                    json!({ "type": "error", "error_type": "auth_error", "description": "Authentication failure" })
                }
                "start_session" if authed => {
                    let line = json!({ "user": user, "cmd": req["cmd"], "env": req["env"] }).to_string();
                    let _ = std::fs::write(started, line + "\n");
                    json!({ "type": "success" })
                }
                "cancel_session" => {
                    user.clear();
                    authed = false;
                    json!({ "type": "success" })
                }
                other => json!({ "type": "error", "error_type": "error", "description": format!("not now: {other}") }),
            };
            write(&mut s, reply);
        }
    }
}
