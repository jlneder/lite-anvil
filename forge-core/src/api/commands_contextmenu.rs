use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.contextmenu`. Replaces `data/core/commands/contextmenu.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/contextmenu.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.contextmenu", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.contextmenu").eval::<LuaValue>()
    })?)
}
