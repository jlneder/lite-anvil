use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.gitignore`. Replaces `data/core/gitignore.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/gitignore.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.gitignore", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.gitignore").eval::<LuaValue>()
    })?)
}
