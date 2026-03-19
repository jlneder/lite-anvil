use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.core`. Replaces `data/core/commands/core.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/core.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.core", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.core").eval::<LuaValue>()
    })?)
}
