use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.lineguide`. Replaces `data/plugins/lineguide.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/lineguide.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.lineguide", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.lineguide").eval::<LuaValue>()
    })?)
}
