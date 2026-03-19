use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.nagview`.
///
/// Owns nag/dialog display, button layout, and response callbacks.
/// Replaces `data/core/nagview.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/nagview.lua");

/// Register `core.nagview` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.nagview",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.nagview").eval::<LuaValue>()
        })?,
    )
}
