use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.projectsearch`. Replaces `data/plugins/projectsearch.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/projectsearch.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.projectsearch", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.projectsearch").eval::<LuaValue>()
    })?)
}
