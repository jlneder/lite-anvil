use mlua::prelude::*;

const CORE_SOURCE: &str = include_str!("lua/core.lua");

/// Registers all Rust-owned builtin preloads that bootstrap the Lua runtime.
pub fn register_builtin_preloads(lua: &Lua) -> LuaResult<()> {
    let package: LuaTable = lua.globals().get("package")?;
    let preload: LuaTable = package.get("preload")?;
    preload.set(
        "core",
        lua.create_function(|lua, ()| {
            lua.load(CORE_SOURCE).set_name("core").eval::<LuaValue>()
        })?,
    )?;
    Ok(())
}
