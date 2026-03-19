use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.language_md`. Replaces `data/plugins/language_md.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/language_md.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.language_md", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.language_md").eval::<LuaValue>()
    })?)
}
