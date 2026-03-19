use mlua::prelude::*;
/// Embedded Lua bootstrap for `core.modkeys-generic`. Replaces `data/core/modkeys-generic.lua`.
const BOOTSTRAP: &str = include_str!("../../../data/core/modkeys-generic.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.modkeys-generic", lua.create_function(|lua, ()| {
        lua.load(BOOTSTRAP).set_name("core.modkeys-generic").eval::<LuaValue>()
    })?)
}
