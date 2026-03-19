use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.common`. Replaces `data/core/common.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/common.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.common", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.common").eval::<LuaValue>()
    })?)
}
