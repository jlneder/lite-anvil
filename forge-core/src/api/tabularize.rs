use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.tabularize`. Replaces `data/plugins/tabularize.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/tabularize.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.tabularize", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.tabularize").eval::<LuaValue>()
    })?)
}
