use crossbeam_channel::{Receiver, Sender, unbounded};
use mlua::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::thread;

struct TransportHandle {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    stderr: Receiver<String>,
    exit_code: Arc<AtomicU64>,
}

static TRANSPORTS: Lazy<Mutex<HashMap<u64, TransportHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(1));

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

fn lua_to_json(value: LuaValue) -> LuaResult<Value> {
    Ok(match value {
        LuaValue::Nil => Value::Null,
        LuaValue::Boolean(v) => Value::Bool(v),
        LuaValue::Integer(v) => Value::Number(Number::from(v)),
        LuaValue::Number(v) => Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        LuaValue::String(v) => Value::String(v.to_str()?.to_string()),
        LuaValue::Table(table) => {
            let mut max_idx = 0i64;
            let mut count = 0i64;
            let mut array_like = true;
            for pair in table.pairs::<LuaValue, LuaValue>() {
                let (key, _) = pair?;
                match key {
                    LuaValue::Integer(idx) if idx >= 1 => {
                        max_idx = max_idx.max(idx);
                        count += 1;
                    }
                    _ => {
                        array_like = false;
                        break;
                    }
                }
            }
            if array_like && count == max_idx {
                let mut out = Vec::new();
                for idx in 1..=max_idx {
                    out.push(lua_to_json(table.raw_get(idx)?)?);
                }
                Value::Array(out)
            } else {
                let mut out = Map::new();
                for pair in table.pairs::<LuaValue, LuaValue>() {
                    let (key, value) = pair?;
                    if let LuaValue::String(key) = key {
                        out.insert(key.to_str()?.to_string(), lua_to_json(value)?);
                    }
                }
                Value::Object(out)
            }
        }
        _ => Value::Null,
    })
}

fn json_to_lua(lua: &Lua, value: &Value) -> LuaResult<LuaValue> {
    Ok(match value {
        Value::Null => LuaValue::Nil,
        Value::Bool(v) => LuaValue::Boolean(*v),
        Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                LuaValue::Integer(i)
            } else {
                LuaValue::Number(v.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(v) => LuaValue::String(lua.create_string(v)?),
        Value::Array(items) => {
            let table = lua.create_table()?;
            for (idx, item) in items.iter().enumerate() {
                table.raw_set((idx + 1) as i64, json_to_lua(lua, item)?)?;
            }
            LuaValue::Table(table)
        }
        Value::Object(map) => {
            let table = lua.create_table()?;
            for (key, item) in map {
                table.set(key.as_str(), json_to_lua(lua, item)?)?;
            }
            LuaValue::Table(table)
        }
    })
}

fn parse_messages(buffer: &mut Vec<u8>, sender: &Sender<Value>) {
    loop {
        let Some(header_end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") else {
            break;
        };
        let header = String::from_utf8_lossy(&buffer[..header_end]);
        let Some(length) = header.lines().find_map(|line| {
            line.split_once(':').and_then(|(k, v)| {
                if k.eq_ignore_ascii_case("Content-Length") {
                    v.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
        }) else {
            buffer.clear();
            break;
        };
        let body_start = header_end + 4;
        let body_end = body_start + length;
        if buffer.len() < body_end {
            break;
        }
        match serde_json::from_slice::<Value>(&buffer[body_start..body_end]) {
            Ok(value) => {
                let _ = sender.send(value);
            }
            Err(e) => {
                log::warn!("LSP: malformed JSON in response, skipping message: {e}");
            }
        }
        buffer.drain(..body_end);
    }
}

fn start_stdout_thread(mut stdout: ChildStdout, sender: Sender<Value>) {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    parse_messages(&mut buf, &sender);
                }
            }
        }
    });
}

fn start_stderr_thread(mut stderr: ChildStderr, sender: Sender<String>) {
    thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match stderr.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&chunk[..n]).to_string();
                    let _ = sender.send(text);
                }
            }
        }
    });
}

