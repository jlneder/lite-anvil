use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.folding`. Replaces `data/plugins/folding.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/folding.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.folding", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.folding").eval::<LuaValue>()
    })?)
}
