use std::fs;
use std::io::{BufRead, BufReader};

use mlua::prelude::*;

const CACHE_MAX: usize = 256;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

fn normalize(lua: &Lua, path: Option<String>) -> LuaResult<Option<String>> {
    let Some(path) = path else { return Ok(None) };
    let common = require_table(lua, "core.common")?;
    let normalized: String = common.call_function("normalize_path", path)?;
    Ok(Some(normalized.replace('\\', "/")))
}

fn dirname(path: &str) -> Option<&str> {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let pos = trimmed.rfind(['/', '\\'])?;
    Some(&trimmed[..pos])
}

fn parent_dir(path: &str) -> Option<&str> {
    let parent = dirname(path)?;
    if parent == path {
        return None;
    }
    Some(parent)
}

fn path_exists(lua: &Lua, path: &str) -> LuaResult<bool> {
    let system: LuaTable = lua.globals().get("system")?;
    let info: LuaValue = system.call_function("get_file_info", path.to_string())?;
    Ok(!info.is_nil())
}

fn glob_to_lua_pattern(glob: &str) -> String {
    let bytes = glob.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'*' {
            out.push_str(".*");
            i += 2;
        } else if bytes[i] == b'*' {
            out.push_str("[^/]*");
            i += 1;
        } else if bytes[i] == b'?' {
            out.push_str("[^/]");
            i += 1;
        } else {
            let ch = bytes[i] as char;
            // Escape Lua pattern metacharacters
            if "%+-^$().[]?".contains(ch) {
                out.push('%');
            }
            out.push(ch);
            i += 1;
        }
    }
    out
}

fn parse_rule(lua: &Lua, line: &str, base_dir: &str) -> LuaResult<Option<LuaTable>> {
    if line.is_empty() || line.trim_start().starts_with('#') {
        return Ok(None);
    }

    let mut s = line;
    let negated = s.starts_with('!');
    if negated {
        s = &s[1..];
    }

    let anchored = s.starts_with('/');
    if anchored {
        s = &s[1..];
    }

    let dir_only = s.ends_with('/');
    if dir_only {
        s = &s[..s.len() - 1];
    }

    if s.is_empty() {
        return Ok(None);
    }

    let has_slash = s.contains('/');
    let prefix = if anchored {
        "^"
    } else if has_slash {
        "^(.-/)?"
    } else {
        "^([^/]+/)*"
    };

    let mut pattern = String::from(prefix);
    pattern.push_str(&glob_to_lua_pattern(s));
    if dir_only {
        pattern.push_str("(/.*)?$");
    } else {
        pattern.push('$');
    }

    let normalized_base = normalize(lua, Some(base_dir.to_string()))?;

    let rule = lua.create_table()?;
    rule.set("base_dir", normalized_base)?;
    rule.set("negated", negated)?;
    rule.set("dir_only", dir_only)?;
    rule.set("anchored", anchored)?;
    rule.set("has_slash", has_slash)?;
    rule.set("pattern", pattern)?;
    rule.set("raw", s.to_string())?;
    Ok(Some(rule))
}

fn load_rules(lua: &Lua, dir: &str, cache: &LuaTable) -> LuaResult<LuaTable> {
    let Some(dir_normalized) = normalize(lua, Some(dir.to_string()))? else {
        return lua.create_table();
    };

    let ignore_path = format!("{dir_normalized}/.gitignore");

    let system: LuaTable = lua.globals().get("system")?;
    let info: LuaValue = system.call_function("get_file_info", ignore_path.clone())?;

    let modified: LuaValue = if let LuaValue::Table(ref info_t) = info {
        info_t.get("modified")?
    } else {
        LuaValue::Boolean(false)
    };

    let cached: Option<LuaTable> = cache.get(&*ignore_path)?;
    if let Some(ref cached_t) = cached {
        let cached_mod: LuaValue = cached_t.get("modified")?;
        if cached_mod == modified {
            return cached_t.get("rules");
        }
    }

    let rules = lua.create_table()?;
    if let LuaValue::Table(ref info_t) = info {
        let file_type: Option<String> = info_t.get("type")?;
        if file_type.as_deref() == Some("file") {
            if let Ok(file) = fs::File::open(&ignore_path) {
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if let Some(rule) = parse_rule(lua, &line, &dir_normalized)? {
                        rules.push(rule)?;
                    }
                }
            }
        }
    }

    let entry = lua.create_table()?;
    entry.set("modified", modified)?;
    entry.set("rules", rules.clone())?;
    cache.set(ignore_path.clone(), entry)?;

    if cached.is_none() {
        let mut count = 0usize;
        for pair in cache.pairs::<LuaValue, LuaValue>() {
            let _ = pair?;
            count += 1;
            if count > CACHE_MAX {
                // Evict entire cache, re-add current entry
                for evict_pair in cache.clone().pairs::<LuaValue, LuaValue>() {
                    let (k, _) = evict_pair?;
                    cache.set(k, LuaNil)?;
                }
                let new_entry = lua.create_table()?;
                new_entry.set("modified", LuaNil)?;
                new_entry.set("rules", rules.clone())?;
                cache.set(ignore_path, new_entry)?;
                break;
            }
        }
    }

    Ok(rules)
}

