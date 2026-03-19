use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.reflow`. Replaces `data/plugins/reflow.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/reflow.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.reflow", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.reflow").eval::<LuaValue>()
    })?)
}
