use mlua::prelude::*;

/// Registers `core.common` as a native Rust preload, replacing `data/core/common.lua`.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.common",
        lua.create_function(|lua, ()| {
            let common = lua.create_table()?;

            // common.is_utf8_cont(s, offset?)
            common.set(
                "is_utf8_cont",
                lua.create_function(|_, (s, offset): (LuaString, Option<usize>)| {
                    let bytes = s.as_bytes();
                    let idx = offset.unwrap_or(1);
                    if idx == 0 || idx > bytes.len() {
                        return Ok(false);
                    }
                    let byte = bytes[idx - 1];
                    Ok((0x80..0xc0).contains(&byte))
                })?,
            )?;

            // common.utf8_chars(text) — returns an iterator yielding one UTF-8 char per call
            common.set(
                "utf8_chars",
                lua.create_function(|lua, text: LuaString| {
                    let gmatch: LuaFunction =
                        lua.globals().get::<LuaTable>("string")?.get("gmatch")?;
                    gmatch.call::<LuaValue>((
                        text,
                        lua.create_string(b"[\0-\x7f\xc2-\xf4][\x80-\xbf]*")?,
                    ))
                })?,
            )?;

            // common.clamp(n, lo, hi)
            common.set(
                "clamp",
                lua.create_function(|_, (n, lo, hi): (f64, f64, f64)| Ok(n.min(hi).max(lo)))?,
            )?;

            // common.merge(a, b)
            common.set(
                "merge",
                lua.create_function(|lua, (a, b): (LuaValue, Option<LuaValue>)| {
                    let t = lua.create_table()?;
                    if let LuaValue::Table(a) = a {
                        for pair in a.pairs::<LuaValue, LuaValue>() {
                            let (k, v) = pair?;
                            t.set(k, v)?;
                        }
                    }
                    if let Some(LuaValue::Table(b)) = b {
                        for pair in b.pairs::<LuaValue, LuaValue>() {
                            let (k, v) = pair?;
                            t.set(k, v)?;
                        }
                    }
                    Ok(t)
                })?,
            )?;

            // common.round(n)
            common.set(
                "round",
                lua.create_function(|_, n: f64| {
                    Ok(if n >= 0.0 {
                        (n + 0.5).floor()
                    } else {
                        (n - 0.5).ceil()
                    })
                })?,
            )?;

            // common.find_index(tbl, prop)
            common.set(
                "find_index",
                lua.create_function(|_, (tbl, prop): (LuaTable, LuaValue)| {
                    for pair in tbl.pairs::<i64, LuaTable>() {
                        let (i, o) = pair?;
                        let val: LuaValue = o.get(prop.clone())?;
                        if val != LuaNil && val != LuaValue::Boolean(false) {
                            return Ok(LuaValue::Integer(i));
                        }
                    }
                    Ok(LuaNil)
                })?,
            )?;

            // common.lerp(a, b, t) — works on numbers and tables
            let lerp_fn = lua.create_function(lerp)?;
            common.set("lerp", lerp_fn)?;

            // common.distance(x1, y1, x2, y2)
            common.set(
                "distance",
                lua.create_function(|_, (x1, y1, x2, y2): (f64, f64, f64, f64)| {
                    Ok(((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt())
                })?,
            )?;

            // common.color(str) -> r, g, b, a
            common.set("color", lua.create_function(color)?)?;

            // common.splice(t, at, remove, insert?)
            common.set(
                "splice",
                lua.create_function(
                    |lua, (t, at, remove, insert): (LuaTable, i64, i64, Option<LuaTable>)| {
                        if remove < 0 {
                            return Err(LuaError::runtime(
                                "bad argument #3 to 'splice' (non-negative value expected)",
                            ));
                        }
                        let insert_tbl = insert.unwrap_or(lua.create_table()?);
                        let table_move: LuaFunction =
                            lua.globals().get::<LuaTable>("table")?.get("move")?;
                        let len: i64 = insert_tbl.raw_len() as i64;
                        let t_len = t.raw_len() as i64;
                        if remove != len {
                            table_move.call::<()>((
                                t.clone(),
                                at + remove,
                                t_len + remove,
                                at + len,
                            ))?;
                        }
                        table_move.call::<()>((insert_tbl, 1, len, at, t))?;
                        Ok(())
                    },
                )?,
            )?;

            // common.fuzzy_match(haystack, needle, files?)
            // When haystack is a table, filters + sorts; when a string, delegates to system.fuzzy_match
            common.set(
                "fuzzy_match",
                lua.create_function(
                    |lua, (haystack, needle, files): (LuaValue, LuaString, Option<bool>)| {
                        let needle_str = needle.to_str()?.to_string();
                        let files = files.unwrap_or(false);
                        match haystack {
                            LuaValue::Table(items) => {
                                fuzzy_match_items(lua, &items, &needle_str, files)
                                    .map(LuaValue::Table)
                            }
                            _ => {
                                let system: LuaTable = lua.globals().get("system")?;
                                let fm: LuaFunction = system.get("fuzzy_match")?;
                                fm.call::<LuaValue>((haystack, needle, files))
                            }
                        }
                    },
                )?,
            )?;

            // common.fuzzy_match_with_recents(haystack, recents, needle)
            common.set(
                "fuzzy_match_with_recents",
                lua.create_function(
                    |lua, (haystack, recents, needle): (LuaTable, LuaTable, LuaString)| {
                        let needle_str = needle.to_str()?.to_string();
                        if needle_str.is_empty() {
                            let result = lua.create_table()?;
                            let recents_len = recents.raw_len();
                            // add recents[2..n], then recents[1]
                            for i in 2..=recents_len {
                                let v: LuaValue = recents.get(i)?;
                                result.push(v)?;
                            }
                            if recents_len >= 1 {
                                let first: LuaValue = recents.get(1)?;
                                result.push(first)?;
                            }
                            // common.fuzzy_match(haystack, "", true) -> others
                            let others = fuzzy_match_items(lua, &haystack, "", true)?;
                            for i in 1..=others.raw_len() {
                                let v: LuaValue = others.get(i)?;
                                result.push(v)?;
                            }
                            Ok(result)
                        } else {
                            fuzzy_match_items(lua, &haystack, &needle_str, true)
                        }
                    },
                )?,
            )?;

            // common.path_suggest(text, root?)
            common.set("path_suggest", lua.create_function(path_suggest)?)?;

            // common.dir_path_suggest(text, root)
            common.set(
                "dir_path_suggest",
                lua.create_function(|lua, (text, root): (LuaString, LuaString)| {
                    let pathsep = get_pathsep(lua)?;
                    let text_str = text.to_str()?.to_string();
                    let pat = format!("^(.-)([^{}]*)$", regex_escape_pathsep(&pathsep));
                    let string_match: LuaFunction =
                        lua.globals().get::<LuaTable>("string")?.get("match")?;
                    let (path, _name): (LuaString, LuaValue) =
                        string_match.call((text.clone(), pat))?;
                    let path_str = path.to_str()?.to_string();
                    let dir = if path_str.is_empty() {
                        root.to_str()?.to_string()
                    } else {
                        path_str.clone()
                    };
                    let system: LuaTable = lua.globals().get("system")?;
                    let list_dir: LuaFunction = system.get("list_dir")?;
                    let result: LuaMultiValue = list_dir.call(dir)?;
                    let mut vals = result.into_vec();
                    if vals.is_empty() || vals[0] == LuaNil {
                        return lua.create_table();
                    }
                    let files = match vals.remove(0) {
                        LuaValue::Table(t) => t,
                        _ => return lua.create_table(),
                    };
                    let types = if !vals.is_empty() {
                        match vals.remove(0) {
                            LuaValue::Table(t) => t,
                            _ => lua.create_table()?,
                        }
                    } else {
                        lua.create_table()?
                    };
                    let res = lua.create_table()?;
                    let text_lower = text_str.to_lowercase();
                    for i in 1..=files.raw_len() {
                        let name: LuaString = files.get(i)?;
                        let ftype: String = types.get::<String>(i).unwrap_or_default();
                        let file = format!("{}{}", path_str, name.to_str()?);
                        if ftype == "dir" && file.to_lowercase().starts_with(&text_lower) {
                            res.push(file)?;
                        }
                    }
                    Ok(res)
                })?,
            )?;

            // common.dir_list_suggest(text, dir_list)
            common.set(
                "dir_list_suggest",
                lua.create_function(|lua, (text, dir_list): (LuaString, LuaTable)| {
                    let text_str = text.to_str()?;
                    let text_lower = text_str.to_lowercase();
                    let res = lua.create_table()?;
                    for val in dir_list.sequence_values::<LuaString>() {
                        let dp = val?;
                        if dp.to_str()?.to_lowercase().starts_with(&text_lower) {
                            res.push(dp)?;
                        }
                    }
                    Ok(res)
                })?,
            )?;

            // common.match_pattern(text, pattern, ...)
            common.set(
                "match_pattern",
                lua.create_function(|lua, args: LuaMultiValue| {
                    let args_vec = args.into_vec();
                    if args_vec.len() < 2 {
                        return Err(LuaError::runtime(
                            "bad argument to 'match_pattern' (expected at least 2 arguments)",
                        ));
                    }
                    let text = args_vec[0].clone();
                    let pattern = args_vec[1].clone();
                    let rest: Vec<LuaValue> = args_vec[2..].to_vec();
                    let string_find: LuaFunction =
                        lua.globals().get::<LuaTable>("string")?.get("find")?;

                    match pattern {
                        LuaValue::String(_) => {
                            let mut call_args = vec![text, pattern];
                            call_args.extend(rest);
                            let result: LuaMultiValue =
                                string_find.call(LuaMultiValue::from_vec(call_args))?;
                            let rv = result.into_vec();
                            if rv.is_empty() || rv[0] == LuaNil {
                                Ok(LuaMultiValue::from_vec(vec![LuaValue::Boolean(false)]))
                            } else {
                                Ok(LuaMultiValue::from_vec(rv))
                            }
                        }
                        LuaValue::Table(patterns) => {
                            // Recursive: try each pattern
                            let common_ref: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("core.common")?;
                            let match_pattern_fn: LuaFunction = common_ref.get("match_pattern")?;
                            for val in patterns.sequence_values::<LuaValue>() {
                                let p = val?;
                                let mut call_args = vec![text.clone(), p];
                                call_args.extend(rest.clone());
                                let result: LuaMultiValue =
                                    match_pattern_fn.call(LuaMultiValue::from_vec(call_args))?;
                                let rv = result.into_vec();
                                if !rv.is_empty() && rv[0] != LuaValue::Boolean(false) {
                                    return Ok(LuaMultiValue::from_vec(rv));
                                }
                            }
                            Ok(LuaMultiValue::from_vec(vec![LuaValue::Boolean(false)]))
                        }
                        _ => Ok(LuaMultiValue::from_vec(vec![LuaValue::Boolean(false)])),
                    }
                })?,
            )?;

            // common.draw_text(font, color, text, align, x, y, w, h)
            common.set(
                "draw_text",
                lua.create_function(
                    |lua,
                     (font, color, text, align, x, y, w, h): (
                        LuaValue,
                        LuaValue,
                        LuaString,
                        Option<LuaString>,
                        f64,
                        f64,
                        f64,
                        f64,
                    )| {
                        let common_ref: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("core.common")?;
                        let round_fn: LuaFunction = common_ref.get("round")?;

                        let get_width: LuaFunction = font
                            .as_table()
                            .ok_or_else(|| LuaError::runtime("font expected"))?
                            .get("get_width")?;
                        let get_height: LuaFunction = font
                            .as_table()
                            .ok_or_else(|| LuaError::runtime("font expected"))?
                            .get("get_height")?;
                        let tw: f64 = get_width.call((font.clone(), text.clone()))?;
                        let th: f64 = get_height.call(font.clone())?;

                        let align_str = match align {
                            Some(s) => s.to_str()?.to_string(),
                            None => String::new(),
                        };
                        let draw_x = match align_str.as_str() {
                            "center" => x + (w - tw) / 2.0,
                            "right" => x + (w - tw),
                            _ => x,
                        };
                        let draw_y: f64 = round_fn.call(y + (h - th) / 2.0)?;

                        let renderer: LuaTable = lua.globals().get("renderer")?;
                        let draw_text_fn: LuaFunction = renderer.get("draw_text")?;
                        let x_advance: f64 =
                            draw_text_fn.call((font, text, draw_x, draw_y, color))?;

                        Ok((x_advance, draw_y + th))
                    },
                )?,
            )?;

            // common.bench(name, fn, ...)
            common.set(
                "bench",
                lua.create_function(
                    |lua, (name, func, args): (LuaString, LuaFunction, LuaMultiValue)| {
                        let system: LuaTable = lua.globals().get("system")?;
                        let get_time: LuaFunction = system.get("get_time")?;
                        let start: f64 = get_time.call(())?;
                        let res: LuaMultiValue = func.call(args)?;
                        let end_time: f64 = get_time.call(())?;
                        let t = end_time - start;
                        let ms = t * 1000.0;
                        let per = (t / (1.0 / 60.0)) * 100.0;
                        let print_fn: LuaFunction = lua.globals().get("print")?;
                        print_fn.call::<()>(format!(
                            "*** {:<16} : {:>8.3}ms {:>6.2}%",
                            name.to_str()?,
                            ms,
                            per
                        ))?;
                        Ok(res)
                    },
                )?,
            )?;

            // common.serialize(val, opts?)
            common.set("serialize", lua.create_function(serialize)?)?;

            // common.basename(path)
            common.set(
                "basename",
                lua.create_function(|lua, path: LuaString| {
                    let pathsep = get_pathsep(lua)?;
                    let s = path.to_str()?.to_string();
                    let result = s
                        .rsplit(|c: char| pathsep.contains(c))
                        .find(|part| !part.is_empty())
                        .unwrap_or(&s);
                    Ok(result.to_string())
                })?,
            )?;

            // common.dirname(path)
            common.set(
                "dirname",
                lua.create_function(|lua, path: LuaString| {
                    let pathsep = get_pathsep(lua)?;
                    let s = path.to_str()?.to_string();
                    // Find last separator that is followed by a non-separator segment
                    if let Some(pos) = s.rfind(|c: char| pathsep.contains(c)) {
                        let after = &s[pos + 1..];
                        if after.is_empty() || after.chars().all(|c| pathsep.contains(c)) {
                            return Ok(LuaNil);
                        }
                        return Ok(LuaValue::String(lua.create_string(&s[..pos])?));
                    }
                    Ok(LuaNil)
                })?,
            )?;

            // common.home_encode(text)
            common.set(
                "home_encode",
                lua.create_function(|lua, text: LuaString| {
                    let home: Option<String> = lua.globals().get("HOME").ok();
                    let s = text.to_str()?;
                    if let Some(ref h) = home {
                        if s.starts_with(h.as_str()) {
                            return Ok(format!("~{}", &s[h.len()..]));
                        }
                    }
                    Ok(s.to_string())
                })?,
            )?;

            // common.home_encode_list(paths)
            common.set(
                "home_encode_list",
                lua.create_function(|lua, paths: LuaTable| {
                    let common_ref: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core.common")?;
                    let encode_fn: LuaFunction = common_ref.get("home_encode")?;
                    let t = lua.create_table()?;
                    for i in 1..=paths.raw_len() {
                        let val: LuaValue = paths.get(i)?;
                        let encoded: LuaValue = encode_fn.call(val)?;
                        t.set(i, encoded)?;
                    }
                    Ok(t)
                })?,
            )?;

            // common.home_expand(text)
            common.set(
                "home_expand",
                lua.create_function(|lua, text: LuaString| {
                    let home: Option<String> = lua.globals().get("HOME").ok();
                    let s = text.to_str()?;
                    if let Some(ref h) = home {
                        if let Some(rest) = s.strip_prefix('~') {
                            return Ok(format!("{h}{rest}"));
                        }
                    }
                    Ok(s.to_string())
                })?,
            )?;

            // common.normalize_volume(filename)
            common.set(
                "normalize_volume",
                lua.create_function(|lua, filename: LuaValue| {
                    let s = match &filename {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        LuaValue::Nil => return Ok(LuaNil),
                        _ => return Ok(filename),
                    };
                    let pathsep = get_pathsep(lua)?;
                    if pathsep == "\\" {
                        let bytes = s.as_bytes();
                        if bytes.len() >= 3
                            && bytes[0].is_ascii_alphabetic()
                            && bytes[1] == b':'
                            && bytes[2] == b'\\'
                        {
                            let drive = (bytes[0] as char).to_uppercase().to_string();
                            let rem = s[3..].trim_end_matches('\\');
                            return Ok(LuaValue::String(
                                lua.create_string(format!("{drive}:\\{rem}"))?,
                            ));
                        }
                    }
                    Ok(LuaValue::String(lua.create_string(&s)?))
                })?,
            )?;

            // common.normalize_path(filename)
            common.set("normalize_path", lua.create_function(normalize_path)?)?;

            // common.is_absolute_path(path)
            common.set(
                "is_absolute_path",
                lua.create_function(|lua, path: LuaString| {
                    let pathsep = get_pathsep(lua)?;
                    let s = path.to_str()?;
                    if s.starts_with(pathsep.as_str()) {
                        return Ok(true);
                    }
                    // Check for Windows drive letter pattern
                    let bytes = s.as_bytes();
                    if bytes.len() >= 2
                        && bytes[0].is_ascii_alphabetic()
                        && bytes[1] == b':'
                        && (bytes.len() < 3 || bytes[2] == b'\\')
                    {
                        return Ok(true);
                    }
                    Ok(false)
                })?,
            )?;

            // common.path_belongs_to(filename, path)
            common.set(
                "path_belongs_to",
                lua.create_function(|lua, (filename, path): (LuaString, LuaString)| {
                    let pathsep = get_pathsep(lua)?;
                    let f = filename.to_str()?;
                    let p = path.to_str()?;
                    let prefix = format!("{p}{pathsep}");
                    Ok(f.starts_with(&prefix))
                })?,
            )?;

            // common.relative_path(ref_dir, dir)
            common.set("relative_path", lua.create_function(relative_path)?)?;

            // common.mkdirp(path)
            common.set(
                "mkdirp",
                lua.create_function(|lua, path: LuaString| {
                    mkdirp(lua, path.to_str()?.to_string())
                })?,
            )?;

            // common.rm(path, recursively)
            common.set(
                "rm",
                lua.create_function(|lua, (path, recursively): (LuaString, Option<bool>)| {
                    rm(
                        lua,
                        path.to_str()?.to_string(),
                        recursively.unwrap_or(false),
                    )
                })?,
            )?;

            Ok(LuaValue::Table(common))
        })?,
    )
}

