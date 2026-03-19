use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.plugin_api`. Replaces `data/core/plugin_api.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/plugin_api.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.plugin_api", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.plugin_api").eval::<LuaValue>()
    })?)
}
