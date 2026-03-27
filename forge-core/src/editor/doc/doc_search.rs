use mlua::prelude::*;

/// Registers `core.doc.search` -- document text search with regex, case, and wrap support.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.doc.search",
        lua.create_function(|lua, ()| {
            let module = lua.create_table()?;

            module.set(
                "find",
                lua.create_function(
                    |lua,
                     (doc, line, col, text, opt): (
                        LuaTable,
                        i64,
                        i64,
                        String,
                        Option<LuaTable>,
                    )| {
                        let sanitize: LuaFunction = doc.get("sanitize_position")?;
                        let (line, col): (i64, i64) = sanitize.call((&doc, line, col))?;

                        let no_case = opt
                            .as_ref()
                            .and_then(|t| t.get::<Option<bool>>("no_case").ok().flatten())
                            .unwrap_or(false);
                        let use_regex = opt
                            .as_ref()
                            .and_then(|t| t.get::<Option<bool>>("regex").ok().flatten())
                            .unwrap_or(false);
                        let reverse = opt
                            .as_ref()
                            .and_then(|t| t.get::<Option<bool>>("reverse").ok().flatten())
                            .unwrap_or(false);
                        let wrap = opt
                            .as_ref()
                            .and_then(|t| t.get::<Option<bool>>("wrap").ok().flatten())
                            .unwrap_or(false);
                        let use_pattern = opt
                            .as_ref()
                            .and_then(|t| t.get::<Option<bool>>("pattern").ok().flatten())
                            .unwrap_or(false);

                        let text = if no_case && !use_regex {
                            text.to_lowercase()
                        } else {
                            text
                        };

                        // Fast path: use doc_native.find for forward non-pattern searches
                        if !use_pattern && !reverse {
                            let doc_native: LuaTable = lua
                                .globals()
                                .get::<LuaFunction>("require")?
                                .call("doc_native")?;
                            let find_fn: LuaFunction = doc_native.get("find")?;
                            let lines: LuaTable = doc.get("lines")?;
                            let find_opts = lua.create_table()?;
                            find_opts.set("no_case", no_case)?;
                            find_opts.set("regex", use_regex)?;
                            let args = vec![
                                LuaValue::Table(lines.clone()),
                                LuaValue::Integer(line),
                                LuaValue::Integer(col),
                                LuaValue::String(lua.create_string(&text)?),
                                LuaValue::Table(find_opts.clone()),
                            ];
                            let result: LuaMultiValue =
                                find_fn.call(LuaMultiValue::from_vec(args))?;
                            if result.len() >= 4 {
                                return Ok(result);
                            }
                            if wrap {
                                let args = vec![
                                    LuaValue::Table(lines),
                                    LuaValue::Integer(1),
                                    LuaValue::Integer(1),
                                    LuaValue::String(lua.create_string(&text)?),
                                    LuaValue::Table(find_opts),
                                ];
                                let result: LuaMultiValue =
                                    find_fn.call(LuaMultiValue::from_vec(args))?;
                                return Ok(result);
                            }
                            return Ok(LuaMultiValue::new());
                        }

                        // Lua pattern / regex / reverse path
                        let lines: LuaTable = doc.get("lines")?;
                        let num_lines = lines.raw_len() as i64;

                        let plain = !use_pattern;

                        // Compile regex if needed
                        let compiled_re: Option<LuaValue> = if use_regex {
                            let regex_mod: LuaTable = lua.globals().get("regex")?;
                            let compile: LuaFunction = regex_mod.get("compile")?;
                            let flags = if no_case { "i" } else { "" };
                            Some(compile.call((text.clone(), flags.to_string()))?)
                        } else {
                            None
                        };

                        let string_find: LuaFunction =
                            lua.globals().get::<LuaTable>("string")?.get("find")?;

                        let search_line =
                            |line_idx: i64,
                             start_col: i64,
                             reverse: bool|
                             -> LuaResult<Option<(i64, i64)>> {
                                let mut line_text: String = lines.raw_get(line_idx)?;
                                if no_case && !use_regex {
                                    line_text = line_text.to_lowercase();
                                }
                                if reverse {
                                    rfind(
                                        lua,
                                        &string_find,
                                        compiled_re.as_ref(),
                                        &line_text,
                                        &text,
                                        start_col - 1,
                                        plain,
                                    )
                                } else if let Some(re) = compiled_re.as_ref() {
                                    let cmatch: LuaFunction = re
                                        .as_table()
                                        .ok_or_else(|| {
                                            LuaError::RuntimeError("regex not a table".into())
                                        })?
                                        .get("cmatch")?;
                                    let result: LuaMultiValue =
                                        cmatch.call((re.clone(), line_text.clone(), start_col))?;
                                    if result.len() >= 2 {
                                        let s = result[0].as_integer().ok_or_else(|| {
                                            LuaError::RuntimeError("bad regex match".into())
                                        })?;
                                        let e = result[1].as_integer().ok_or_else(|| {
                                            LuaError::RuntimeError("bad regex match".into())
                                        })? - 1;
                                        Ok(Some((s, e)))
                                    } else {
                                        Ok(None)
                                    }
                                } else {
                                    let result: LuaMultiValue = string_find.call((
                                        line_text.clone(),
                                        text.clone(),
                                        start_col,
                                        plain,
                                    ))?;
                                    if result.len() >= 2 {
                                        let s = result[0].as_integer().ok_or_else(|| {
                                            LuaError::RuntimeError("bad find result".into())
                                        })?;
                                        let e = result[1].as_integer().ok_or_else(|| {
                                            LuaError::RuntimeError("bad find result".into())
                                        })?;
                                        Ok(Some((s, e)))
                                    } else {
                                        Ok(None)
                                    }
                                }
                            };

                        let try_search = |start: i64,
                                          finish: i64,
                                          step: i64,
                                          init_col: i64|
                         -> LuaResult<LuaMultiValue> {
                            let mut col = init_col;
                            let mut l = start;
                            loop {
                                if let Some((s, e)) = search_line(l, col, reverse)? {
                                    let line_text: String = lines.raw_get(l)?;
                                    let mut line2 = l;
                                    let mut end_col = e + 1;
                                    if e as usize >= line_text.len() {
                                        line2 = l + 1;
                                        end_col = 1;
                                    }
                                    if line2 <= num_lines {
                                        return Ok(LuaMultiValue::from_vec(vec![
                                            LuaValue::Integer(l),
                                            LuaValue::Integer(s),
                                            LuaValue::Integer(line2),
                                            LuaValue::Integer(end_col),
                                        ]));
                                    }
                                }
                                col = if reverse { -1 } else { 1 };
                                if (step > 0 && l >= finish) || (step < 0 && l <= finish) {
                                    break;
                                }
                                l += step;
                            }
                            Ok(LuaMultiValue::new())
                        };

                        let (start, finish, step) = if reverse {
                            (line, 1i64, -1i64)
                        } else {
                            (line, num_lines, 1i64)
                        };

                        let result = try_search(start, finish, step, col)?;
                        if !result.is_empty() {
                            return Ok(result);
                        }

                        if wrap {
                            let (ws, wf, wstep, wcol) = if reverse {
                                let last_line: String = lines.raw_get(num_lines)?;
                                (num_lines, 1i64, -1i64, last_line.len() as i64)
                            } else {
                                (1i64, num_lines, 1i64, 1i64)
                            };
                            return try_search(ws, wf, wstep, wcol);
                        }

                        Ok(LuaMultiValue::new())
                    },
                )?,
            )?;

            Ok(LuaValue::Table(module))
        })?,
    )
}

