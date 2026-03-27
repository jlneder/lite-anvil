use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

fn font_get_width(font: &LuaValue, text: &str) -> LuaResult<f64> {
    match font {
        LuaValue::Table(t) => t.call_method("get_width", text),
        LuaValue::UserData(ud) => ud.call_method("get_width", text),
        _ => Ok(0.0),
    }
}

fn font_call<R: FromLuaMulti>(
    font: &LuaValue,
    method: &str,
    args: impl IntoLuaMulti,
) -> LuaResult<R> {
    match font {
        LuaValue::Table(t) => t.call_method(method, args),
        LuaValue::UserData(ud) => ud.call_method(method, args),
        other => Err(LuaError::runtime(format!(
            "expected font, got {}",
            other.type_name()
        ))),
    }
}

fn linewrap_config(lua: &Lua) -> LuaResult<LuaTable> {
    let config = require_table(lua, "core.config")?;
    let plugins: LuaTable = config.get("plugins")?;
    plugins
        .get::<Option<LuaTable>>("linewrapping")?
        .ok_or_else(|| LuaError::runtime("missing linewrapping config"))
}

// ── Wrap-state field names on DocView ─────────────────────────────────────────
// wrapped_lines       : flat LuaTable [line1, col1, line2, col2, ...]
// wrapped_line_to_idx : LuaTable keyed by original line → first wrapped idx
// wrapped_line_offsets: LuaTable keyed by original line → indent x-offset (f64)
// wrapped_settings    : LuaTable {width, font} or nil when wrapping disabled
// wrapping_enabled    : bool
// _wrap_cid           : i64 doc change-id at last reconstruct

/// True when wrapping is active (wrapped_settings is non-nil).
pub(crate) fn is_active(docview: &LuaTable) -> LuaResult<bool> {
    Ok(docview
        .get::<Option<LuaTable>>("wrapped_settings")?
        .is_some())
}

/// Total number of visual (wrapped) lines.
pub(crate) fn get_total_wrapped_lines(docview: &LuaTable) -> LuaResult<usize> {
    if let Some(wl) = docview.get::<Option<LuaTable>>("wrapped_lines")? {
        Ok(wl.raw_len() / 2)
    } else {
        let doc: LuaTable = docview.get("doc")?;
        let lines: LuaTable = doc.get("lines")?;
        Ok(lines.raw_len())
    }
}

/// Visual index (1-based) → (original_line, start_col).
pub(crate) fn get_idx_line_col(docview: &LuaTable, idx: usize) -> LuaResult<(usize, usize)> {
    let doc: LuaTable = docview.get("doc")?;
    let lines_table: LuaTable = doc.get("lines")?;
    let n = lines_table.raw_len();
    if let Some(wl) = docview.get::<Option<LuaTable>>("wrapped_lines")? {
        if idx < 1 {
            return Ok((1, 1));
        }
        let offset = (idx - 1) * 2 + 1;
        if offset > wl.raw_len() {
            let last_text: String = lines_table.get(n)?;
            return Ok((n, last_text.len() + 1));
        }
        Ok((wl.get(offset)?, wl.get(offset + 1)?))
    } else {
        if idx > n {
            let last_text: String = lines_table.get(n)?;
            return Ok((n, last_text.len() + 1));
        }
        Ok((idx, 1))
    }
}

/// Byte length of the content in the visual segment at the given index.
pub(crate) fn get_idx_line_length(docview: &LuaTable, idx: usize) -> LuaResult<usize> {
    let doc: LuaTable = docview.get("doc")?;
    let lines_table: LuaTable = doc.get("lines")?;
    if let Some(wl) = docview.get::<Option<LuaTable>>("wrapped_lines")? {
        let offset = (idx - 1) * 2 + 1;
        let current_line: usize = wl.get(offset)?;
        let start: usize = wl.get(offset + 1)?;
        let next_line: Option<usize> = wl.get(offset + 2)?;
        if next_line == Some(current_line) {
            let next_col: usize = wl.get(offset + 3)?;
            Ok(next_col - start)
        } else {
            let text: String = lines_table.get(current_line)?;
            Ok(text.len() - start + 1)
        }
    } else {
        let text: String = lines_table.get(idx)?;
        Ok(text.len() + 1)
    }
}

