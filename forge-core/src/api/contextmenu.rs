use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.contextmenu`.
///
/// Owns right-click menu draw, item registration, and click routing.
/// Replaces `data/core/contextmenu.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/contextmenu.lua");

/// Register `core.contextmenu` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.contextmenu",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.contextmenu").eval::<LuaValue>()
        })?,
    )
}
