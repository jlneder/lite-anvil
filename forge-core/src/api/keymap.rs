use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.keymap`.
///
/// Owns keybinding tables and dispatch. Replaces `data/core/keymap.lua`
/// which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/keymap.lua");

/// Register `core.keymap` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.keymap",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.keymap").eval::<LuaValue>()
        })?,
    )
}
