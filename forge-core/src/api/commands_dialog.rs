use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.dialog`. Replaces `data/core/commands/dialog.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/dialog.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.dialog", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.dialog").eval::<LuaValue>()
    })?)
}