/// Maps (original_line, col?) → (wrapped_idx, ncol, count, scol).
///
/// - `idx`  : 1-based visual index for the segment containing `col`
/// - `ncol` : column relative to the segment start (1-based)
/// - `count`: total number of visual rows this original line spans
/// - `scol` : 1-based start column of the segment containing `col`
pub(crate) fn get_line_idx_col_count(
    docview: &LuaTable,
    line: usize,
    col: Option<usize>,
    line_end: bool,
) -> LuaResult<(usize, usize, usize, usize)> {
    let doc: LuaTable = docview.get("doc")?;
    let lines_table: LuaTable = doc.get("lines")?;
    let n = lines_table.raw_len();

    if let Some(wl) = docview.get::<Option<LuaTable>>("wrapped_lines")? {
        let wl_len = wl.raw_len();
        if line > n {
            let last_text: String = lines_table.get(n)?;
            return get_line_idx_col_count(docview, n, Some(last_text.len() + 1), false);
        }
        let line = line.max(1);
        let wl_to_idx: LuaTable = docview.get("wrapped_line_to_idx")?;
        let total = wl_len / 2;
        let first_idx: usize = wl_to_idx.get(line).unwrap_or(1);
        let mut idx = first_idx;
        let mut scol = 1usize;

        if let Some(col) = col {
            let mut i = idx + 1;
            loop {
                let entry_line: Option<usize> = wl.get((i - 1) * 2 + 1)?;
                if entry_line != Some(line) {
                    break;
                }
                let entry_col: usize = wl.get((i - 1) * 2 + 2)?;
                if col < entry_col || (line_end && col == entry_col) {
                    break;
                }
                scol = entry_col;
                i += 1;
                idx += 1;
            }
        }

        let ncol = col.map(|c| c.saturating_sub(scol) + 1).unwrap_or(1);
        let next_idx: usize = wl_to_idx.get(line + 1).unwrap_or(total + 1);
        let count = next_idx - first_idx;
        Ok((idx, ncol, count, scol))
    } else {
        let line = line.clamp(1, n);
        Ok((line, col.unwrap_or(1), 1, 1))
    }
}

// ── Wrap computation ───────────────────────────────────────────────────────────

/// Computes wrap-break column positions for a single document line.
///
/// Returns `(splits, begin_width)` where `splits` is a vec of 1-based byte-column
/// indices at which new visual rows begin (always starts with `[1]`), and
/// `begin_width` is the x-offset applied to continuation rows (indent following).
fn compute_line_breaks(
    font: &LuaValue,
    line_text: &str,
    width: f64,
    mode: &str,
    indent: bool,
) -> LuaResult<(Vec<usize>, f64)> {
    let mut xoffset = 0.0f64;
    let mut begin_width = 0.0f64;
    let mut last_space: Option<usize> = None;
    let mut last_width = 0.0f64;
    let mut splits = vec![1usize];

    if indent {
        let trimmed_len = line_text.len() - line_text.trim_start().len();
        if trimmed_len > 0 {
            begin_width = font_get_width(font, &line_text[..trimmed_len])?;
        }
    }

    let total_w = font_get_width(font, line_text)?;
    if total_w <= width {
        return Ok((splits, begin_width));
    }

    let mut i = 1usize;
    for ch in line_text.chars() {
        let w = font_get_width(font, &ch.to_string())?;
        xoffset += w;
        if xoffset > width {
            if mode == "word" {
                if let Some(ls) = last_space {
                    splits.push(ls + 1);
                    xoffset = w + begin_width + (xoffset - last_width);
                } else {
                    splits.push(i);
                    xoffset = w + begin_width;
                }
            } else {
                splits.push(i);
                xoffset = w + begin_width;
            }
            last_space = None;
        } else if ch == ' ' {
            last_space = Some(i);
            last_width = xoffset;
        }
        i += ch.len_utf8();
    }

    Ok((splits, begin_width))
}