/// Reverse-find: search backwards from `index` in `text`.
fn rfind(
    lua: &Lua,
    string_find: &LuaFunction,
    compiled_re: Option<&LuaValue>,
    text: &str,
    pattern: &str,
    index: i64,
    plain: bool,
) -> LuaResult<Option<(i64, i64)>> {
    let mut last: Option<(i64, i64)> = None;
    let effective_index = if index < 0 {
        text.len() as i64 + index + 1
    } else {
        index
    };

    let find_at = |pos: i64| -> LuaResult<Option<(i64, i64)>> {
        if let Some(re) = compiled_re {
            let cmatch: LuaFunction = re
                .as_table()
                .ok_or_else(|| LuaError::RuntimeError("regex not a table".into()))?
                .get("cmatch")?;
            let result: LuaMultiValue = cmatch.call((re.clone(), text.to_string(), pos))?;
            if result.len() >= 2 {
                let s = result[0]
                    .as_integer()
                    .ok_or_else(|| LuaError::RuntimeError("bad regex match".into()))?;
                let e = result[1]
                    .as_integer()
                    .ok_or_else(|| LuaError::RuntimeError("bad regex match".into()))?
                    - 1;
                Ok(Some((s, e)))
            } else {
                Ok(None)
            }
        } else {
            let result: LuaMultiValue =
                string_find.call((text.to_string(), pattern.to_string(), pos, plain))?;
            if result.len() >= 2 {
                let s = result[0]
                    .as_integer()
                    .ok_or_else(|| LuaError::RuntimeError("bad find".into()))?;
                let e = result[1]
                    .as_integer()
                    .ok_or_else(|| LuaError::RuntimeError("bad find".into()))?;
                Ok(Some((s, e)))
            } else {
                Ok(None)
            }
        }
    };

    let first = find_at(1)?;
    if let Some((mut s, mut e)) = first {
        while e <= effective_index {
            last = Some((s, e));
            match find_at(s + 1)? {
                Some((ns, ne)) => {
                    s = ns;
                    e = ne;
                }
                None => break,
            }
        }
    }
    let _ = lua;
    Ok(last)
}