fn get_pathsep(lua: &Lua) -> LuaResult<String> {
    lua.globals()
        .get::<String>("PATHSEP")
        .or_else(|_| Ok("/".to_string()))
}

fn regex_escape_pathsep(pathsep: &str) -> String {
    if pathsep == "\\" {
        "\\\\/".to_string()
    } else {
        pathsep.to_string()
    }
}

/// Implements common.lerp for both numbers and tables.
fn lerp(lua: &Lua, (a, b, t): (LuaValue, LuaValue, f64)) -> LuaResult<LuaValue> {
    match (&a, &b) {
        (LuaValue::Number(a_n), LuaValue::Number(b_n)) => {
            Ok(LuaValue::Number(a_n + (b_n - a_n) * t))
        }
        (LuaValue::Integer(a_i), LuaValue::Number(b_n)) => {
            let a_n = *a_i as f64;
            Ok(LuaValue::Number(a_n + (b_n - a_n) * t))
        }
        (LuaValue::Number(a_n), LuaValue::Integer(b_i)) => {
            let b_n = *b_i as f64;
            Ok(LuaValue::Number(a_n + (b_n - a_n) * t))
        }
        (LuaValue::Integer(a_i), LuaValue::Integer(b_i)) => {
            let a_n = *a_i as f64;
            let b_n = *b_i as f64;
            Ok(LuaValue::Number(a_n + (b_n - a_n) * t))
        }
        (LuaValue::Table(a_tbl), LuaValue::Table(b_tbl)) => {
            let res = lua.create_table()?;
            for pair in b_tbl.pairs::<LuaValue, LuaValue>() {
                let (k, bv) = pair?;
                let av: LuaValue = a_tbl.get(k.clone())?;
                let rv = lerp(lua, (av, bv, t))?;
                res.set(k, rv)?;
            }
            Ok(LuaValue::Table(res))
        }
        _ => {
            let a_f: f64 = match &a {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(n) => *n as f64,
                _ => {
                    return Err(LuaError::runtime(
                        "bad argument to 'lerp' (number or table expected)",
                    ));
                }
            };
            let b_f: f64 = match &b {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(n) => *n as f64,
                _ => {
                    return Err(LuaError::runtime(
                        "bad argument to 'lerp' (number or table expected)",
                    ));
                }
            };
            Ok(LuaValue::Number(a_f + (b_f - a_f) * t))
        }
    }
}

