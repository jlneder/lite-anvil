use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.modkeys-macos`. Replaces `data/core/modkeys-macos.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/modkeys-macos.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.modkeys-macos", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.modkeys-macos").eval::<LuaValue>()
    })?)
}
