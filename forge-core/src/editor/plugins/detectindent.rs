use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command = require_table(lua, "core.command")?;

    // ── set-file-indent-type ─────────────────────────────────────────────────
    let cmd_set_type = lua.create_function(|lua, dv: LuaTable| {
        let core = require_table(lua, "core")?;
        let command_view: LuaTable = core.get("command_view")?;
        let dv_key = lua.create_registry_value(dv)?;
        let opts = lua.create_table()?;
        opts.set(
            "submit",
            lua.create_function(move |lua, (value, _item): (String, LuaValue)| {
                let dv: LuaTable = lua.registry_value(&dv_key)?;
                let doc: LuaTable = dv.get("doc")?;
                let indent_type = if value.to_lowercase() == "tabs" {
                    "hard"
                } else {
                    "soft"
                };
                if let Some(info) = doc.get::<Option<LuaTable>>("indent_info")? {
                    info.set("type", indent_type)?;
                    info.set("confirmed", true)?;
                } else {
                    let info = lua.create_table()?;
                    info.set("type", indent_type)?;
                    info.set("size", 4i64)?;
                    info.set("confirmed", true)?;
                    doc.set("indent_info", info)?;
                }
                Ok(())
            })?,
        )?;
        opts.set(
            "suggest",
            lua.create_function(|lua, text: String| {
                let common = require_table(lua, "core.common")?;
                let choices = lua.create_sequence_from(["tabs", "spaces"])?;
                common.call_function::<LuaTable>("fuzzy_match", (choices, text))
            })?,
        )?;
        opts.set(
            "validate",
            lua.create_function(|_, text: String| {
                let t = text.to_lowercase();
                Ok(t == "tabs" || t == "spaces")
            })?,
        )?;
        command_view.call_method::<()>("enter", ("Specify indent style for this file", opts))
    })?;

    // ── set-file-indent-size ─────────────────────────────────────────────────
    let cmd_set_size = lua.create_function(|lua, dv: LuaTable| {
        let core = require_table(lua, "core")?;
        let command_view: LuaTable = core.get("command_view")?;
        let dv_key = lua.create_registry_value(dv)?;
        let opts = lua.create_table()?;
        opts.set(
            "submit",
            lua.create_function(move |lua, (value, _item): (String, LuaValue)| {
                let dv: LuaTable = lua.registry_value(&dv_key)?;
                let doc: LuaTable = dv.get("doc")?;
                let size: i64 = value.trim().parse().unwrap_or(4).max(1);
                if let Some(info) = doc.get::<Option<LuaTable>>("indent_info")? {
                    info.set("size", size)?;
                    info.set("confirmed", true)?;
                } else {
                    let info = lua.create_table()?;
                    info.set("type", "soft")?;
                    info.set("size", size)?;
                    info.set("confirmed", true)?;
                    doc.set("indent_info", info)?;
                }
                Ok(())
            })?,
        )?;
        opts.set(
            "validate",
            lua.create_function(|_, value: String| {
                Ok(value.trim().parse::<i64>().map(|v| v >= 1).unwrap_or(false))
            })?,
        )?;
        command_view.call_method::<()>("enter", ("Specify indent size for current file", opts))
    })?;

    let docview_cmds = lua.create_table()?;
    docview_cmds.set("indent:set-file-indent-type", cmd_set_type)?;
    docview_cmds.set("indent:set-file-indent-size", cmd_set_size)?;
    command.call_function::<()>("add", ("core.docview", docview_cmds))?;

    // ── switch to tabs (only when currently soft) ────────────────────────────
    let to_tabs_pred = lua.create_function(|lua, ()| {
        let core = require_table(lua, "core")?;
        let av: LuaTable = core.get("active_view")?;
        let dv = require_table(lua, "core.docview")?;
        if !av.call_method::<bool>("is", dv)? {
            return Ok(false);
        }
        let doc: LuaTable = av.get("doc")?;
        let info: Option<LuaTable> = doc.get("indent_info")?;
        Ok(info
            .and_then(|t| t.get::<Option<String>>("type").ok().flatten())
            .as_deref()
            == Some("soft"))
    })?;
    let to_tabs_cmd = lua.create_function(|lua, ()| {
        let av: LuaTable = require_table(lua, "core")?.get("active_view")?;
        let doc: LuaTable = av.get("doc")?;
        if let Some(info) = doc.get::<Option<LuaTable>>("indent_info")? {
            info.set("type", "hard")?;
            info.set("confirmed", true)?;
        }
        Ok(())
    })?;
    let tabs_cmds = lua.create_table()?;
    tabs_cmds.set("indent:switch-file-to-tabs-indentation", to_tabs_cmd)?;
    command.call_function::<()>("add", (to_tabs_pred, tabs_cmds))?;

    // ── switch to spaces (only when currently hard) ──────────────────────────
    let to_spaces_pred = lua.create_function(|lua, ()| {
        let core = require_table(lua, "core")?;
        let av: LuaTable = core.get("active_view")?;
        let dv = require_table(lua, "core.docview")?;
        if !av.call_method::<bool>("is", dv)? {
            return Ok(false);
        }
        let doc: LuaTable = av.get("doc")?;
        let info: Option<LuaTable> = doc.get("indent_info")?;
        Ok(info
            .and_then(|t| t.get::<Option<String>>("type").ok().flatten())
            .as_deref()
            == Some("hard"))
    })?;
    let to_spaces_cmd = lua.create_function(|lua, ()| {
        let av: LuaTable = require_table(lua, "core")?.get("active_view")?;
        let doc: LuaTable = av.get("doc")?;
        if let Some(info) = doc.get::<Option<LuaTable>>("indent_info")? {
            info.set("type", "soft")?;
            info.set("confirmed", true)?;
        }
        Ok(())
    })?;
    let spaces_cmds = lua.create_table()?;
    spaces_cmds.set("indent:switch-file-to-spaces-indentation", to_spaces_cmd)?;
    command.call_function::<()>("add", (to_spaces_pred, spaces_cmds))?;

    Ok(())
}

/// Detection runs in Rust via `doc_native.update_indent_info`; this preload registers commands.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.detectindent",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )?;
    Ok(())
}
