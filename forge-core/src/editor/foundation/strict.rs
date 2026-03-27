use mlua::prelude::*;

/// Registers `core.strict` — sets a metatable on `_G` that errors on undefined globals.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.strict",
        lua.create_function(|lua, ()| {
            let globals = lua.globals();
            let defined = lua.create_table()?;

            let defined_ref = lua.create_registry_value(defined.clone())?;

            let global_fn = lua.create_function({
                let defined_ref = lua.create_registry_value(defined.clone())?;
                move |lua, t: LuaTable| {
                    let defined: LuaTable = lua.registry_value(&defined_ref)?;
                    for pair in t.pairs::<LuaValue, LuaValue>() {
                        let (k, v) = pair?;
                        defined.set(k.clone(), true)?;
                        lua.globals().raw_set(k, v)?;
                    }
                    Ok(())
                }
            })?;
            globals.raw_set("global", global_fn)?;

            let mt = lua.create_table()?;

            mt.set(
                "__newindex",
                lua.create_function(|_lua, (_t, k, _v): (LuaTable, LuaValue, LuaValue)| {
                    let name = match &k {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => format!("{k:?}"),
                    };
                    Err::<(), _>(LuaError::runtime(format!(
                        "cannot set undefined variable: {name}"
                    )))
                })?,
            )?;

            mt.set(
                "__index",
                lua.create_function(move |lua, (_t, k): (LuaTable, LuaValue)| {
                    let defined: LuaTable = lua.registry_value(&defined_ref)?;
                    let is_defined: bool = defined.get(k.clone()).unwrap_or(false);
                    if !is_defined {
                        let name = match &k {
                            LuaValue::String(s) => s.to_str()?.to_string(),
                            _ => format!("{k:?}"),
                        };
                        return Err(LuaError::runtime(format!(
                            "cannot get undefined variable: {name}"
                        )));
                    }
                    Ok(LuaNil)
                })?,
            )?;

            globals.set_metatable(Some(mt))?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
