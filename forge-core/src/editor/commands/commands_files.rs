use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers file-management commands (create-directory).
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    let cmds = lua.create_table()?;

    cmds.set(
        "files:create-directory",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;

            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function(move |lua, text: String| {
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let result: LuaMultiValue = common.call_function("mkdirp", text)?;
                    let mut vals = result.into_iter();
                    let success = vals.next().unwrap_or(LuaValue::Nil);
                    let err = vals.next().unwrap_or(LuaValue::Nil);
                    let path = vals.next().unwrap_or(LuaValue::Nil);
                    if !success.as_boolean().unwrap_or(false) {
                        let err_str = match &err {
                            LuaValue::String(s) => s
                                .to_str()
                                .map(|b| b.to_string())
                                .unwrap_or_else(|_| "unknown".to_string()),
                            _ => "unknown".to_string(),
                        };
                        let path_str = match &path {
                            LuaValue::String(s) => {
                                s.to_str().map(|b| b.to_string()).unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        core.call_function::<()>(
                            "error",
                            format!("cannot create directory {path_str:?}: {err_str}"),
                        )?;
                    }
                    Ok(())
                })?,
            )?;

            command_view.call_method::<()>("enter", ("New directory name", opts))?;
            Ok(())
        })?,
    )?;

    add_fn.call::<()>((LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers the `core.commands.files` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.files",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
