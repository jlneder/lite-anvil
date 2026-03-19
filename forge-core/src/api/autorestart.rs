use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.autorestart`. Replaces `data/plugins/autorestart.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/autorestart.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.autorestart", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.autorestart").eval::<LuaValue>()
    })?)
}
