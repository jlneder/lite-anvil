use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.remotessh`. Replaces `data/plugins/remotessh.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/remotessh.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.remotessh", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.remotessh").eval::<LuaValue>()
    })?)
}
