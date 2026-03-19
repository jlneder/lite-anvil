use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.autocomplete`. Replaces `data/plugins/autocomplete/init.lua`.
const INIT_BOOTSTRAP: &str = include_str!("../../../data/plugins/autocomplete/init.lua");
/// Embedded Lua bootstrap for `plugins.autocomplete.drawing`. Replaces `data/plugins/autocomplete/drawing.lua`.
const DRAWING_BOOTSTRAP: &str = include_str!("../../../data/plugins/autocomplete/drawing.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.autocomplete", lua.create_function(|lua, ()| {
        lua.load(INIT_BOOTSTRAP).set_name("plugins.autocomplete").eval::<LuaValue>()
    })?)?;
    preload.set("plugins.autocomplete.drawing", lua.create_function(|lua, ()| {
        lua.load(DRAWING_BOOTSTRAP).set_name("plugins.autocomplete.drawing").eval::<LuaValue>()
    })?)
}
