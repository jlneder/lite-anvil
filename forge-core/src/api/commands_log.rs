use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers log commands (open-as-doc, copy-to-clipboard).
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    let cmds = lua.create_table()?;

    cmds.set(
        "log:open-as-doc",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let doc: LuaTable = core.call_function("open_doc", "logs.txt")?;
            let root_view: LuaTable = core.get("root_view")?;
            root_view.call_method::<()>("open_doc", doc.clone())?;
            let log_text: String = core.call_function("get_log", ())?;
            doc.call_method::<()>("insert", (1, 1, log_text))?;
            doc.set("new_file", false)?;
            doc.call_method::<()>("clean", ())?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "log:copy-to-clipboard",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let system: LuaTable = lua.globals().get("system")?;
            let log_text: String = core.call_function("get_log", ())?;
            system.call_function::<()>("set_clipboard", log_text)?;
            Ok(())
        })?,
    )?;

    add_fn.call::<()>((LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers the `core.commands.log` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.log",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
