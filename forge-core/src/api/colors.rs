use mlua::prelude::*;
/// Embedded Lua color themes. Replace `data/colors/*.lua`.
const DEFAULT: &str = include_str!("../../../data/colors/default.lua");
const DARK_DEFAULT: &str = include_str!("../../../data/colors/dark_default.lua");
const LIGHT_DEFAULT: &str = include_str!("../../../data/colors/light_default.lua");
const FALL: &str = include_str!("../../../data/colors/fall.lua");
const SUMMER: &str = include_str!("../../../data/colors/summer.lua");
const TEXTADEPT: &str = include_str!("../../../data/colors/textadept.lua");

pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("colors.default", lua.create_function(|lua, ()| {
        lua.load(DEFAULT).set_name("colors.default").eval::<LuaValue>()
    })?)?;
    preload.set("colors.dark_default", lua.create_function(|lua, ()| {
        lua.load(DARK_DEFAULT).set_name("colors.dark_default").eval::<LuaValue>()
    })?)?;
    preload.set("colors.light_default", lua.create_function(|lua, ()| {
        lua.load(LIGHT_DEFAULT).set_name("colors.light_default").eval::<LuaValue>()
    })?)?;
    preload.set("colors.fall", lua.create_function(|lua, ()| {
        lua.load(FALL).set_name("colors.fall").eval::<LuaValue>()
    })?)?;
    preload.set("colors.summer", lua.create_function(|lua, ()| {
        lua.load(SUMMER).set_name("colors.summer").eval::<LuaValue>()
    })?)?;
    preload.set("colors.textadept", lua.create_function(|lua, ()| {
        lua.load(TEXTADEPT).set_name("colors.textadept").eval::<LuaValue>()
    })?)
}