/// Parses CSS-style color strings: #rrggbb, #rrggbbaa, rgb(), rgba().
fn color(_lua: &Lua, s: LuaString) -> LuaResult<LuaMultiValue> {
    let input = s.to_str()?;
    let input = input.trim();

    // Try #rrggbb or #rrggbbaa
    if let Some(hex) = input.strip_prefix('#') {
        if (hex.len() == 6 || hex.len() == 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let r =
                u8::from_str_radix(&hex[0..2], 16).map_err(|e| LuaError::runtime(e.to_string()))?;
            let g =
                u8::from_str_radix(&hex[2..4], 16).map_err(|e| LuaError::runtime(e.to_string()))?;
            let b =
                u8::from_str_radix(&hex[4..6], 16).map_err(|e| LuaError::runtime(e.to_string()))?;
            let a = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16).map_err(|e| LuaError::runtime(e.to_string()))?
            } else {
                0xff
            };
            return Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Number(r as f64),
                LuaValue::Number(g as f64),
                LuaValue::Number(b as f64),
                LuaValue::Number(a as f64),
            ]));
        }
    }

    // Try rgb() or rgba()
    if input.starts_with("rgb") {
        let nums: Vec<f64> = input
            .chars()
            .fold((Vec::new(), String::new()), |(mut nums, mut cur), c| {
                if c.is_ascii_digit() || c == '.' {
                    cur.push(c);
                } else if !cur.is_empty() {
                    if let Ok(n) = cur.parse::<f64>() {
                        nums.push(n);
                    }
                    cur.clear();
                }
                (nums, cur)
            })
            .0;

        let r = nums.first().copied().unwrap_or(0.0);
        let g = nums.get(1).copied().unwrap_or(0.0);
        let b = nums.get(2).copied().unwrap_or(0.0);
        let a = nums.get(3).copied().unwrap_or(1.0) * 255.0;

        return Ok(LuaMultiValue::from_vec(vec![
            LuaValue::Number(r),
            LuaValue::Number(g),
            LuaValue::Number(b),
            LuaValue::Number(a),
        ]));
    }

    Err(LuaError::runtime(format!("bad color string '{input}'")))
}

