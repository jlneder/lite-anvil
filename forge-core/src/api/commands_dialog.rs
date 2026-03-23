use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers dialog (nag-view) commands: select-initial, navigation, yes/no/select.
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    let cmds = lua.create_table()?;

    cmds.set(
        "dialog:select-initial",
        lua.create_function(|lua, (v, ch): (LuaTable, LuaValue)| {
            let core: LuaTable = require_table(lua, "core")?;
            let nag_view: LuaTable = core.get("nag_view")?;

            // v must be core.nag_view and ch must be a single-char string
            if !v.equals(&nag_view)? {
                return Ok(LuaValue::Nil);
            }
            let ch_str = match &ch {
                LuaValue::String(s) => {
                    let s = s.to_str()?.to_string();
                    if s.len() != 1 {
                        return Ok(LuaValue::Nil);
                    }
                    s
                }
                _ => return Ok(LuaValue::Nil),
            };

            let lower = ch_str.to_lowercase();
            let options: LuaTable = v.get("options")?;
            let mut matched: Option<usize> = None;

            for i in 1..=options.raw_len() {
                let option: LuaTable = options.get(i)?;
                let text: String = option.get("text").unwrap_or_default();
                let initial = text.chars().next().map(|c| c.to_lowercase().to_string());
                if initial.as_deref() == Some(lower.as_str()) {
                    if matched.is_some() {
                        matched = None;
                        break;
                    }
                    matched = Some(i);
                }
            }

            if let Some(idx) = matched {
                let command: LuaTable = require_table(lua, "core.command")?;
                v.call_method::<()>("change_hovered", idx)?;
                command.call_function::<()>("perform", "dialog:select")?;
                return Ok(LuaValue::Boolean(true));
            }
            Ok(LuaValue::Nil)
        })?,
    )?;

    cmds.set(
        "dialog:previous-entry",
        lua.create_function(|_lua, v: LuaTable| {
            let hover: i64 = v.get::<Option<i64>>("hovered_item")?.unwrap_or(1);
            let options: LuaTable = v.get("options")?;
            let len = options.raw_len() as i64;
            let new_hover = if hover == 1 { len } else { hover - 1 };
            v.call_method::<()>("change_hovered", new_hover)?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "dialog:next-entry",
        lua.create_function(|_lua, v: LuaTable| {
            let hover: i64 = v.get::<Option<i64>>("hovered_item")?.unwrap_or(1);
            let options: LuaTable = v.get("options")?;
            let len = options.raw_len() as i64;
            let new_hover = if hover == len { 1 } else { hover + 1 };
            v.call_method::<()>("change_hovered", new_hover)?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "dialog:select-yes",
        lua.create_function(|lua, v: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let nag_view: LuaTable = core.get("nag_view")?;
            if !v.equals(&nag_view)? {
                return Ok(());
            }
            let common: LuaTable = require_table(lua, "core.common")?;
            let options: LuaTable = v.get("options")?;
            let idx: LuaValue = common.call_function("find_index", (options, "default_yes"))?;
            v.call_method::<()>("change_hovered", idx)?;
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", "dialog:select")?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "dialog:select-no",
        lua.create_function(|lua, v: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let nag_view: LuaTable = core.get("nag_view")?;
            if !v.equals(&nag_view)? {
                return Ok(());
            }
            let common: LuaTable = require_table(lua, "core.common")?;
            let options: LuaTable = v.get("options")?;
            let idx: LuaValue = common.call_function("find_index", (options, "default_no"))?;
            v.call_method::<()>("change_hovered", idx)?;
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", "dialog:select")?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "dialog:select",
        lua.create_function(|_lua, v: LuaTable| {
            let hovered: Option<i64> = v.get("hovered_item")?;
            if let Some(idx) = hovered {
                let options: LuaTable = v.get("options")?;
                let option: LuaValue = options.get(idx)?;
                let on_selected: LuaFunction = v.get("on_selected")?;
                on_selected.call::<()>(option)?;
                v.call_method::<()>("next", ())?;
            }
            Ok(())
        })?,
    )?;

    add_fn.call::<()>(("core.nagview", cmds))?;
    Ok(())
}

/// Registers the `core.commands.dialog` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.dialog",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
