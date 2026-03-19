use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.doc.search`. Replaces `data/core/doc/search.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/doc/search.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.doc.search", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.doc.search").eval::<LuaValue>()
    })?)
}
