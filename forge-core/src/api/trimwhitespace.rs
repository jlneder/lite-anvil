use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.trimwhitespace`. Replaces `data/plugins/trimwhitespace.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/trimwhitespace.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.trimwhitespace", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.trimwhitespace").eval::<LuaValue>()
    })?)
}
