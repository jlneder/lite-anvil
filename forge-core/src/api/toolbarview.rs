use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.toolbarview`. Replaces `data/plugins/toolbarview.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/toolbarview.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.toolbarview", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.toolbarview").eval::<LuaValue>()
    })?)
}