/// Rebuilds the full wrap state for all document lines and stores it on `docview`.
///
/// Pass `f64::INFINITY` as `width` to disable wrapping (sets all tables to nil).
pub(crate) fn reconstruct_breaks(
    lua: &Lua,
    docview: &LuaTable,
    font: &LuaValue,
    width: f64,
) -> LuaResult<()> {
    if width == f64::INFINITY || width <= 0.0 {
        docview.set("wrapped_lines", LuaValue::Nil)?;
        docview.set("wrapped_line_to_idx", LuaValue::Nil)?;
        docview.set("wrapped_line_offsets", LuaValue::Nil)?;
        docview.set("wrapped_settings", LuaValue::Nil)?;
        return Ok(());
    }

    let cfg = linewrap_config(lua)?;
    let mode: String = cfg
        .get::<Option<String>>("mode")?
        .unwrap_or_else(|| "letter".into());
    let indent: bool = cfg.get::<Option<bool>>("indent")?.unwrap_or(true);

    let doc: LuaTable = docview.get("doc")?;
    let lines_table: LuaTable = doc.get("lines")?;
    let line_count = lines_table.raw_len();

    // Build in Rust vecs for speed, then write to Lua tables once.
    let mut wrapped_lines: Vec<usize> = Vec::with_capacity(line_count * 2 + 8);
    let mut to_idx: Vec<usize> = Vec::with_capacity(line_count + 1);
    let mut offsets: Vec<f64> = Vec::with_capacity(line_count + 1);

    let mut wrapped_idx = 1usize;
    for i in 1..=line_count {
        let text: String = lines_table.get(i)?;
        let (splits, begin_w) = compute_line_breaks(font, &text, width, &mode, indent)?;
        offsets.push(begin_w);
        to_idx.push(wrapped_idx);
        for col in &splits {
            wrapped_lines.push(i);
            wrapped_lines.push(*col);
            wrapped_idx += 1;
        }
    }

    let wl_tbl = lua.create_table_with_capacity(wrapped_lines.len(), 0)?;
    for (k, v) in wrapped_lines.iter().enumerate() {
        wl_tbl.raw_set(k + 1, *v)?;
    }

    let to_idx_tbl = lua.create_table_with_capacity(to_idx.len(), 0)?;
    for (k, v) in to_idx.iter().enumerate() {
        to_idx_tbl.raw_set(k + 1, *v)?;
    }

    let offsets_tbl = lua.create_table_with_capacity(offsets.len(), 0)?;
    for (k, v) in offsets.iter().enumerate() {
        offsets_tbl.raw_set(k + 1, *v)?;
    }

    let settings = lua.create_table()?;
    settings.set("width", width)?;
    settings.set("font", font.clone())?;

    docview.set("wrapped_lines", wl_tbl)?;
    docview.set("wrapped_line_to_idx", to_idx_tbl)?;
    docview.set("wrapped_line_offsets", offsets_tbl)?;
    docview.set("wrapped_settings", settings)?;

    Ok(())
}

/// Rebuilds wrap state if the view width or document content has changed.
pub(crate) fn update_docview_breaks(lua: &Lua, docview: &LuaTable) -> LuaResult<()> {
    let size: LuaTable = docview.get("size")?;
    if size.get::<f64>("x")? <= 0.0 {
        return Ok(());
    }

    let target_width = compute_target_width(lua, docview)?;

    let doc: LuaTable = docview.get("doc")?;
    let change_id: i64 = doc.call_method("get_change_id", ())?;

    let current_width: Option<f64> = docview
        .get::<Option<LuaTable>>("wrapped_settings")?
        .and_then(|s| s.get("width").ok());
    let cached_cid: Option<i64> = docview.get("_wrap_cid")?;

    if current_width == Some(target_width) && cached_cid == Some(change_id) {
        return Ok(());
    }

    let scroll: LuaTable = docview.get("scroll")?;
    let to: LuaTable = scroll.get("to")?;
    to.set("x", 0.0f64)?;

    let font = docview.call_method::<LuaValue>("get_font", ())?;
    reconstruct_breaks(lua, docview, &font, target_width)?;
    docview.set("_wrap_cid", change_id)?;

    Ok(())
}

fn compute_target_width(lua: &Lua, docview: &LuaTable) -> LuaResult<f64> {
    let cfg = linewrap_config(lua);
    if let Ok(cfg) = cfg {
        let width_override: LuaValue = cfg.get("width_override").unwrap_or(LuaValue::Nil);
        match &width_override {
            LuaValue::Function(f) => return f.call::<f64>(docview.clone()),
            LuaValue::Number(n) => return Ok(*n),
            LuaValue::Integer(i) => return Ok(*i as f64),
            _ => {}
        }
    }
    let v_scrollbar: LuaTable = docview.get("v_scrollbar")?;
    let expanded_size: Option<f64> = v_scrollbar.get("expanded_size")?;
    let style = require_table(lua, "core.style")?;
    let default_scrollbar: f64 = style.get("expanded_scrollbar_size").unwrap_or(12.0);
    let scrollbar_w = expanded_size.unwrap_or(default_scrollbar);
    let gw: f64 = docview.call_method("get_gutter_width", ())?;
    let size: LuaTable = docview.get("size")?;
    Ok(size.get::<f64>("x")? - gw - scrollbar_w)
}

// ── Wrap-aware coordinate calculations ────────────────────────────────────────