/// Filters and sorts items by fuzzy match score.
fn fuzzy_match_items(
    lua: &Lua,
    items: &LuaTable,
    needle: &str,
    files: bool,
) -> LuaResult<LuaTable> {
    let system: LuaTable = lua.globals().get("system")?;
    let fm: LuaFunction = system.get("fuzzy_match")?;

    let platform: String = lua
        .globals()
        .get("PLATFORM")
        .unwrap_or_else(|_| "Linux".to_string());
    let pathsep = get_pathsep(lua)?;

    let adjusted_needle = if platform == "Windows" && files {
        needle.replace('/', &pathsep)
    } else {
        needle.to_string()
    };

    struct ScoredItem {
        text: LuaValue,
        sort_text: String,
        score: i64,
    }

    let mut scored = Vec::new();
    let tostring: LuaFunction = lua.globals().get("tostring")?;

    for val in items.sequence_values::<LuaValue>() {
        let item = val?;
        let sort_text: String = tostring.call(item.clone())?;
        let score_val: LuaValue = fm.call((sort_text.clone(), adjusted_needle.clone(), files))?;
        if let Some(score) = match score_val {
            LuaValue::Integer(n) => Some(n),
            LuaValue::Number(n) => Some(n as i64),
            _ => None,
        } {
            scored.push(ScoredItem {
                text: item,
                sort_text,
                score,
            });
        }
    }

    scored.sort_by(|a, b| {
        if a.score == b.score {
            a.sort_text.cmp(&b.sort_text)
        } else {
            b.score.cmp(&a.score)
        }
    });

    let res = lua.create_table()?;
    for item in scored {
        res.push(item.text)?;
    }
    Ok(res)
}

