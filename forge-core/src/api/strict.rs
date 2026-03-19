use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.strict`. Replaces `data/core/strict.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/strict.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.strict", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.strict").eval::<LuaValue>()
    })?)
}
