use mlua::prelude::*;

/// Registers `core.modkeys-generic` — modifier key name normalization for non-macOS.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.modkeys-generic",
        lua.create_function(|lua, ()| {
            let modkeys = lua.create_table()?;
            let map = lua.create_table()?;

            map.set("left ctrl", "ctrl")?;
            map.set("right ctrl", "ctrl")?;
            map.set("left shift", "shift")?;
            map.set("right shift", "shift")?;
            map.set("left alt", "alt")?;
            map.set("right alt", "altgr")?;
            map.set("left gui", "super")?;
            map.set("left windows", "super")?;
            map.set("right gui", "super")?;
            map.set("right windows", "super")?;

            modkeys.set("map", map)?;

            let keys = lua.create_table()?;
            for (i, key) in ["ctrl", "shift", "alt", "altgr", "super"]
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
