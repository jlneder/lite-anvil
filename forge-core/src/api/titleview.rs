use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.titleview`. Replaces `data/core/titleview.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/titleview.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.titleview", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.titleview").eval::<LuaValue>()
    })?)
}
