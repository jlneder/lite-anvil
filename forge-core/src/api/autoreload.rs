use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.autoreload`. Replaces `data/plugins/autoreload.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/autoreload.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.autoreload", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.autoreload").eval::<LuaValue>()
    })?)
}
