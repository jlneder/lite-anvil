use mlua::prelude::*;

/// Registers command-view keyboard commands (submit, complete, escape, select).
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    let cmds = lua.create_table()?;

    cmds.set(
        "command:submit",
        lua.create_function(|_lua, active_view: LuaTable| {
            active_view.call_method::<()>("submit", ())
        })?,
    )?;

    cmds.set(
        "command:complete",
        lua.create_function(|_lua, active_view: LuaTable| {
            active_view.call_method::<()>("complete", ())
        })?,
    )?;

    cmds.set(
        "command:escape",
        lua.create_function(|_lua, active_view: LuaTable| {
            active_view.call_method::<()>("exit", ())
        })?,
    )?;

    cmds.set(
        "command:select-previous",
        lua.create_function(|_lua, active_view: LuaTable| {
            active_view.call_method::<()>("move_suggestion_idx", 1)
        })?,
    )?;

    cmds.set(
        "command:select-next",
        lua.create_function(|_lua, active_view: LuaTable| {
            active_view.call_method::<()>("move_suggestion_idx", -1)
        })?,
    )?;

    add_fn.call::<()>(("core.commandview", cmds))?;
    Ok(())
}

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers the `core.commands.command` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.command",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
