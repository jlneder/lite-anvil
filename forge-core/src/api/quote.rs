use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.quote`. Replaces `data/plugins/quote.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/quote.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.quote", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.quote").eval::<LuaValue>()
    })?)
}
