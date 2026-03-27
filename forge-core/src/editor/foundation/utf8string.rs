use mlua::prelude::*;

/// Registers `core.utf8string` — injects utf8 functions into the string table.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.utf8string",
        lua.create_function(|lua, ()| {
            let utf8: LuaTable = lua
                .globals()
                .get::<LuaTable>("package")?
                .get::<LuaTable>("loaded")?
                .get("utf8extra")?;
            let string_table: LuaTable = lua.globals().get("string")?;

            let mappings: &[(&str, &str)] = &[
                ("ubyte", "byte"),
                ("uchar", "char"),
                ("ufind", "find"),
                ("ugmatch", "gmatch"),
                ("ugsub", "gsub"),
                ("ulen", "len"),
                ("ulower", "lower"),
                ("umatch", "match"),
                ("ureverse", "reverse"),
                ("usub", "sub"),
                ("uupper", "upper"),
                ("uescape", "escape"),
                ("ucharpos", "charpos"),
                ("unext", "next"),
                ("uinsert", "insert"),
                ("uremove", "remove"),
                ("uwidth", "width"),
                ("uwidthindex", "widthindex"),
                ("utitle", "title"),
                ("ufold", "fold"),
                ("uncasecmp", "ncasecmp"),
                ("uoffset", "offset"),
                ("ucodepoint", "codepoint"),
                ("ucodes", "codes"),
            ];

            for &(string_key, utf8_key) in mappings {
                let val: LuaValue = utf8.get(utf8_key)?;
                string_table.set(string_key, val)?;
            }

            Ok(LuaValue::Boolean(true))
        })?,
    )
}
