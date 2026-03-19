use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.emptyview`. Replaces `data/core/emptyview.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/emptyview.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.emptyview", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.emptyview").eval::<LuaValue>()
    })?)
}
