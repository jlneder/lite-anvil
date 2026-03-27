use mlua::prelude::*;

/// Registers `core.doc.translate` -- cursor position translation helpers.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.doc.translate",
        lua.create_function(|lua, ()| {
            let module = lua.create_table()?;

            let is_non_word = lua.create_function(|lua, ch: LuaString| {
                let config: LuaTable = lua
                    .globals()
                    .get::<LuaFunction>("require")?
                    .call("core.config")?;
                let non_word_chars: LuaString = config.get("non_word_chars")?;
                let ch_bytes: &[u8] = &ch.as_bytes();
                Ok(non_word_chars
                    .as_bytes()
                    .windows(ch_bytes.len())
                    .any(|w| w == ch_bytes))
            })?;
            lua.set_named_registry_value("doc_translate.is_non_word", is_non_word)?;

            module.set(
                "previous_char",
                lua.create_function(|lua, (doc, line, col): (LuaTable, i64, i64)| {
                    let common: LuaTable = lua
                        .globals()
                        .get::<LuaFunction>("require")?
                        .call("core.common")?;
                    let is_utf8_cont: LuaFunction = common.get("is_utf8_cont")?;
                    let pos_offset: LuaFunction = doc.get("position_offset")?;
                    let get_char: LuaFunction = doc.get("get_char")?;
                    let (mut l, mut c): (i64, i64) = pos_offset.call((&doc, line, col, -1))?;
                    loop {
                        let ch: LuaString = get_char.call((&doc, l, c))?;
                        if !is_utf8_cont.call::<bool>(ch)? {
                            break;
                        }
                        let (nl, nc): (i64, i64) = pos_offset.call((&doc, l, c, -1))?;
                        if nl == l && nc == c {
                            break;
                        }
                        (l, c) = (nl, nc);
                    }
                    Ok((l, c))
                })?,
            )?;

            module.set(
                "next_char",
                lua.create_function(|lua, (doc, line, col): (LuaTable, i64, i64)| {
                    let common: LuaTable = lua
                        .globals()
                        .get::<LuaFunction>("require")?
                        .call("core.common")?;
                    let is_utf8_cont: LuaFunction = common.get("is_utf8_cont")?;
                    let pos_offset: LuaFunction = doc.get("position_offset")?;
                    let get_char: LuaFunction = doc.get("get_char")?;
                    let (mut l, mut c): (i64, i64) = pos_offset.call((&doc, line, col, 1))?;
                    loop {
                        let ch: LuaString = get_char.call((&doc, l, c))?;
                        if !is_utf8_cont.call::<bool>(ch)? {
                            break;
                        }
                        let (nl, nc): (i64, i64) = pos_offset.call((&doc, l, c, 1))?;
                        if nl == l && nc == c {
                            break;
                        }
                        (l, c) = (nl, nc);
                    }
                    Ok((l, c))
                })?,
            )?;

            module.set(
                "start_of_word",
                lua.create_function(|lua, (doc, line, col): (LuaTable, i64, i64)| {
                    let is_non_word: LuaFunction =
                        lua.named_registry_value("doc_translate.is_non_word")?;
                    let pos_offset: LuaFunction = doc.get("position_offset")?;
                    let get_char: LuaFunction = doc.get("get_char")?;
                    let (mut l, mut c) = (line, col);
                    loop {
                        let (l2, c2): (i64, i64) = pos_offset.call((&doc, l, c, -1))?;
                        let ch: LuaString = get_char.call((&doc, l2, c2))?;
                        if is_non_word.call::<bool>(ch)? || (l == l2 && c == c2) {
                            break;
                        }
                        (l, c) = (l2, c2);
                    }
                    Ok((l, c))
                })?,
            )?;

            module.set(
                "end_of_word",
                lua.create_function(|lua, (doc, line, col): (LuaTable, i64, i64)| {
                    let is_non_word: LuaFunction =
                        lua.named_registry_value("doc_translate.is_non_word")?;
                    let pos_offset: LuaFunction = doc.get("position_offset")?;
                    let get_char: LuaFunction = doc.get("get_char")?;
                    let (mut l, mut c) = (line, col);
                    loop {
                        let (l2, c2): (i64, i64) = pos_offset.call((&doc, l, c, 1))?;
                        let ch: LuaString = get_char.call((&doc, l, c))?;
                        if is_non_word.call::<bool>(ch)? || (l == l2 && c == c2) {
                            break;
                        }
                        (l, c) = (l2, c2);
                    }
                    Ok((l, c))
                })?,
            )?;

            module.set(
                "previous_word_start",
                lua.create_function(|lua, (doc, line, col): (LuaTable, i64, i64)| {
                    let is_non_word: LuaFunction =
                        lua.named_registry_value("doc_translate.is_non_word")?;
                    let pos_offset: LuaFunction = doc.get("position_offset")?;
                    let get_char: LuaFunction = doc.get("get_char")?;
                    let start_of_word: LuaFunction = lua
                        .globals()
                        .get::<LuaFunction>("require")?
                        .call::<LuaTable>("core.doc.translate")?
                        .get("start_of_word")?;
                    let (mut l, mut c) = (line, col);
                    let mut prev: Option<Vec<u8>> = None;
                    while l > 1 || c > 1 {
                        let (nl, nc): (i64, i64) = pos_offset.call((&doc, l, c, -1))?;
                        let ch: LuaString = get_char.call((&doc, nl, nc))?;
                        let is_nw = is_non_word.call::<bool>(ch.clone())?;
                        let ch_bytes: &[u8] = &ch.as_bytes();
                        if prev.as_deref().is_some_and(|p| p != ch_bytes) || !is_nw {
                            break;
                        }
                        prev = Some(ch_bytes.to_vec());
                        (l, c) = (nl, nc);
                    }
                    start_of_word.call::<LuaMultiValue>((&doc, l, c))
                })?,
            )?;

            module.set(
                "next_word_end",
                lua.create_function(|lua, (doc, line, col): (LuaTable, i64, i64)| {
                    let is_non_word: LuaFunction =
                        lua.named_registry_value("doc_translate.is_non_word")?;
                    let pos_offset: LuaFunction = doc.get("position_offset")?;
                    let get_char: LuaFunction = doc.get("get_char")?;
                    let translate: LuaTable = lua
                        .globals()
                        .get::<LuaFunction>("require")?
                        .call("core.doc.translate")?;
                    let end_of_word: LuaFunction = translate.get("end_of_word")?;
                    let end_of_doc: LuaFunction = translate.get("end_of_doc")?;
                    let (end_line, end_col): (i64, i64) = end_of_doc.call((&doc, line, col))?;
                    let (mut l, mut c) = (line, col);
                    let mut prev: Option<Vec<u8>> = None;
                    while l < end_line || c < end_col {
                        let ch: LuaString = get_char.call((&doc, l, c))?;
                        let is_nw = is_non_word.call::<bool>(ch.clone())?;
                        let ch_bytes: &[u8] = &ch.as_bytes();
                        if prev.as_deref().is_some_and(|p| p != ch_bytes) || !is_nw {
                            break;
                        }
                        (l, c) = pos_offset.call((&doc, l, c, 1))?;
                        prev = Some(ch_bytes.to_vec());
                    }
                    end_of_word.call::<LuaMultiValue>((&doc, l, c))
                })?,
            )?;

            module.set(
                "previous_block_start",
                lua.create_function(|_, (doc, line, _col): (LuaTable, i64, i64)| {
                    let lines: LuaTable = doc.get("lines")?;
                    let mut l = line;
                    loop {
                        l -= 1;
                        if l <= 1 {
                            return Ok((1i64, 1i64));
                        }
                        let prev_line: String = lines.raw_get(l - 1)?;
                        let cur_line: String = lines.raw_get(l)?;
                        let prev_blank = prev_line.trim().is_empty();
                        let cur_blank = cur_line.trim().is_empty();
                        if prev_blank && !cur_blank {
                            let first_non_ws = cur_line
                                .find(|c: char| !c.is_whitespace())
                                .map(|i| (i + 1) as i64)
                                .unwrap_or(1);
                            return Ok((l, first_non_ws));
                        }
                    }
                })?,
            )?;

            module.set(
                "next_block_end",
                lua.create_function(|_, (doc, line, _col): (LuaTable, i64, i64)| {
                    let lines: LuaTable = doc.get("lines")?;
                    let num_lines = lines.raw_len() as i64;
                    let mut l = line;
                    loop {
                        if l >= num_lines {
                            let last_line: String = lines.raw_get(num_lines)?;
                            return Ok((num_lines, last_line.len() as i64));
                        }
                        let next_line: String = lines.raw_get(l + 1)?;
                        let cur_line: String = lines.raw_get(l)?;
                        let next_blank = next_line.trim().is_empty();
                        let cur_blank = cur_line.trim().is_empty();
                        if next_blank && !cur_blank {
                            return Ok((l + 1, next_line.len() as i64));
                        }
                        l += 1;
                    }
                })?,
            )?;

            module.set(
                "start_of_line",
                lua.create_function(|_, (_doc, line, _col): (LuaTable, i64, i64)| {
                    Ok((line, 1i64))
                })?,
            )?;

            module.set(
                "start_of_indentation",
                lua.create_function(|_, (doc, line, col): (LuaTable, i64, i64)| {
                    let lines: LuaTable = doc.get("lines")?;
                    let line_text: String = lines.raw_get(line)?;
                    let indent_end =
                        line_text.find(|c: char| !c.is_whitespace()).unwrap_or(0) as i64 + 1;
                    let result_col = if col > indent_end { indent_end } else { 1 };
                    Ok((line, result_col))
                })?,
            )?;

            module.set(
                "end_of_line",
                lua.create_function(|_, (_doc, line, _col): (LuaTable, i64, i64)| {
                    Ok((line, i64::MAX))
                })?,
            )?;

            module.set(
                "start_of_doc",
                lua.create_function(|_, (_doc, _line, _col): (LuaTable, i64, i64)| {
                    Ok((1i64, 1i64))
                })?,
            )?;

            module.set(
                "end_of_doc",
                lua.create_function(|_, (doc, _line, _col): (LuaTable, i64, i64)| {
                    let lines: LuaTable = doc.get("lines")?;
                    let num_lines = lines.raw_len() as i64;
                    let last_line: String = lines.raw_get(num_lines)?;
                    Ok((num_lines, last_line.len() as i64))
                })?,
            )?;

            Ok(LuaValue::Table(module))
        })?,
    )
}
