use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.findfile`. Replaces `data/plugins/findfile.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/findfile.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.findfile", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.findfile").eval::<LuaValue>()
    })?)
}
