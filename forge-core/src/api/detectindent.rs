use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.detectindent`. Replaces `data/plugins/detectindent.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/detectindent.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.detectindent", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.detectindent").eval::<LuaValue>()
    })?)
}
