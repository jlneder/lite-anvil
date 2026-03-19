use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.theme_toggle`. Replaces `data/plugins/theme_toggle.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/theme_toggle.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.theme_toggle", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.theme_toggle").eval::<LuaValue>()
    })?)
}
