use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.command`.
///
/// Owns the command registry and predicate dispatch.
/// Replaces `data/core/command.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/command.lua");

/// Register `core.command` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.command",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.command").eval::<LuaValue>()
        })?,
    )
}
