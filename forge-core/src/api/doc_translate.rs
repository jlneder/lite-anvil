use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.doc.translate`. Replaces `data/core/doc/translate.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/doc/translate.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.doc.translate", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.doc.translate").eval::<LuaValue>()
    })?)
}
