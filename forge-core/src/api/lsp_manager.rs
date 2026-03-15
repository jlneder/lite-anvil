use mlua::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Spec {
    name: String,
    command: Value,
    filetypes: Vec<String>,
    root_patterns: Vec<String>,
    initialization_options: Option<Value>,
    settings: Option<Value>,
    env: Option<Value>,
}

#[derive(Clone, Default)]
struct DocState {
    version: Option<i64>,
    last_diagnostic_version: Option<i64>,
    pending_semantic_at: Option<f64>,
}

#[derive(Default)]
struct State {
    specs: Vec<Spec>,
    diagnostics: HashMap<String, Value>,
    docs: HashMap<String, DocState>,
}

static STATE: Lazy<Mutex<State>> = Lazy::new(|| Mutex::new(State::default()));

fn builtin_specs() -> Vec<Spec> {
    vec![Spec {
        name: "rust_analyzer".to_string(),
        command: Value::Array(vec![Value::String("rust-analyzer".to_string())]),
        filetypes: vec!["rust".to_string()],
        root_patterns: vec![
            "Cargo.toml".to_string(),
            "rust-project.json".to_string(),
            ".git".to_string(),
        ],
        initialization_options: None,
        settings: None,
        env: None,
    }]
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
                for pair in table.pairs::<String, LuaValue>() {
                    let (key, value) = pair?;
                    out.insert(key, lua_to_json(value)?);
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

fn read_json_file(path: &str) -> LuaResult<Value> {
    let content = fs::read_to_string(path).map_err(|e| LuaError::RuntimeError(e.to_string()))?;
    if content.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&content).map_err(|e| LuaError::RuntimeError(e.to_string()))
}

fn normalize_spec(name: &str, raw: &Value) -> Option<Spec> {
    let raw = raw.as_object()?;
    let command = raw.get("command")?.clone();
    match &command {
        Value::String(_) => {}
        Value::Array(items) if !items.is_empty() && items.iter().all(|item| item.is_string()) => {}
        _ => return None,
    }

    let filetypes = raw
        .get("filetypes")?
        .as_array()?
        .iter()
        .filter_map(|item| item.as_str().map(|s| s.to_lowercase()))
        .collect::<Vec<_>>();
    if filetypes.is_empty() {
        return None;
    }

    let autostart = raw
        .get("autostart")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if !autostart {
        return None;
    }

    let root_patterns = raw
        .get("rootPatterns")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(Spec {
        name: name.to_string(),
        command,
        filetypes,
        root_patterns,
        initialization_options: raw.get("initializationOptions").cloned(),
        settings: raw.get("settings").cloned(),
        env: raw.get("env").cloned(),
    })
}

fn merge_specs(map: &mut HashMap<String, Value>, value: Value) {
    if let Value::Object(entries) = value {
        for (name, raw) in entries {
            map.insert(name, raw);
        }
    }
}

fn dirname(path: &str) -> Option<PathBuf> {
    Path::new(path).parent().map(Path::to_path_buf)
}

fn path_exists(path: &Path) -> bool {
    path.exists()
}

fn is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

fn find_root_for_doc(doc_path: &str, project_root: &str, spec: &Spec) -> String {
    if spec.root_patterns.is_empty() {
        return project_root.to_string();
    }

    let project_root = PathBuf::from(project_root);
    let mut current = dirname(doc_path).unwrap_or_else(|| project_root.clone());
    loop {
        if !is_within(&current, &project_root) {
            break;
        }
        for pattern in &spec.root_patterns {
            if path_exists(&current.join(pattern)) {
                return current.to_string_lossy().into_owned();
            }
        }
        if current == project_root {
            break;
        }
        if !current.pop() {
            break;
        }
    }

    project_root.to_string_lossy().into_owned()
}

fn spec_to_table(lua: &Lua, spec: &Spec, root_dir: Option<String>) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    table.set("name", spec.name.as_str())?;
    table.set("command", json_to_lua(lua, &spec.command)?)?;
    let filetypes = lua.create_table()?;
    for (idx, filetype) in spec.filetypes.iter().enumerate() {
        filetypes.raw_set((idx + 1) as i64, filetype.as_str())?;
    }
    table.set("filetypes", filetypes)?;
    if !spec.root_patterns.is_empty() {
        let patterns = lua.create_table()?;
        for (idx, pattern) in spec.root_patterns.iter().enumerate() {
            patterns.raw_set((idx + 1) as i64, pattern.as_str())?;
        }
        table.set("rootPatterns", patterns)?;
    }
    if let Some(value) = &spec.initialization_options {
        table.set("initializationOptions", json_to_lua(lua, value)?)?;
    }
    if let Some(value) = &spec.settings {
        table.set("settings", json_to_lua(lua, value)?)?;
    }
    if let Some(value) = &spec.env {
        table.set("env", json_to_lua(lua, value)?)?;
    }
    if let Some(root_dir) = root_dir {
        table.set("root_dir", root_dir)?;
    }
    Ok(table)
}

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "reload_config",
        lua.create_function(|_, config_paths: LuaTable| {
            let mut raw = HashMap::new();
            for spec in builtin_specs() {
                let mut map = Map::new();
                map.insert("command".to_string(), spec.command.clone());
                map.insert(
                    "filetypes".to_string(),
                    Value::Array(spec.filetypes.iter().cloned().map(Value::String).collect()),
                );
                map.insert(
                    "rootPatterns".to_string(),
                    Value::Array(
                        spec.root_patterns
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    ),
                );
                raw.insert(spec.name.clone(), Value::Object(map));
            }

            for path in config_paths.sequence_values::<String>() {
                let path = path?;
                if Path::new(&path).exists() {
                    merge_specs(&mut raw, read_json_file(&path)?);
                }
            }

            let mut specs = raw
                .into_iter()
                .filter_map(|(name, value)| normalize_spec(&name, &value))
                .collect::<Vec<_>>();
            specs.sort_by(|a, b| a.name.cmp(&b.name));

            let mut state = STATE.lock();
            state.specs = specs;
            state.diagnostics.clear();
            state.docs.clear();
            Ok(state.specs.len() as i64)
        })?,
    )?;

    module.set(
        "find_spec",
        lua.create_function(
            |lua, (filetype, doc_path, project_root): (String, String, String)| {
                let filetype = filetype.to_lowercase();
                let state = STATE.lock();
                for spec in &state.specs {
                    if spec.filetypes.iter().any(|entry| entry == &filetype) {
                        let root_dir = find_root_for_doc(&doc_path, &project_root, spec);
                        return Ok(Some(spec_to_table(lua, spec, Some(root_dir))?));
                    }
                }
                Ok(None::<LuaTable>)
            },
        )?,
    )?;

    module.set(
        "list_specs",
        lua.create_function(|lua, ()| {
            let state = STATE.lock();
            let out = lua.create_table()?;
            for (idx, spec) in state.specs.iter().enumerate() {
                out.raw_set((idx + 1) as i64, spec_to_table(lua, spec, None)?)?;
            }
            Ok(out)
        })?,
    )?;

    module.set(
        "open_doc",
        lua.create_function(|_, (uri, version): (String, Option<i64>)| {
            let mut state = STATE.lock();
            state.docs.entry(uri).or_default().version = version;
            Ok(true)
        })?,
    )?;

    module.set(
        "update_doc",
        lua.create_function(|_, (uri, version): (String, i64)| {
            let mut state = STATE.lock();
            state.docs.entry(uri).or_default().version = Some(version);
            Ok(true)
        })?,
    )?;

    module.set(
        "close_doc",
        lua.create_function(|_, uri: String| {
            let mut state = STATE.lock();
            state.docs.remove(&uri);
            state.diagnostics.remove(&uri);
            Ok(true)
        })?,
    )?;

    module.set(
        "publish_diagnostics",
        lua.create_function(
            |_, (uri, version, diagnostics): (String, Option<i64>, LuaValue)| {
                let diagnostics = lua_to_json(diagnostics)?;
                let mut state = STATE.lock();
                let doc_state = state.docs.entry(uri.clone()).or_default();
                if let (Some(incoming), Some(current)) = (version, doc_state.version) {
                    if incoming < current {
                        return Ok(false);
                    }
                }
                doc_state.last_diagnostic_version = version.or(doc_state.version);
                state.diagnostics.insert(uri, diagnostics);
                Ok(true)
            },
        )?,
    )?;

    module.set(
        "get_diagnostics",
        lua.create_function(|lua, uri: String| {
            let state = STATE.lock();
            if let Some(value) = state.diagnostics.get(&uri) {
                json_to_lua(lua, value)
            } else {
                Ok(LuaValue::Table(lua.create_table()?))
            }
        })?,
    )?;

    module.set(
        "schedule_semantic",
        lua.create_function(|_, (uri, now, delay): (String, f64, Option<f64>)| {
            let mut state = STATE.lock();
            let doc_state = state.docs.entry(uri).or_default();
            doc_state.pending_semantic_at = Some(now + delay.unwrap_or(0.35));
            Ok(true)
        })?,
    )?;

    module.set(
        "take_due_semantic",
        lua.create_function(|lua, now: f64| {
            let mut state = STATE.lock();
            let out = lua.create_table()?;
            let mut idx = 1i64;
            for (uri, doc_state) in &mut state.docs {
                if let Some(when) = doc_state.pending_semantic_at {
                    if when <= now {
                        doc_state.pending_semantic_at = None;
                        out.raw_set(idx, uri.as_str())?;
                        idx += 1;
                    }
                }
            }
            Ok(out)
        })?,
    )?;

    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::{find_root_for_doc, normalize_spec};
    use serde_json::json;

    #[test]
    fn normalizes_valid_spec() {
        let spec = normalize_spec(
            "rust_analyzer",
            &json!({
                "command": ["rust-analyzer"],
                "filetypes": ["rust"],
                "rootPatterns": ["Cargo.toml"]
            }),
        )
        .expect("spec");
        assert_eq!(spec.filetypes, vec!["rust".to_string()]);
    }

    #[test]
    fn falls_back_to_project_root() {
        let spec = normalize_spec(
            "rust_analyzer",
            &json!({
                "command": ["rust-analyzer"],
                "filetypes": ["rust"],
                "rootPatterns": ["Cargo.toml"]
            }),
        )
        .expect("spec");
        let root = find_root_for_doc("/tmp/project/src/main.rs", "/tmp/project", &spec);
        assert_eq!(root, "/tmp/project");
    }
}
