use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.command`. Replaces `data/core/commands/command.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/command.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.command", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.command").eval::<LuaValue>()
    })?)
}
