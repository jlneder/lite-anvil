use mlua::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::{BTreeSet, HashMap, HashSet};

static SYMBOLS: Lazy<Mutex<HashMap<u64, Vec<String>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn scan_symbols(
    lines: LuaTable,
    max_symbols: usize,
    excluded: &HashSet<String>,
) -> LuaResult<(Vec<String>, bool)> {
    let mut seen = BTreeSet::new();
    for line in lines.sequence_values::<String>() {
        let line = line?;
        let bytes = line.as_bytes();
        let mut idx = 0usize;
        while idx < bytes.len() {
            let ch = bytes[idx];
            let is_start = ch == b'_' || ch.is_ascii_alphabetic();
            if !is_start {
                idx += 1;
                continue;
            }
            let start = idx;
            idx += 1;
            while idx < bytes.len() {
                let ch = bytes[idx];
                if ch == b'_' || ch.is_ascii_alphanumeric() {
                    idx += 1;
                } else {
                    break;
                }
            }
            let sym = &line[start..idx];
            if !excluded.contains(sym) {
                seen.insert(sym.to_string());
                if seen.len() > max_symbols {
                    return Ok((Vec::new(), true));
                }
            }
        }
    }
    Ok((seen.into_iter().collect(), false))
}

fn set_from_table(table: Option<LuaTable>) -> LuaResult<HashSet<String>> {
    let mut out = HashSet::new();
    if let Some(table) = table {
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            match (key, value) {
                (LuaValue::String(s), LuaValue::Boolean(true)) => {
                    out.insert(s.to_str()?.to_string());
                }
                (LuaValue::String(s), LuaValue::String(_)) => {
                    out.insert(s.to_str()?.to_string());
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "set_doc_symbols",
        lua.create_function(
            |lua, (doc_id, lines, max_symbols, excluded): (u64, LuaTable, usize, Option<LuaTable>)| {
                let excluded = set_from_table(excluded)?;
                let (symbols, exceeded) = scan_symbols(lines, max_symbols, &excluded)?;
                let out = lua.create_table()?;
                out.set("exceeded", exceeded)?;
                out.set("count", symbols.len())?;
                if !exceeded {
                    SYMBOLS.lock().insert(doc_id, symbols);
                } else {
                    SYMBOLS.lock().remove(&doc_id);
                }
                Ok(out)
            },
        )?,
    )?;

    module.set(
        "remove_doc",
        lua.create_function(|_, doc_id: u64| Ok(SYMBOLS.lock().remove(&doc_id).is_some()))?,
    )?;

    module.set(
        "get_doc_symbols",
        lua.create_function(|lua, doc_id: u64| {
            let out = lua.create_table()?;
            if let Some(symbols) = SYMBOLS.lock().get(&doc_id) {
                for (idx, sym) in symbols.iter().enumerate() {
                    out.raw_set((idx + 1) as i64, sym.as_str())?;
                }
            }
            Ok(out)
        })?,
    )?;

    module.set(
        "collect",
        lua.create_function(|lua, doc_ids: LuaTable| {
            let mut merged = BTreeSet::new();
            let guard = SYMBOLS.lock();
            for value in doc_ids.sequence_values::<u64>() {
                let doc_id = value?;
                if let Some(symbols) = guard.get(&doc_id) {
                    for sym in symbols {
                        merged.insert(sym.clone());
                    }
                }
            }
            let out = lua.create_table_with_capacity(merged.len(), 0)?;
            for (idx, sym) in merged.into_iter().enumerate() {
                out.raw_set((idx + 1) as i64, sym)?;
            }
            Ok(out)
        })?,
    )?;

    module.set(
        "clear_all",
        lua.create_function(|_, ()| {
            let mut guard = SYMBOLS.lock();
            guard.clear();
            guard.shrink_to_fit();
            Ok(true)
        })?,
    )?;

    module.set(
        "shrink",
        lua.create_function(|_, ()| {
            SYMBOLS.lock().shrink_to_fit();
            Ok(true)
        })?,
    )?;

    Ok(module)
}
