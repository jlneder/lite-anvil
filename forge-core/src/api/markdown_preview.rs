use mlua::prelude::*;
/// Embedded Lua bootstrap for `plugins.markdown_preview`. Replaces `data/plugins/markdown_preview/init.lua`.
const INIT_BOOTSTRAP: &str = include_str!("../../../data/plugins/markdown_preview/init.lua");
/// Embedded Lua bootstrap for `plugins.markdown_preview.layout`. Replaces `data/plugins/markdown_preview/layout.lua`.
const LAYOUT_BOOTSTRAP: &str = include_str!("../../../data/plugins/markdown_preview/layout.lua");
/// Embedded Lua bootstrap for `plugins.markdown_preview.renderers`. Replaces `data/plugins/markdown_preview/renderers.lua`.
const RENDERERS_BOOTSTRAP: &str =
    include_str!("../../../data/plugins/markdown_preview/renderers.lua");
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("plugins.markdown_preview", lua.create_function(|lua, ()| {
        lua.load(INIT_BOOTSTRAP).set_name("plugins.markdown_preview").eval::<LuaValue>()
    })?)?;
    preload.set("plugins.markdown_preview.layout", lua.create_function(|lua, ()| {
        lua.load(LAYOUT_BOOTSTRAP).set_name("plugins.markdown_preview.layout").eval::<LuaValue>()
    })?)?;
    preload.set("plugins.markdown_preview.renderers", lua.create_function(|lua, ()| {
        lua.load(RENDERERS_BOOTSTRAP)
            .set_name("plugins.markdown_preview.renderers")
            .eval::<LuaValue>()
    })?)
}
