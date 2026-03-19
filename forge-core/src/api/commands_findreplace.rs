use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.findreplace`. Replaces `data/core/commands/findreplace.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/findreplace.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.findreplace", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.findreplace").eval::<LuaValue>()
    })?)
}
