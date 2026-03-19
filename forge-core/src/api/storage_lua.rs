use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.storage`. Replaces `data/core/storage.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/storage.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.storage", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.storage").eval::<LuaValue>()
    })?)
}
