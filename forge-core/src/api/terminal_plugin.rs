use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.terminal`. Replaces `data/plugins/terminal/init.lua`.
const INIT_BOOTSTRAP: &str = include_str!("../../../data/plugins/terminal/init.lua");
/// Embedded Lua bootstrap for `plugins.terminal.colors`. Replaces `data/plugins/terminal/colors.lua`.
const COLORS_BOOTSTRAP: &str = include_str!("../../../data/plugins/terminal/colors.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.terminal", lua.create_function(|lua, ()| {
        lua.load(INIT_BOOTSTRAP).set_name("plugins.terminal").eval::<LuaValue>()
    })?)?;
    preload.set("plugins.terminal.colors", lua.create_function(|lua, ()| {
        lua.load(COLORS_BOOTSTRAP).set_name("plugins.terminal.colors").eval::<LuaValue>()
    })?)
}
