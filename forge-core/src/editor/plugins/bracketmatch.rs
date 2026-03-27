use mlua::prelude::*;

/// Bracket matching is implemented directly in `docview.rs`. This preload is a no-op kept
/// so that any `require "plugins.bracketmatch"` calls from user configs do not error.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.bracketmatch",
        lua.create_function(|_, ()| Ok(LuaValue::Boolean(true)))?,
    )?;
    Ok(())
}
