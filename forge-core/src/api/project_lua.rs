use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.project`. Replaces `data/core/project.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/project.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.project", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.project").eval::<LuaValue>()
    })?)
}
