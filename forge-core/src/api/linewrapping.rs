use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command = require_table(lua, "core.command")?;
    let cmds = lua.create_table()?;

    cmds.set(
        "line-wrapping:enable",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            if let Some(av) = core.get::<Option<LuaTable>>("active_view")? {
                if av.get::<Option<LuaTable>>("doc")?.is_some() {
                    av.set("wrapping_enabled", true)?;
                    super::linewrap::update_docview_breaks(lua, &av)?;
                }
            }
            Ok(())
        })?,
    )?;

    cmds.set(
        "line-wrapping:disable",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            if let Some(av) = core.get::<Option<LuaTable>>("active_view")? {
                if av.get::<Option<LuaTable>>("doc")?.is_some() {
                    av.set("wrapping_enabled", false)?;
                    let font = av.call_method::<LuaValue>("get_font", ())?;
                    super::linewrap::reconstruct_breaks(lua, &av, &font, f64::INFINITY)?;
                }
            }
            Ok(())
        })?,
    )?;

    cmds.set(
        "line-wrapping:toggle",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            let command = require_table(lua, "core.command")?;
            if let Some(av) = core.get::<Option<LuaTable>>("active_view")? {
                if av.get::<Option<LuaTable>>("doc")?.is_some() {
                    let is_active = super::linewrap::is_active(&av)?;
                    let cmd = if is_active { "line-wrapping:disable" } else { "line-wrapping:enable" };
                    command.call_function::<()>("perform", cmd)?;
                }
            }
            Ok(())
        })?,
    )?;

    command.call_function::<()>("add", (LuaValue::Nil, cmds))?;

    let keymap = require_table(lua, "core.keymap")?;
    let keys = lua.create_table()?;
    keys.set("f10", "line-wrapping:toggle")?;
    keymap.call_function::<()>("add", keys)?;

    Ok(())
}

fn translate_end_of_line(lua: &Lua, (doc, line, col): (LuaTable, usize, usize)) -> LuaResult<(usize, LuaValue)> {
    let core = require_table(lua, "core")?;
    let active_view: Option<LuaTable> = core.get("active_view")?;

    let wrap_active = if let Some(av) = &active_view {
        let av_doc: Option<LuaTable> = av.get("doc")?;
        let doc_matches = av_doc.as_ref().and_then(|d| d.equals(&doc).ok()).unwrap_or(false);
        doc_matches && super::linewrap::is_active(av)?
    } else {
        false
    };

    if !wrap_active {
        return Ok((line, LuaValue::Number(f64::INFINITY)));
    }

    let av = active_view.unwrap();
    let (idx, _, _, _) = super::linewrap::get_line_idx_col_count(&av, line, Some(col), false)?;
    let (nline, ncol2) = super::linewrap::get_idx_line_col(&av, idx + 1)?;
    if nline != line {
        Ok((line, LuaValue::Number(f64::INFINITY)))
    } else {
        Ok((line, LuaValue::Integer(ncol2 as i64 - 1)))
    }
}

fn translate_start_of_line(lua: &Lua, (doc, line, col): (LuaTable, usize, usize)) -> LuaResult<(usize, usize)> {
    let core = require_table(lua, "core")?;
    let active_view: Option<LuaTable> = core.get("active_view")?;

    let wrap_active = if let Some(av) = &active_view {
        let av_doc: Option<LuaTable> = av.get("doc")?;
        let doc_matches = av_doc.as_ref().and_then(|d| d.equals(&doc).ok()).unwrap_or(false);
        doc_matches && super::linewrap::is_active(av)?
    } else {
        false
    };

    if !wrap_active {
        return Ok((line, 1));
    }

    let av = active_view.unwrap();
    let (_, _, _, scol) = super::linewrap::get_line_idx_col_count(&av, line, Some(col), false)?;
    Ok((line, scol))
}

/// Registers the `plugins.linewrapping` preload — all logic is in Rust.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.linewrapping",
        lua.create_function(|lua, ()| {
            let translate = require_table(lua, "core.doc.translate")?;
            translate.set("end_of_line", lua.create_function(translate_end_of_line)?)?;
            translate.set("start_of_line", lua.create_function(translate_start_of_line)?)?;
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )?;
    Ok(())
}
