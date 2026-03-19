use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.git`. Replaces `data/plugins/git/init.lua`.
const INIT_BOOTSTRAP: &str = include_str!("../../../data/plugins/git/init.lua");
/// Embedded Lua bootstrap for `plugins.git.status`. Replaces `data/plugins/git/status.lua`.
const STATUS_BOOTSTRAP: &str = include_str!("../../../data/plugins/git/status.lua");
/// Embedded Lua bootstrap for `plugins.git.ui`. Replaces `data/plugins/git/ui.lua`.
const UI_BOOTSTRAP: &str = include_str!("../../../data/plugins/git/ui.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.git", lua.create_function(|lua, ()| {
        lua.load(INIT_BOOTSTRAP).set_name("plugins.git").eval::<LuaValue>()
    })?)?;
    preload.set("plugins.git.status", lua.create_function(|lua, ()| {
        lua.load(STATUS_BOOTSTRAP).set_name("plugins.git.status").eval::<LuaValue>()
    })?)?;
    preload.set("plugins.git.ui", lua.create_function(|lua, ()| {
        lua.load(UI_BOOTSTRAP).set_name("plugins.git.ui").eval::<LuaValue>()
    })?)
}
