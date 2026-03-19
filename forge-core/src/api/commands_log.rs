use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.log`. Replaces `data/core/commands/log.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/log.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.log", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.log").eval::<LuaValue>()
    })?)
}
