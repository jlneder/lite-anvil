use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.commands.root`. Replaces `data/core/commands/root.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/commands/root.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.commands.root", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.commands.root").eval::<LuaValue>()
    })?)
}
