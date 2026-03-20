use mlua::prelude::*;

// Lua 5.4 source for the utf8extra module.
//
// Pattern-matching functions delegate to the Lua standard string library
// (which works correctly for the ASCII-dominated patterns used in the editor).
// All other functions implement the luautf8 API subset used by utf8string.lua.
const LUA_SRC: &str = include_str!("lua/utf8extra.lua");

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    lua.load(LUA_SRC).eval()
}
