use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.scale`. Replaces `data/plugins/scale.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/plugins/scale.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.scale", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("plugins.scale").eval::<LuaValue>()
    })?)
}