fn command_from_lua(value: LuaValue) -> LuaResult<Vec<String>> {
    match value {
        LuaValue::String(s) => Ok(vec![s.to_str()?.to_string()]),
        LuaValue::Table(t) => {
            let mut out = Vec::new();
            for entry in t.sequence_values::<String>() {
                out.push(entry?);
            }
            Ok(out)
        }
        _ => Err(LuaError::RuntimeError("invalid LSP command".to_string())),
    }
}

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "spawn",
        lua.create_function(
            |_, (command, cwd, env): (LuaValue, String, Option<LuaTable>)| {
                let command = command_from_lua(command)?;
                let mut cmd = Command::new(
                    command
                        .first()
                        .ok_or_else(|| LuaError::RuntimeError("empty LSP command".to_string()))?,
                );
                for arg in command.iter().skip(1) {
                    cmd.arg(arg);
                }
                cmd.current_dir(cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                if let Some(env) = env {
                    for pair in env.pairs::<String, String>() {
                        let (key, value) = pair?;
                        cmd.env(key, value);
                    }
                }
                let mut child = cmd
                    .spawn()
                    .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
                let stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| LuaError::RuntimeError("missing LSP stdin".to_string()))?;
                let stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| LuaError::RuntimeError("missing LSP stdout".to_string()))?;
                let stderr = child
                    .stderr
                    .take()
                    .ok_or_else(|| LuaError::RuntimeError("missing LSP stderr".to_string()))?;

                let (msg_tx, msg_rx) = unbounded();
                let (err_tx, err_rx) = unbounded();
                start_stdout_thread(stdout, msg_tx);
                start_stderr_thread(stderr, err_tx);

                let id = next_id();
                TRANSPORTS.lock().insert(
                    id,
                    TransportHandle {
                        child,
                        stdin,
                        messages: msg_rx,
                        stderr: err_rx,
                        exit_code: Arc::new(AtomicU64::new(u64::MAX)),
                    },
                );
                Ok(id)
            },
        )?,
    )?;

    module.set(
        "send",
        lua.create_function(|_, (id, message): (u64, LuaValue)| {
            let payload = lua_to_json(message).and_then(|value| {
                serde_json::to_vec(&value).map_err(|e| LuaError::RuntimeError(e.to_string()))
            })?;
            let framed = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
            let mut transports = TRANSPORTS.lock();
            let handle = transports
                .get_mut(&id)
                .ok_or_else(|| LuaError::RuntimeError("unknown LSP transport".to_string()))?;
            handle
                .stdin
                .write_all(&framed)
                .and_then(|_| handle.stdin.write_all(&payload))
                .and_then(|_| handle.stdin.flush())
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
            Ok(true)
        })?,
    )?;

    module.set(
        "poll",
        lua.create_function(|lua, (id, max_messages): (u64, Option<usize>)| {
            let mut transports = TRANSPORTS.lock();
            let handle = transports
                .get_mut(&id)
                .ok_or_else(|| LuaError::RuntimeError("unknown LSP transport".to_string()))?;
            let messages = lua.create_table()?;
            let stderr = lua.create_table()?;
            let max_messages = max_messages.unwrap_or(64);
            let mut idx = 1i64;
            for _ in 0..max_messages {
                match handle.messages.try_recv() {
                    Ok(message) => {
                        messages.raw_set(idx, json_to_lua(lua, &message)?)?;
                        idx += 1;
                    }
                    Err(_) => break,
                }
            }
            idx = 1;
            while let Ok(line) = handle.stderr.try_recv() {
                stderr.raw_set(idx, line)?;
                idx += 1;
            }
            let running = match handle.child.try_wait() {
                Ok(Some(status)) => {
                    handle
                        .exit_code
                        .store(status.code().unwrap_or(-1) as u64, Ordering::Relaxed);
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            };
            let out = lua.create_table()?;
            out.set("messages", messages)?;
            out.set("stderr", stderr)?;
            out.set("running", running)?;
            let code = handle.exit_code.load(Ordering::Relaxed);
            if code != u64::MAX {
                out.set("exit_code", code as i64)?;
            }
            Ok(out)
        })?,
    )?;

    module.set(
        "terminate",
        lua.create_function(|_, id: u64| {
            if let Some(handle) = TRANSPORTS.lock().get_mut(&id) {
                if let Err(e) = handle.child.kill() {
                    log::warn!("failed to kill LSP transport {id}: {e}");
                }
                Ok(true)
            } else {
                Ok(false)
            }
        })?,
    )?;

    module.set(
        "remove",
        lua.create_function(|_, id: u64| Ok(TRANSPORTS.lock().remove(&id).is_some()))?,
    )?;

    module.set(
        "clear_all",
        lua.create_function(|_, ()| {
            let mut transports = TRANSPORTS.lock();
            for handle in transports.values_mut() {
                if let Err(e) = handle.child.kill() {
                    log::warn!("failed to kill LSP transport: {e}");
                }
            }
            transports.clear();
            transports.shrink_to_fit();
            Ok(true)
        })?,
    )?;

    Ok(module)
}