/// Implements common.path_suggest(text, root?).
fn path_suggest(lua: &Lua, (text, root): (LuaString, Option<LuaString>)) -> LuaResult<LuaTable> {
    let pathsep = get_pathsep(lua)?;
    let platform: String = lua
        .globals()
        .get("PLATFORM")
        .unwrap_or_else(|_| "Linux".to_string());
    let text_str = text.to_str()?.to_string();

    let root_str = match &root {
        Some(r) => {
            let mut s = r.to_str()?.to_string();
            if !s.ends_with(pathsep.as_str()) {
                s.push_str(&pathsep);
            }
            Some(s)
        }
        None => None,
    };

    // Extract the directory portion of text
    let sep_chars: String = if platform == "Windows" {
        "\\/".to_string()
    } else {
        pathsep.clone()
    };

    let path_part = {
        let last_sep = text_str.rfind(|c: char| sep_chars.contains(c));
        match last_sep {
            Some(pos) => text_str[..=pos].to_string(),
            None => String::new(),
        }
    };

    let common_tbl: LuaTable = lua
        .globals()
        .get::<LuaTable>("package")?
        .get::<LuaTable>("loaded")?
        .get("core.common")?;
    let is_abs_fn: LuaFunction = common_tbl.get("is_absolute_path")?;
    let is_absolute: bool = is_abs_fn.call(text_str.clone())?;

    let mut clean_dotslash = false;
    let lookup_path = if is_absolute {
        path_part.clone()
    } else if path_part.is_empty() {
        match &root_str {
            Some(r) => r.clone(),
            None => {
                clean_dotslash = true;
                ".".to_string()
            }
        }
    } else {
        match &root_str {
            Some(r) => format!("{r}{path_part}"),
            None => path_part.clone(),
        }
    };

    // Ensure path ends with separator
    let lookup_path = if pathsep == "\\" {
        if !lookup_path.ends_with('\\') && !lookup_path.ends_with('/') {
            format!("{lookup_path}{pathsep}")
        } else {
            lookup_path
        }
    } else if !lookup_path.ends_with(pathsep.as_str()) {
        format!("{lookup_path}{pathsep}")
    } else {
        lookup_path
    };

    let system: LuaTable = lua.globals().get("system")?;
    let list_dir: LuaFunction = system.get("list_dir")?;
    let result: LuaMultiValue = list_dir.call(lookup_path.clone())?;
    let mut vals = result.into_vec();
    if vals.is_empty() || vals[0] == LuaNil {
        return lua.create_table();
    }
    let files = match vals.remove(0) {
        LuaValue::Table(t) => t,
        _ => return lua.create_table(),
    };
    let types = if !vals.is_empty() {
        match vals.remove(0) {
            LuaValue::Table(t) => t,
            _ => lua.create_table()?,
        }
    } else {
        lua.create_table()?
    };

    let res = lua.create_table()?;
    let text_lower = text_str.to_lowercase();

    for i in 1..=files.raw_len() {
        let name: String = files.get(i)?;
        let ftype: String = types.get::<String>(i).unwrap_or_default();
        let mut file = format!("{lookup_path}{name}");
        if ftype == "dir" {
            file.push_str(&pathsep);
        }
        // Remove root prefix or dot-slash prefix
        if let Some(ref r) = root_str {
            if file.starts_with(r.as_str()) {
                file = file[r.len()..].to_string();
            }
        } else if clean_dotslash {
            let dot_sep = format!(".{pathsep}");
            if file.starts_with(&dot_sep) {
                file = file[dot_sep.len()..].to_string();
            }
        }
        if file.to_lowercase().starts_with(&text_lower) {
            res.push(file)?;
        }
    }
    Ok(res)
}

