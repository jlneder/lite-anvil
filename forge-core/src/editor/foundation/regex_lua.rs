use mlua::prelude::*;

/// Registers `core.regex` — adds `find`, `find_offsets`, `match` helpers onto the `regex` table.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.regex",
        lua.create_function(|lua, ()| {
            let regex: LuaTable = lua
                .globals()
                .get::<LuaTable>("package")?
                .get::<LuaTable>("loaded")?
                .get("regex")?;

            // regex.__index = function(table, key) return regex[key] end
            regex.set(
                "__index",
                lua.create_function({
                    let regex_key = lua.create_registry_value(regex.clone())?;
                    move |lua, (_table, key): (LuaTable, LuaValue)| {
                        let regex: LuaTable = lua.registry_value(&regex_key)?;
                        regex.raw_get::<LuaValue>(key)
                    }
                })?,
            )?;

            // regex.find_offsets(pattern, str, offset, options) -> start, end, ...
            regex.set(
                "find_offsets",
                lua.create_function(
                    |lua,
                     (pattern, str, offset, options): (
                        LuaValue,
                        LuaString,
                        Option<i64>,
                        Option<i64>,
                    )| {
                        let regex: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("regex")?;
                        let compiled: LuaValue = if matches!(pattern, LuaValue::Table(_)) {
                            pattern.clone()
                        } else {
                            let compile_fn: LuaFunction = regex.get("compile")?;
                            compile_fn.call(pattern)?
                        };
                        let cmatch_fn: LuaFunction = regex.get("cmatch")?;
                        let res: LuaMultiValue = cmatch_fn.call((
                            compiled,
                            str,
                            offset.unwrap_or(1),
                            options.unwrap_or(0),
                        ))?;
                        let vals: Vec<LuaValue> = res.into_vec();
                        if vals.is_empty() {
                            return Ok(LuaMultiValue::new());
                        }
                        // Reduce every end delimiter (even indices, 0-based) by 1
                        let mut out = Vec::with_capacity(vals.len());
                        for (i, v) in vals.iter().enumerate() {
                            if (i + 1) % 2 == 0 {
                                // end position: subtract 1
                                if let LuaValue::Integer(n) = v {
                                    out.push(LuaValue::Integer(n - 1));
                                } else if let LuaValue::Number(n) = v {
                                    out.push(LuaValue::Number(n - 1.0));
                                } else {
                                    out.push(v.clone());
                                }
                            } else {
                                out.push(v.clone());
                            }
                        }
                        Ok(LuaMultiValue::from_vec(out))
                    },
                )?,
            )?;

            // regex.find(pattern, str, offset, options) -> start, end, captured_strings...
            regex.set(
                "find",
                lua.create_function(
                    |lua,
                     (pattern, str, offset, options): (
                        LuaValue,
                        LuaString,
                        Option<i64>,
                        Option<i64>,
                    )| {
                        let regex: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("regex")?;
                        let find_offsets_fn: LuaFunction = regex.get("find_offsets")?;
                        let res: LuaMultiValue =
                            find_offsets_fn.call((pattern, str.clone(), offset, options))?;
                        let vals: Vec<LuaValue> = res.into_vec();
                        if vals.is_empty() {
                            return Ok(LuaMultiValue::new());
                        }
                        let mut out = Vec::new();
                        // First two values: start and end offsets
                        out.push(vals[0].clone());
                        out.push(vals[1].clone());
                        // Remaining pairs: captured groups
                        let mut i = 2;
                        let str_bytes = str.as_bytes();
                        while i + 1 < vals.len() {
                            let start = match &vals[i] {
                                LuaValue::Integer(n) => *n,
                                LuaValue::Number(n) => *n as i64,
                                _ => {
                                    i += 2;
                                    continue;
                                }
                            };
                            let end = match &vals[i + 1] {
                                LuaValue::Integer(n) => *n,
                                LuaValue::Number(n) => *n as i64,
                                _ => {
                                    i += 2;
                                    continue;
                                }
                            };
                            if start > end {
                                // Empty group: return offset like string.find
                                out.push(LuaValue::Integer(start));
                            } else {
                                // Extract substring (1-based to 0-based)
                                let s_idx = (start - 1).max(0) as usize;
                                let e_idx = end.max(0) as usize;
                                let e_idx = e_idx.min(str_bytes.len());
                                let s_idx = s_idx.min(e_idx);
                                let sub = &str_bytes[s_idx..e_idx];
                                out.push(LuaValue::String(lua.create_string(sub)?));
                            }
                            i += 2;
                        }
                        Ok(LuaMultiValue::from_vec(out))
                    },
                )?,
            )?;

            // regex.match(pattern, str, offset, options) -> captured_strings or full match
            regex.set(
                "match",
                lua.create_function(
                    |lua,
                     (pattern, str, offset, options): (
                        LuaValue,
                        LuaString,
                        Option<i64>,
                        Option<i64>,
                    )| {
                        let regex: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("regex")?;
                        let find_fn: LuaFunction = regex.get("find")?;
                        let res: LuaMultiValue =
                            find_fn.call((pattern, str.clone(), offset, options))?;
                        let vals: Vec<LuaValue> = res.into_vec();
                        if vals.is_empty() {
                            return Ok(LuaMultiValue::new());
                        }
                        // If captures exist (more than 2 values), return only captures
                        if vals.len() > 2 {
                            return Ok(LuaMultiValue::from_vec(vals[2..].to_vec()));
                        }
                        // Otherwise return the full match as a substring
                        let start = match &vals[0] {
                            LuaValue::Integer(n) => *n,
                            LuaValue::Number(n) => *n as i64,
                            _ => return Ok(LuaMultiValue::new()),
                        };
                        let end = match &vals[1] {
                            LuaValue::Integer(n) => *n,
                            LuaValue::Number(n) => *n as i64,
                            _ => return Ok(LuaMultiValue::new()),
                        };
                        let str_bytes = str.as_bytes();
                        let s_idx = (start - 1).max(0) as usize;
                        let e_idx = end.max(0) as usize;
                        let e_idx = e_idx.min(str_bytes.len());
                        let s_idx = s_idx.min(e_idx);
                        let sub = &str_bytes[s_idx..e_idx];
                        Ok(LuaMultiValue::from_vec(vec![LuaValue::String(
                            lua.create_string(sub)?,
                        )]))
                    },
                )?,
            )?;

            Ok(LuaValue::Nil)
        })?,
    )
}
