use mlua::prelude::*;

/// Registers `core.modkeys-macos` — modifier key name normalization for macOS.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.modkeys-macos",
        lua.create_function(|lua, ()| {
            let modkeys = lua.create_table()?;
            let map = lua.create_table()?;

            map.set("left command", "cmd")?;
            map.set("right command", "cmd")?;
            map.set("left ctrl", "ctrl")?;
            map.set("right ctrl", "ctrl")?;
            map.set("left shift", "shift")?;
            map.set("right shift", "shift")?;
            map.set("left option", "option")?;
            map.set("right option", "option")?;
            map.set("left alt", "alt")?;
            map.set("right alt", "altgr")?;

            modkeys.set("map", map)?;

            let keys = lua.create_table()?;
            for (i, key) in ["ctrl", "alt", "option", "altgr", "shift", "cmd"]
                .iter()
                .enumerate()
            {
                keys.set(i + 1, *key)?;
            }
            modkeys.set("keys", keys)?;

            Ok(LuaValue::Table(modkeys))
        })?,
    )
}
