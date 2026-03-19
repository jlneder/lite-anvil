use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.ime`. Replaces `data/core/ime.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/ime.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.ime", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.ime").eval::<LuaValue>()
    })?)
}
