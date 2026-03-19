use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.logview`.
///
/// Owns log message buffer, filtering, auto-scroll, and copy.
/// Replaces `data/core/logview.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/logview.lua");

/// Register `core.logview` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.logview",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.logview").eval::<LuaValue>()
        })?,
    )
}