fn collect_dirs(lua: &Lua, root: &str, path: &str) -> LuaResult<Vec<String>> {
    let mut dirs = Vec::new();
    let Some(mut current) = normalize(lua, Some(path.to_string()))? else {
        return Ok(dirs);
    };

    let system: LuaTable = lua.globals().get("system")?;
    let info: LuaValue = system.call_function("get_file_info", current.clone())?;
    if let LuaValue::Table(ref info_t) = info {
        let file_type: Option<String> = info_t.get("type")?;
        if file_type.as_deref() == Some("file") {
            if let Some(d) = dirname(&current) {
                current = d.to_string();
            }
        }
    }

    let Some(root_normalized) = normalize(lua, Some(root.to_string()))? else {
        return Ok(dirs);
    };

    let common = require_table(lua, "core.common")?;
    loop {
        let belongs: bool = common.call_function(
            "path_belongs_to",
            (current.clone(), root_normalized.clone()),
        )?;
        if !belongs {
            break;
        }
        dirs.insert(0, current.clone());
        if current == root_normalized {
            break;
        }
        match parent_dir(&current) {
            Some(p) => current = p.to_string(),
            None => break,
        }
    }

    Ok(dirs)
}

fn find_root_impl(lua: &Lua, start_path: Option<String>) -> LuaResult<LuaValue> {
    let Some(mut current) = normalize(lua, start_path)? else {
        return Ok(LuaNil);
    };

    let system: LuaTable = lua.globals().get("system")?;
    let info: LuaValue = system.call_function("get_file_info", current.clone())?;
    if let LuaValue::Table(ref info_t) = info {
        let file_type: Option<String> = info_t.get("type")?;
        if file_type.as_deref() == Some("file") {
            if let Some(d) = dirname(&current) {
                current = d.to_string();
            }
        }
    }

    loop {
        let git_path = format!("{current}/.git");
        if path_exists(lua, &git_path)? {
            return Ok(LuaValue::String(lua.create_string(&current)?));
        }
        match parent_dir(&current) {
            Some(p) => current = p.to_string(),
            None => break,
        }
    }

    Ok(LuaNil)
}

fn match_impl(
    lua: &Lua,
    root: Option<String>,
    path: Option<String>,
    info: Option<LuaTable>,
    cache: &LuaTable,
) -> LuaResult<bool> {
    let config = require_table(lua, "core.config")?;
    let gi_config: LuaValue = config.get("gitignore")?;
    if let LuaValue::Table(ref gi_t) = gi_config {
        let enabled: LuaValue = gi_t.get("enabled")?;
        if enabled == LuaValue::Boolean(false) {
            return Ok(false);
        }
    }

    let Some(root_n) = normalize(lua, root)? else {
        return Ok(false);
    };
    let Some(path_n) = normalize(lua, path)? else {
        return Ok(false);
    };

    let common = require_table(lua, "core.common")?;
    let belongs: bool =
        common.call_function("path_belongs_to", (path_n.clone(), root_n.clone()))?;
    if !belongs {
        return Ok(false);
    }

    let mut ignored = false;
    let dirs = collect_dirs(lua, &root_n, &path_n)?;

    let string_match: LuaFunction = lua
        .load("local t,p = ... return t:match(p) ~= nil")
        .eval()?;

    for dir in &dirs {
        let rel: String = common.call_function("relative_path", (dir.clone(), path_n.clone()))?;
        let rel = rel.replace('\\', "/");

        let rules = load_rules(lua, dir, cache)?;
        let len = rules.len()?;
        for i in 1..=len {
            let rule: LuaTable = rules.get(i)?;
            let has_slash: bool = rule.get("has_slash")?;
            let anchored: bool = rule.get("anchored")?;
            let pattern: String = rule.get("pattern")?;
            let dir_only: bool = rule.get("dir_only")?;
            let negated: bool = rule.get("negated")?;

            let target: String = if !has_slash && !anchored {
                common.call_function("basename", path_n.clone())?
            } else {
                rel.clone()
            };

            let matched: bool = string_match.call((target, pattern))?;

            if matched {
                let is_dir = if let Some(ref info_t) = info {
                    let t: Option<String> = info_t.get("type")?;
                    t.as_deref() == Some("dir")
                } else {
                    false
                };
                if !dir_only || is_dir {
                    ignored = !negated;
                }
            }
        }
    }

    if let LuaValue::Table(ref gi_t) = gi_config {
        let additional: LuaValue = gi_t.get("additional_patterns")?;
        if let LuaValue::Table(ref patterns) = additional {
            let rel: String = common.call_function("relative_path", (root_n, path_n.clone()))?;
            let basename: String = common.call_function("basename", path_n)?;
            let len = patterns.len()?;
            for i in 1..=len {
                let pat: String = patterns.get(i)?;
                let match_rel: bool =
                    common.call_function("match_pattern", (rel.clone(), pat.clone()))?;
                let match_base: bool =
                    common.call_function("match_pattern", (basename.clone(), pat))?;
                if match_rel || match_base {
                    ignored = true;
                    break;
                }
            }
        }
    }

    Ok(ignored)
}

/// Registers the `core.gitignore` preload with gitignore pattern matching implemented in Rust.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.gitignore",
        lua.create_function(|lua, ()| {
            let module = lua.create_table()?;
            let cache = lua.create_table()?;
            module.set("cache", cache.clone())?;

            module.set(
                "find_root",
                lua.create_function(|lua, start_path: Option<String>| {
                    find_root_impl(lua, start_path)
                })?,
            )?;

            let cache_key = lua.create_registry_value(cache)?;
            module.set(
                "match",
                lua.create_function(
                    move |lua,
                          (root, path, info): (
                        Option<String>,
                        Option<String>,
                        Option<LuaTable>,
                    )| {
                        let cache: LuaTable = lua.registry_value(&cache_key)?;
                        match_impl(lua, root, path, info, &cache)
                    },
                )?,
            )?;

            Ok(LuaValue::Table(module))
        })?,
    )
}
