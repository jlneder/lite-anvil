use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.process`.
///
/// Wraps the native `process` module with stream helpers.
/// Replaces `data/core/process.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/process.lua");

/// Register `core.process` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.process",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.process").eval::<LuaValue>()
        })?,
    )
}
