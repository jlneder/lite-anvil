use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.dirwatch`. Replaces `data/core/dirwatch.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/dirwatch.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.dirwatch", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.dirwatch").eval::<LuaValue>()
    })?)
}
