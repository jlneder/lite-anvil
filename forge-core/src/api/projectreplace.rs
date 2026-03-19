use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.projectreplace`. Replaces `data/plugins/projectreplace.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/projectreplace.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.projectreplace", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.projectreplace").eval::<LuaValue>()
    })?)
}
