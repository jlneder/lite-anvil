use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.view`.
///
/// Owns the base View class and scroll state. Replaces `data/core/view.lua`
/// which is no longer read from disk.
const BOOTSTRAP: &str = include_str!("../../../data/core/view.lua");

/// Register `core.view` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.view",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.view").eval::<LuaValue>()
        })?,
    )
}
