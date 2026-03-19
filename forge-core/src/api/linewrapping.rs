use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.linewrapping`. Replaces `data/plugins/linewrapping.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/linewrapping.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.linewrapping", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.linewrapping").eval::<LuaValue>()
    })?)
}
