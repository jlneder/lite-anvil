use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.files`. Replaces `data/core/commands/files.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/files.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.files", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.files").eval::<LuaValue>()
    })?)
}
