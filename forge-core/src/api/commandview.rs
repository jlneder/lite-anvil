use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commandview`. Replaces `data/core/commandview.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commandview.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commandview", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commandview").eval::<LuaValue>()
    })?)
}
