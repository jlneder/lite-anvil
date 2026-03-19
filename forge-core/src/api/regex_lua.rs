use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.regex`. Replaces `data/core/regex.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/regex.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.regex", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.regex").eval::<LuaValue>()
    })?)
}
