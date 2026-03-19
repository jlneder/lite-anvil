use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.bracketmatch`. Replaces `data/plugins/bracketmatch.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/bracketmatch.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.bracketmatch", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.bracketmatch").eval::<LuaValue>()
    })?)
}
