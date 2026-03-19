use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.object`. Replaces `data/core/object.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/object.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.object", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.object").eval::<LuaValue>()
    })?)
}
