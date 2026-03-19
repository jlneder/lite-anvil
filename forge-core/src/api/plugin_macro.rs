use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.macro`. Replaces `data/plugins/macro.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/macro.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.macro", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.macro").eval::<LuaValue>()
    })?)
}