/// Serializes a Lua value into loadable Lua source code.
fn serialize(lua: &Lua, (val, opts): (LuaValue, Option<LuaTable>)) -> LuaResult<String> {
    let opts = opts.unwrap_or(lua.create_table()?);
    let pretty: bool = opts.get("pretty").unwrap_or(false);
    let indent_str: String = opts
        .get::<String>("indent_str")
        .unwrap_or_else(|_| "  ".to_string());
    let escape: bool = opts.get("escape").unwrap_or(false);
    let sort: bool = opts.get("sort").unwrap_or(false);
    let initial_indent: usize = opts.get::<usize>("initial_indent").unwrap_or(0);
    let limit_opt: LuaValue = opts.get("limit")?;
    let limit: usize = match limit_opt {
        LuaValue::Integer(n) => n as usize + initial_indent,
        LuaValue::Number(n) if n.is_finite() => n as usize + initial_indent,
        _ => usize::MAX,
    };

    let indent = if pretty {
        indent_str.repeat(initial_indent)
    } else {
        String::new()
    };

    let mut out = indent;
    let opts = SerOpts {
        pretty,
        indent_str: &indent_str,
        escape,
        sort,
        limit,
    };
    serialize_value(lua, &val, &opts, initial_indent, &mut out)?;
    Ok(out)
}

struct SerOpts<'a> {
    pretty: bool,
    indent_str: &'a str,
    escape: bool,
    sort: bool,
    limit: usize,
}

fn serialize_value(
    lua: &Lua,
    val: &LuaValue,
    opts: &SerOpts<'_>,
    level: usize,
    out: &mut String,
) -> LuaResult<()> {
    let SerOpts {
        pretty,
        indent_str,
        escape,
        sort,
        limit,
    } = *opts;
    let space = if pretty { " " } else { "" };
    let indent = if pretty {
        indent_str.repeat(level)
    } else {
        String::new()
    };
    let newline = if pretty { "\n" } else { "" };

    match val {
        LuaValue::String(s) => {
            let str_val = s.to_str()?.to_string();
            let formatted = format_lua_string(&str_val, escape);
            out.push_str(&formatted);
        }
        LuaValue::Table(tbl) => {
            if level >= limit {
                let tostring: LuaFunction = lua.globals().get("tostring")?;
                let s: String = tostring.call(val.clone())?;
                out.push_str(&s);
                return Ok(());
            }
            let next_indent = if pretty {
                format!("{indent}{indent_str}")
            } else {
                String::new()
            };

            let mut entries = Vec::new();
            for pair in tbl.pairs::<LuaValue, LuaValue>() {
                let (k, v) = pair?;
                let mut entry = next_indent.clone();
                entry.push('[');
                serialize_value(lua, &k, opts, level + 1, &mut entry)?;
                entry.push(']');
                entry.push_str(space);
                entry.push('=');
                entry.push_str(space);
                serialize_value(lua, &v, opts, level + 1, &mut entry)?;
                entries.push(entry);
            }
            if entries.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            if sort {
                entries.sort();
            }
            out.push('{');
            out.push_str(newline);
            out.push_str(&entries.join(&format!(",{newline}")));
            out.push_str(newline);
            out.push_str(&indent);
            out.push('}');
        }
        LuaValue::Number(n) => {
            if n.is_infinite() {
                if *n > 0.0 {
                    out.push_str("1/0");
                } else {
                    out.push_str("-1/0");
                }
            } else if n.is_nan() {
                // Distinguish -nan from nan by sign bit
                if n.is_sign_negative() {
                    out.push_str("-(0/0)");
                } else {
                    out.push_str("0/0");
                }
            } else {
                // Locale-independent formatting
                let s = format!("{n}");
                let s = s.replace(',', ".");
                out.push_str(&s);
            }
        }
        LuaValue::Integer(n) => {
            out.push_str(&n.to_string());
        }
        LuaValue::Boolean(b) => {
            out.push_str(if *b { "true" } else { "false" });
        }
        LuaValue::Nil => {
            out.push_str("nil");
        }
        _ => {
            let tostring: LuaFunction = lua.globals().get("tostring")?;
            let s: String = tostring.call(val.clone())?;
            out.push_str(&s);
        }
    }
    Ok(())
}

/// Formats a string as a Lua quoted literal, matching `string.format("%q", s)` behavior.
fn format_lua_string(s: &str, escape: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for byte in s.bytes() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => {
                if escape {
                    out.push_str("\\n");
                } else {
                    out.push_str("\\\n");
                }
            }
            b'\r' => {
                if escape {
                    out.push_str("\\r");
                } else {
                    out.push_str("\\13");
                }
            }
            b'\0' => out.push_str("\\0"),
            b'\x07' => {
                if escape {
                    out.push_str("\\a");
                } else {
                    out.push_str("\\7");
                }
            }
            b'\x08' => {
                if escape {
                    out.push_str("\\b");
                } else {
                    out.push_str("\\8");
                }
            }
            b'\t' => {
                if escape {
                    out.push_str("\\t");
                } else {
                    out.push_str("\\9");
                }
            }
            b'\x0b' => {
                if escape {
                    out.push_str("\\v");
                } else {
                    out.push_str("\\11");
                }
            }
            b'\x0c' => {
                if escape {
                    out.push_str("\\f");
                } else {
                    out.push_str("\\12");
                }
            }
            b if b < 0x20 => {
                out.push_str(&format!("\\{b}"));
            }
            _ => out.push(byte as char),
        }
    }
    out.push('"');
    out
}

