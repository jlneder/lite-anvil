use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.statusbar`. Replaces `data/core/commands/statusbar.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/statusbar.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.statusbar", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.statusbar").eval::<LuaValue>()
    })?)
}
