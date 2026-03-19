use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.scrollbar`.
///
/// Owns scrollbar geometry, thumb tracking, and drag state.
/// Replaces `data/core/scrollbar.lua` which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/scrollbar.lua");

/// Register `core.scrollbar` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.scrollbar",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.scrollbar").eval::<LuaValue>()
        })?,
    )
}