fn split_on_slash(s: &str, pathsep: &str) -> Vec<String> {
    let mut parts = Vec::new();
    if s.starts_with(|c: char| pathsep.contains(c)) {
        parts.push(String::new());
    }
    for fragment in s.split(|c: char| pathsep.contains(c)) {
        if !fragment.is_empty() {
            parts.push(fragment.to_string());
        }
    }
    parts
}

/// Implements common.normalize_path(filename).
fn normalize_path(lua: &Lua, filename: LuaValue) -> LuaResult<LuaValue> {
    let s = match &filename {
        LuaValue::String(s) => s.to_str()?.to_string(),
        LuaValue::Nil => return Ok(LuaNil),
        _ => return Ok(filename),
    };
    let pathsep = get_pathsep(lua)?;

    let mut filename_str;
    let mut volume = String::new();

    if pathsep == "\\" {
        filename_str = s.replace(['/', '\\'], "\\");
        let bytes = filename_str.as_bytes();
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\'
        {
            volume = format!("{}:\\", (bytes[0] as char).to_uppercase());
            filename_str = filename_str[3..].to_string();
        } else if filename_str.starts_with("\\\\") {
            // UNC path: \\server\share\
            if let Some(end) = filename_str[2..].find('\\').and_then(|first_sep| {
                let after = first_sep + 3;
                filename_str[after..].find('\\').map(|s| after + s + 1)
            }) {
                volume = filename_str[..end].to_string();
                filename_str = filename_str[end..].to_string();
            }
        }
    } else {
        filename_str = s.clone();
        if filename_str.starts_with('/') {
            volume = "/".to_string();
            filename_str = filename_str[1..].to_string();
        }
    }

    let parts = split_on_slash(&filename_str, &pathsep);
    let mut accu: Vec<String> = Vec::new();
    for part in &parts {
        if part == ".." {
            if !accu.is_empty() && accu.last().is_some_and(|p| p != "..") {
                accu.pop();
            } else if !volume.is_empty() {
                return Err(LuaError::runtime(format!(
                    "invalid path {volume}{filename_str}"
                )));
            } else {
                accu.push(part.clone());
            }
        } else if part != "." {
            accu.push(part.clone());
        }
    }
    let npath = accu.join(&pathsep);
    let result = if npath.is_empty() {
        format!("{volume}{pathsep}")
    } else {
        format!("{volume}{npath}")
    };
    Ok(LuaValue::String(lua.create_string(&result)?))
}

/// Implements common.relative_path(ref_dir, dir).
fn relative_path(lua: &Lua, (ref_dir, dir): (LuaString, LuaString)) -> LuaResult<String> {
    let pathsep = get_pathsep(lua)?;
    let ref_str = ref_dir.to_str()?.to_string();
    let dir_str = dir.to_str()?.to_string();

    // On Windows, check for different drive letters
    if pathsep == "\\" {
        let drive_of = |s: &str| -> Option<char> {
            let b = s.as_bytes();
            if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
                Some(b[0] as char)
            } else {
                None
            }
        };
        if let (Some(d1), Some(d2)) = (drive_of(&dir_str), drive_of(&ref_str)) {
            if d1 != d2 {
                return Ok(dir_str);
            }
        }
    }

    let ref_parts = split_on_slash(&ref_str, &pathsep);
    let dir_parts = split_on_slash(&dir_str, &pathsep);

    let mut i = 0;
    while i < ref_parts.len() && i < dir_parts.len() && ref_parts[i] == dir_parts[i] {
        i += 1;
    }

    let mut ups = String::new();
    for _ in i..ref_parts.len() {
        ups.push_str("..");
        ups.push_str(&pathsep);
    }

    let rel = dir_parts[i..].join(&pathsep);
    let result = format!("{ups}{rel}");
    if result.is_empty() {
        Ok(".".to_string())
    } else {
        Ok(result)
    }
}

/// Implements common.mkdirp(path) -> success, error, path.
fn mkdirp(lua: &Lua, path: String) -> LuaResult<LuaMultiValue> {
    let system: LuaTable = lua.globals().get("system")?;
    let pathsep = get_pathsep(lua)?;

    let get_file_info: LuaFunction = system.get("get_file_info")?;
    let stat: LuaValue = get_file_info.call(path.clone())?;
    if let LuaValue::Table(ref info) = stat {
        let ftype: LuaValue = info.get("type")?;
        if ftype != LuaNil {
            return Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Boolean(false),
                LuaValue::String(lua.create_string("path exists")?),
                LuaValue::String(lua.create_string(&path)?),
            ]));
        }
    }

    let mkdir: LuaFunction = system.get("mkdir")?;
    let mut subdirs: Vec<String> = Vec::new();
    let mut current = path.clone();
    while !current.is_empty() {
        let success: bool = mkdir
            .call::<LuaValue>(current.clone())?
            .as_boolean()
            .unwrap_or(false);
        if success {
            break;
        }
        // Split off last component
        if let Some(pos) = current.rfind(|c: char| pathsep.contains(c)) {
            let basename = current[pos + 1..].to_string();
            subdirs.insert(0, basename);
            current = current[..pos].to_string();
        } else {
            subdirs.insert(0, current.clone());
            current.clear();
        }
    }

    for dirname in &subdirs {
        current = if current.is_empty() {
            dirname.clone()
        } else {
            format!("{current}{pathsep}{dirname}")
        };
        let success: bool = mkdir
            .call::<LuaValue>(current.clone())?
            .as_boolean()
            .unwrap_or(false);
        if !success {
            return Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Boolean(false),
                LuaValue::String(lua.create_string("cannot create directory")?),
                LuaValue::String(lua.create_string(&current)?),
            ]));
        }
    }

    Ok(LuaMultiValue::from_vec(vec![LuaValue::Boolean(true)]))
}