/// x-offset of `col` within the visual row of `line`, accounting for wrap indentation.
pub(crate) fn get_col_x_offset(
    lua: &Lua,
    docview: &LuaTable,
    line: usize,
    col: usize,
    line_end: bool,
) -> LuaResult<f64> {
    let (_, _, _, scol) = get_line_idx_col_count(docview, line, Some(col), line_end)?;
    let offsets: LuaTable = docview.get("wrapped_line_offsets")?;
    let xoffset_start: f64 = if scol != 1 {
        offsets.get(line).unwrap_or(0.0)
    } else {
        0.0
    };

    let doc: LuaTable = docview.get("doc")?;
    let highlighter: LuaTable = doc.get("highlighter")?;
    let line_info: LuaTable = highlighter.call_method("get_line", line)?;
    let tokens: LuaTable = line_info.get("tokens")?;

    let default_font = docview.call_method::<LuaValue>("get_font", ())?;
    let style = require_table(lua, "core.style")?;
    let syntax_fonts: LuaTable = style.get("syntax_fonts")?;

    let mut xoffset = xoffset_start;
    let mut i = 1usize;
    let mut tok_idx = 1usize;

    while tok_idx <= tokens.raw_len() {
        let token_type: String = tokens.get(tok_idx)?;
        let text: String = tokens.get(tok_idx + 1)?;
        let tok_end = i + text.len();

        if tok_end > scol {
            let font = syntax_fonts
                .get::<Option<LuaValue>>(token_type.as_str())?
                .unwrap_or_else(|| default_font.clone());
            let mut char_i = i;
            for ch in text.chars() {
                if char_i >= scol && char_i < col {
                    xoffset += font_call::<f64>(&font, "get_width", ch.to_string())?;
                } else if char_i >= col {
                    return Ok(xoffset);
                }
                char_i += ch.len_utf8();
            }
        }

        i += text.len();
        tok_idx += 2;
    }
    Ok(xoffset)
}

/// Maps visual index `idx` and screen x-position to (original_line, column).
pub(crate) fn get_line_col_from_x(
    lua: &Lua,
    docview: &LuaTable,
    idx: usize,
    x: f64,
) -> LuaResult<(usize, usize)> {
    if idx < 1 {
        return Ok((1, 1));
    }
    let (line, col) = get_idx_line_col(docview, idx)?;
    let offsets: LuaTable = docview.get("wrapped_line_offsets")?;
    let xoffset_start: f64 = if col != 1 {
        offsets.get(line).unwrap_or(0.0)
    } else {
        0.0
    };

    if x < xoffset_start {
        return Ok((line, col));
    }

    let doc: LuaTable = docview.get("doc")?;
    let lines_table: LuaTable = doc.get("lines")?;
    let highlighter: LuaTable = doc.get("highlighter")?;
    let line_info: LuaTable = highlighter.call_method("get_line", line)?;
    let tokens: LuaTable = line_info.get("tokens")?;

    let default_font = docview.call_method::<LuaValue>("get_font", ())?;
    let style = require_table(lua, "core.style")?;
    let syntax_fonts: LuaTable = style.get("syntax_fonts")?;

    let mut xoffset = xoffset_start;
    let mut last_i = col;
    let mut i = 1usize;
    let mut prev_w = 0.0f64;
    let mut tok_idx = 1usize;

    while tok_idx <= tokens.raw_len() {
        let token_type: String = tokens.get(tok_idx)?;
        let text: String = tokens.get(tok_idx + 1)?;
        let font = syntax_fonts
            .get::<Option<LuaValue>>(token_type.as_str())?
            .unwrap_or_else(|| default_font.clone());

        for ch in text.chars() {
            if i >= col {
                if xoffset >= x {
                    // prev_w is width of previous character (0 on first char of segment)
                    if xoffset - x > prev_w / 2.0 {
                        return Ok((line, last_i));
                    } else {
                        return Ok((line, i));
                    }
                }
                prev_w = font_call::<f64>(&font, "get_width", ch.to_string())?;
                xoffset += prev_w;
            }
            last_i = i;
            i += ch.len_utf8();
        }
        tok_idx += 2;
    }

    let last_text: String = lines_table.get(line)?;
    Ok((line, last_text.len()))
}

/// Draws the vertical guide line at the wrap width, if enabled.
pub(crate) fn draw_guide(lua: &Lua, docview: &LuaTable) -> LuaResult<()> {
    let cfg = match linewrap_config(lua) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let guide: bool = cfg.get::<Option<bool>>("guide")?.unwrap_or(true);
    if !guide {
        return Ok(());
    }
    let settings: Option<LuaTable> = docview.get("wrapped_settings")?;
    let settings = match settings {
        Some(s) => s,
        None => return Ok(()),
    };
    let wrap_width: f64 = settings.get("width")?;
    if wrap_width.is_infinite() {
        return Ok(());
    }

    let (x, y): (f64, f64) = docview.call_method("get_content_offset", ())?;
    let gw: f64 = docview.call_method("get_gutter_width", ())?;

    let style = require_table(lua, "core.style")?;
    let selection_color: LuaValue = style.get("selection")?;

    let core = require_table(lua, "core")?;
    let root_view: LuaTable = core.get("root_view")?;
    let root_size: LuaTable = root_view.get("size")?;
    let h: f64 = root_size.get("y")?;

    let renderer: LuaTable = lua.globals().get("renderer")?;
    renderer.call_function::<()>(
        "draw_rect",
        (x + gw + wrap_width, y, 1.0, h, selection_color),
    )
}
