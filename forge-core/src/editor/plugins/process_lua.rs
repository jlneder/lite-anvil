use mlua::prelude::*;

/// Registers `core.process` -- wraps the native `process` module with Stream helpers.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.process",
        lua.create_function(|lua, ()| {
            let config: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("core.config")?;
            let common: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("core.common")?;

            // The "process" table is our module return value. It also serves as
            // the metatable for wrapper instances and holds the stream class.
            let process = lua.create_table()?;

            // ── stream class ─────────────────────────────────────────────
            let stream_class = lua.create_table()?;
            let stream_index = stream_class.clone();
            stream_class.set("__index", stream_index)?;

            // stream.new(proc, fd)
            let stream_mt = stream_class.clone();
            stream_class.set(
                "new",
                lua.create_function(move |lua, (proc, fd): (LuaTable, i64)| {
                    let s = lua.create_table()?;
                    s.set("fd", fd)?;
                    s.set("process", proc)?;
                    let buf = lua.create_table()?;
                    s.set("buf", buf)?;
                    s.set("len", 0i64)?;
                    s.set_metatable(Some(stream_mt.clone()))?;
                    Ok(s)
                })?,
            )?;

            // stream:read(bytes, options)
            // Non-blocking: reads available data in a single pass. Does not yield.
            // Returns (data) on success/EOF, or (nil, scan_interval) when no data
            // is available yet and the caller should yield and retry.
            stream_class.set(
                "read",
                lua.create_function({
                    let config = config.clone();
                    move |lua, (this, bytes_arg, options): (LuaTable, LuaValue, Option<LuaTable>)| -> LuaResult<LuaMultiValue> {
                        let options = options.unwrap_or(lua.create_table()?);
                        let table_concat: LuaFunction =
                            lua.globals().get::<LuaTable>("table")?.get("concat")?;
                        let table_insert: LuaFunction =
                            lua.globals().get::<LuaTable>("table")?.get("insert")?;
                        let math_max: LuaFunction =
                            lua.globals().get::<LuaTable>("math")?.get("max")?;

                        let bytes_str: Option<String> = match &bytes_arg {
                            LuaValue::String(s) => {
                                let s = s.to_str()?.to_string();
                                Some(s.strip_prefix('*').map(|x| x.to_string()).unwrap_or(s))
                            }
                            _ => None,
                        };

                        let big = 1024i64 * 1024 * 1024 * 1024;
                        let mut target: i64;
                        let mode: &str;

                        if let Some(ref s) = bytes_str {
                            match s.as_str() {
                                "line" | "l" | "L" => {
                                    mode = if s == "L" { "L" } else { "line" };
                                    let buf: LuaTable = this.get("buf")?;
                                    let buf_len = buf.raw_len() as i64;
                                    if buf_len > 0 {
                                        target = 0;
                                        for i in 1..=buf_len {
                                            let v: String = buf.raw_get(i)?;
                                            if let Some(pos) = v.find('\n') {
                                                target += (pos + 1) as i64;
                                                break;
                                            } else if i < buf_len {
                                                target += v.len() as i64;
                                            } else {
                                                target = big;
                                            }
                                        }
                                    } else {
                                        target = big;
                                    }
                                }
                                "all" | "a" => {
                                    mode = "all";
                                    target = big;
                                }
                                other => {
                                    return Err(LuaError::RuntimeError(format!(
                                        "'{}' is an unsupported read option for this stream",
                                        other
                                    )));
                                }
                            }
                        } else if let LuaValue::Integer(n) = bytes_arg {
                            mode = "bytes";
                            target = n;
                        } else if let LuaValue::Number(n) = bytes_arg {
                            mode = "bytes";
                            target = n as i64;
                        } else {
                            return Err(LuaError::RuntimeError(
                                "unsupported read option for this stream".into(),
                            ));
                        }

                        let self_len = this.get::<i64>("len")?;
                        let mut current_len = self_len;

                        let proc_wrapper: LuaTable = this.get("process")?;
                        let inner_proc: LuaAnyUserData = proc_wrapper.get("process")?;
                        let fd: i64 = this.get("fd")?;

                        // Tracks whether we stopped because the stream had no data yet.
                        let mut pending = false;

                        while current_len < target {
                            let read_size: i64 = math_max.call((target - current_len, 0))?;
                            let chunk: LuaValue =
                                inner_proc.call_method("read", (fd, read_size))?;

                            match chunk {
                                LuaValue::Nil => break,
                                LuaValue::String(ref s) if s.as_bytes().is_empty() => {
                                    pending = true;
                                    break;
                                }
                                LuaValue::String(ref s) => {
                                    let chunk_len = s.as_bytes().len() as i64;
                                    let buf: LuaTable = this.get("buf")?;
                                    table_insert.call::<()>((buf.clone(), chunk.clone()))?;
                                    current_len += chunk_len;
                                    this.set("len", current_len)?;

                                    if mode == "line" || mode == "L" {
                                        let chunk_str = s.to_str()?.to_string();
                                        if let Some(pos) = chunk_str.find('\n') {
                                            target = current_len - chunk_len + (pos as i64 + 1);
                                        }
                                    }
                                }
                                _ => break,
                            }
                        }

                        // If pending and we have no buffered data, signal the caller to retry.
                        if pending && current_len == 0 {
                            let fps: f64 = config.get("fps")?;
                            let scan: f64 =
                                options.get::<Option<f64>>("scan")?.unwrap_or(1.0 / fps);
                            return Ok(LuaMultiValue::from_vec(vec![
                                LuaValue::Nil,
                                LuaValue::Number(scan),
                            ]));
                        }

                        let buf: LuaTable = this.get("buf")?;
                        if buf.raw_len() == 0 {
                            return Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]));
                        }

                        let str_val: String = table_concat.call(&buf)?;
                        let actual_target = target.min(current_len);
                        let new_len = (current_len - actual_target).max(0);
                        this.set("len", new_len)?;

                        let new_buf = lua.create_table()?;
                        if new_len > 0 {
                            let rest = &str_val[actual_target as usize..];
                            new_buf.raw_set(1, rest)?;
                        }
                        this.set("buf", new_buf)?;

                        let end_idx = actual_target as usize;
                        let result = if mode == "line" {
                            if end_idx > 0
                                && str_val.as_bytes().get(end_idx - 1) == Some(&b'\n')
                            {
                                &str_val[..end_idx - 1]
                            } else {
                                &str_val[..end_idx]
                            }
                        } else {
                            &str_val[..end_idx.min(str_val.len())]
                        };

                        let data = LuaValue::String(lua.create_string(result)?);
                        // Second return value: scan interval if still pending (caller should
                        // yield and call read again), or nil if read is complete.
                        if pending {
                            let fps: f64 = config.get("fps")?;
                            let scan: f64 =
                                options.get::<Option<f64>>("scan")?.unwrap_or(1.0 / fps);
                            Ok(LuaMultiValue::from_vec(vec![data, LuaValue::Number(scan)]))
                        } else {
                            Ok(LuaMultiValue::from_vec(vec![data]))
                        }
                    }
                })?,
            )?;

            // stream:write(bytes, options)
            // Non-blocking: performs a single write attempt. Does not yield.
            // Returns bytes_written. Caller should retry with remaining data if needed.
            stream_class.set(
                "write",
                lua.create_function(
                    |_, (this, bytes): (LuaTable, String)| -> LuaResult<i64> {
                        let proc_wrapper: LuaTable = this.get("process")?;
                        let inner_proc: LuaAnyUserData = proc_wrapper.get("process")?;

                        let written: LuaValue = inner_proc.call_method("write", bytes.clone())?;
                        match written {
                            LuaValue::Integer(len) => Ok(len),
                            LuaValue::Nil => Ok(0),
                            _ => Ok(0),
                        }
                    },
                )?,
            )?;

            // stream:close()
            stream_class.set(
                "close",
                lua.create_function(|_, this: LuaTable| -> LuaResult<LuaValue> {
                    let proc_wrapper: LuaTable = this.get("process")?;
                    let inner_proc: LuaAnyUserData = proc_wrapper.get("process")?;
                    let fd: i64 = this.get("fd")?;
                    inner_proc.call_method("close_stream", fd)
                })?,
            )?;

            process.set("stream", stream_class)?;

            // ── process:wait(timeout, scan) ──────────────────────────────
            // Non-blocking poll: checks once if process is done. Does not yield.
            // Returns (returncode) if process finished, or (nil, scan_interval)
            // if still running so the caller can yield and retry.
            process.set(
                "wait",
                lua.create_function({
                    let config = config.clone();
                    move |_lua, (this, timeout, scan): (LuaTable, Option<f64>, Option<f64>)| -> LuaResult<LuaMultiValue> {
                        let inner_proc: LuaAnyUserData = this.get("process")?;

                        // If a timeout was given and it's zero (or very small), do a blocking wait.
                        if let Some(t) = timeout {
                            if t <= 0.0 {
                                let result: LuaValue =
                                    inner_proc.call_method("wait", (t * 1000.0) as i32)?;
                                return Ok(LuaMultiValue::from_vec(vec![result]));
                            }
                        }

                        let running: bool = inner_proc.call_method("running", ())?;
                        if !running {
                            let code: LuaValue = inner_proc.call_method("returncode", ())?;
                            return Ok(LuaMultiValue::from_vec(vec![code]));
                        }

                        let fps: f64 = config.get("fps")?;
                        let scan_interval = scan.unwrap_or(1.0 / fps);
                        Ok(LuaMultiValue::from_vec(vec![
                            LuaValue::Nil,
                            LuaValue::Number(scan_interval),
                        ]))
                    }
                })?,
            )?;

            // ── process.__index ──────────────────────────────────────────
            let process_methods = process.clone();
            process.set(
                "__index",
                lua.create_function(move |_, (this, key): (LuaTable, String)| -> LuaResult<LuaValue> {
                    // Check process table methods first
                    let method: LuaValue = process_methods.raw_get(key.as_str())?;
                    if !method.is_nil() {
                        return Ok(method);
                    }
                    // Proxy to inner process userdata
                    let inner: LuaAnyUserData = this.raw_get("process")?;
                    let val: LuaValue = inner.get(key.as_str())?;
                    Ok(val)
                })?,
            )?;

            // ── env helpers (for process.start) ──────────────────────────
            let env_key = lua.create_function(|lua, s: String| -> LuaResult<String> {
                let platform: String = lua.globals().get("PLATFORM")?;
                if platform == "Windows" {
                    Ok(s.to_uppercase())
                } else {
                    Ok(s)
                }
            })?;

            // ── process.start ────────────────────────────────────────────
            // Grab original process.start from the native module
            let native_process: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("process")?;
            let old_start: LuaFunction = native_process.get("start")?;

            // Copy constants from native process module
            let constants = [
                "STREAM_STDIN",
                "STREAM_STDOUT",
                "STREAM_STDERR",
                "WAIT_NONE",
                "WAIT_DEADLINE",
                "WAIT_INFINITE",
                "REDIRECT_DEFAULT",
                "REDIRECT_STDOUT",
                "REDIRECT_STDERR",
                "REDIRECT_PARENT",
                "REDIRECT_DISCARD",
            ];
            for name in constants {
                let val: LuaValue = native_process.get(name)?;
                process.set(name, val)?;
            }
            if let Ok(strerror) = native_process.get::<LuaFunction>("strerror") {
                process.set("strerror", strerror)?;
            }

            let process_mt = process.clone();
            let env_key_fn = env_key;
            let common_ref = common;
            process.set(
                "start",
                lua.create_function(
                    move |lua, (command, options): (LuaValue, Option<LuaTable>)| -> LuaResult<LuaTable> {
                        let platform: String = lua.globals().get("PLATFORM")?;
                        let is_windows = platform == "Windows";

                        // Validate arguments
                        let cmd_type = command.type_name();
                        if cmd_type != "table" && cmd_type != "string" {
                            return Err(LuaError::RuntimeError(format!(
                                "invalid argument #1 to process.start(), expected string or table, got {}",
                                cmd_type
                            )));
                        }
                        // options is already typed as Option<LuaTable>, so if Some it's a table

                        let final_command: LuaValue = if is_windows {
                            if let LuaValue::Table(ref cmd_table) = command {
                                // Escape arguments into a command line string (Windows)
                                let mut arglist: Vec<String> = Vec::new();
                                let len = cmd_table.raw_len();
                                for i in 1..=len {
                                    let v: String = cmd_table.raw_get(i as i64)?;
                                    let mut arg = String::new();
                                    let mut backslash = 0usize;
                                    for c in v.chars() {
                                        if c == '\\' {
                                            backslash += 1;
                                        } else if c == '"' {
                                            arg.push_str(&"\\".repeat(backslash * 2 + 1));
                                            arg.push('"');
                                            backslash = 0;
                                        } else {
                                            arg.push_str(&"\\".repeat(backslash));
                                            arg.push(c);
                                            backslash = 0;
                                        }
                                    }
                                    arg.push_str(&"\\".repeat(backslash));
                                    if v.is_empty() || v.contains(['\t', '\x0B', '\r', '\n', ' ']) {
                                        arglist.push(format!("\"{}\"", arg));
                                    } else {
                                        arglist.push(arg);
                                    }
                                }
                                LuaValue::String(lua.create_string(arglist.join(" "))?)
                            } else {
                                command.clone()
                            }
                        } else {
                            // Unix: ensure command is a table
                            match &command {
                                LuaValue::Table(_) => command.clone(),
                                LuaValue::String(s) => {
                                    let t = lua.create_table()?;
                                    t.raw_set(1, s.to_str()?.to_string())?;
                                    LuaValue::Table(t)
                                }
                                _ => command.clone(),
                            }
                        };

                        // Handle env option
                        let final_options = if let Some(ref opts) = options {
                            let env_val: LuaValue = opts.get("env")?;
                            if let LuaValue::Table(user_env) = env_val {
                                let env_key_fn = env_key_fn.clone();
                                let common_ref = common_ref.clone();
                                let _ = &common_ref;
                                let is_windows_env = is_windows;
                                opts.set(
                                    "env",
                                    lua.create_function(move |lua, system_env: LuaTable| {
                                        let final_env = lua.create_table()?;
                                        // Add system env
                                        for pair in system_env.pairs::<String, String>() {
                                            let (k, v) = pair?;
                                            let ek: String = env_key_fn.call(k.clone())?;
                                            final_env.set(ek, format!("{}={}", k, v))?;
                                        }
                                        // Override with user env
                                        for pair in user_env.pairs::<String, String>() {
                                            let (k, v) = pair?;
                                            let ek: String = env_key_fn.call(k.clone())?;
                                            final_env.set(ek, format!("{}={}", k, v))?;
                                        }
                                        let mut envlist: Vec<String> = Vec::new();
                                        for pair in final_env.pairs::<LuaValue, String>() {
                                            let (_, v) = pair?;
                                            envlist.push(v);
                                        }
                                        if is_windows_env {
                                            envlist.sort_by(|a, b| {
                                                let ak = a.split('=').next().unwrap_or("").to_uppercase();
                                                let bk = b.split('=').next().unwrap_or("").to_uppercase();
                                                ak.cmp(&bk)
                                            });
                                        }
                                        let mut result = envlist.join("\0");
                                        result.push_str("\0\0");
                                        Ok(result)
                                    })?,
                                )?;
                            }
                            options.clone()
                        } else {
                            options.clone()
                        };

                        // Call native process.start
                        let inner_proc: LuaValue = old_start.call((final_command, final_options))?;

                        // Create wrapper table with metatable
                        let wrapper = lua.create_table()?;
                        wrapper.set("process", inner_proc)?;
                        wrapper.set_metatable(Some(process_mt.clone()))?;

                        // Create stream objects
                        let stream_cls: LuaTable = process_mt.get("stream")?;
                        let stream_new: LuaFunction = stream_cls.get("new")?;

                        let stdout_stream: LuaTable = {
                            let stdout_const: i64 = process_mt.get("STREAM_STDOUT")?;
                            stream_new.call((&wrapper, stdout_const))?
                        };
                        let stderr_stream: LuaTable = {
                            let stderr_const: i64 = process_mt.get("STREAM_STDERR")?;
                            stream_new.call((&wrapper, stderr_const))?
                        };
                        let stdin_stream: LuaTable = {
                            let stdin_const: i64 = process_mt.get("STREAM_STDIN")?;
                            stream_new.call((&wrapper, stdin_const))?
                        };

                        wrapper.set("stdout", stdout_stream)?;
                        wrapper.set("stderr", stderr_stream)?;
                        wrapper.set("stdin", stdin_stream)?;

                        Ok(wrapper)
                    },
                )?,
            )?;

            Ok(LuaValue::Table(process))
        })?,
    )
}
