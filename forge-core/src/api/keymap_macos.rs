use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.keymap-macos`. Replaces `data/core/keymap-macos.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/keymap-macos.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.keymap-macos", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.keymap-macos").eval::<LuaValue>()
    })?)
}
