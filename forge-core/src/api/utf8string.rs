use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.utf8string`. Replaces `data/core/utf8string.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/utf8string.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.utf8string", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.utf8string").eval::<LuaValue>()
    })?)
}