/// Implements common.rm(path, recursively) -> success, error, path.
fn rm(lua: &Lua, path: String, recursively: bool) -> LuaResult<LuaMultiValue> {
    let system: LuaTable = lua.globals().get("system")?;
    let pathsep = get_pathsep(lua)?;
    let get_file_info: LuaFunction = system.get("get_file_info")?;
    let os_remove: LuaFunction = lua.globals().get::<LuaTable>("os")?.get("remove")?;

    let stat: LuaValue = get_file_info.call(path.clone())?;
    let ftype = match &stat {
        LuaValue::Table(info) => {
            let t: String = info.get::<String>("type").unwrap_or_default();
            t
        }
        _ => {
            return Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Boolean(false),
                LuaValue::String(lua.create_string("invalid path given")?),
                LuaValue::String(lua.create_string(&path)?),
            ]));
        }
    };

    if ftype != "file" && ftype != "dir" {
        return Ok(LuaMultiValue::from_vec(vec![
            LuaValue::Boolean(false),
            LuaValue::String(lua.create_string("invalid path given")?),
            LuaValue::String(lua.create_string(&path)?),
        ]));
    }

    if ftype == "file" {
        let result: LuaMultiValue = os_remove.call(path.clone())?;
        let rv = result.into_vec();
        if rv.is_empty() || rv[0] == LuaNil || rv[0] == LuaValue::Boolean(false) {
            let err_msg = rv
                .get(1)
                .and_then(|v| match v {
                    LuaValue::String(s) => s.to_str().ok().map(|s| s.to_string()),
                    _ => None,
                })
                .unwrap_or_default();
            return Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Boolean(false),
                LuaValue::String(lua.create_string(&err_msg)?),
                LuaValue::String(lua.create_string(&path)?),
            ]));
        }
        return Ok(LuaMultiValue::from_vec(vec![LuaValue::Boolean(true)]));
    }

    // Directory
    let list_dir: LuaFunction = system.get("list_dir")?;
    let contents_result: LuaMultiValue = list_dir.call(path.clone())?;
    let vals = contents_result.into_vec();
    let contents = match vals.first() {
        Some(LuaValue::Table(t)) => t.clone(),
        _ => lua.create_table()?,
    };
    let contents_len = contents.raw_len();

    if contents_len > 0 && !recursively {
        return Ok(LuaMultiValue::from_vec(vec![
            LuaValue::Boolean(false),
            LuaValue::String(lua.create_string("directory is not empty")?),
            LuaValue::String(lua.create_string(&path)?),
        ]));
    }

    for val in contents.sequence_values::<LuaString>() {
        let item = val?;
        let item_path = format!("{path}{pathsep}{}", item.to_str()?);
        let item_stat: LuaValue = get_file_info.call(item_path.clone())?;
        let item_type = match &item_stat {
            LuaValue::Table(info) => info.get::<String>("type").unwrap_or_default(),
            _ => {
                return Ok(LuaMultiValue::from_vec(vec![
                    LuaValue::Boolean(false),
                    LuaValue::String(lua.create_string("invalid file encountered")?),
                    LuaValue::String(lua.create_string(&item_path)?),
                ]));
            }
        };

        if item_type == "dir" {
            let result = rm(lua, item_path, recursively)?;
            let rv = result.into_vec();
            if rv.first().is_some_and(|v| v == &LuaValue::Boolean(false)) {
                return Ok(LuaMultiValue::from_vec(rv));
            }
        } else if item_type == "file" {
            let result: LuaMultiValue = os_remove.call(item_path.clone())?;
            let rv = result.into_vec();
            if rv.is_empty() || rv[0] == LuaNil || rv[0] == LuaValue::Boolean(false) {
                let err_msg = rv
                    .get(1)
                    .and_then(|v| match v {
                        LuaValue::String(s) => s.to_str().ok().map(|s| s.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default();
                return Ok(LuaMultiValue::from_vec(vec![
                    LuaValue::Boolean(false),
                    LuaValue::String(lua.create_string(&err_msg)?),
                    LuaValue::String(lua.create_string(&item_path)?),
                ]));
            }
        }
    }

    let rmdir: LuaFunction = system.get("rmdir")?;
    let result: LuaMultiValue = rmdir.call(path.clone())?;
    let rv = result.into_vec();
    if rv.is_empty() || rv[0] == LuaNil || rv[0] == LuaValue::Boolean(false) {
        let err_msg = rv
            .get(1)
            .and_then(|v| match v {
                LuaValue::String(s) => s.to_str().ok().map(|s| s.to_string()),
                _ => None,
            })
            .unwrap_or_default();
        return Ok(LuaMultiValue::from_vec(vec![
            LuaValue::Boolean(false),
            LuaValue::String(lua.create_string(&err_msg)?),
            LuaValue::String(lua.create_string(&path)?),
        ]));
    }

    Ok(LuaMultiValue::from_vec(vec![LuaValue::Boolean(true)]))
}
