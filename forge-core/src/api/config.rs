use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.config`.
///
/// Owns all default config values and the `config.plugins` metatable.
/// Replaces `data/core/config.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/config.lua");

/// Register `core.config` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.config",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.config").eval::<LuaValue>()
        })?,
    )
}
