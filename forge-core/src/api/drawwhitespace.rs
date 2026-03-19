use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.drawwhitespace`. Replaces `data/plugins/drawwhitespace.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/drawwhitespace.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.drawwhitespace", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.drawwhitespace").eval::<LuaValue>()
    })?)
}
