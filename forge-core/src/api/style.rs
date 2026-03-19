use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.style`.
///
/// Owns the color palette, font loading, and theme application logic.
/// Replaces `data/core/style.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/style.lua");

/// Register `core.style` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.style",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.style").eval::<LuaValue>()
        })?,
    )
}
