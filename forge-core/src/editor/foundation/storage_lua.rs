use mlua::prelude::*;

/// Registers `core.storage` — persistent storage with load/save/keys/clear.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.storage",
        lua.create_function(|lua, ()| {
            let storage = lua.create_table()?;

            // storage.load(module, key) -> value or nil
            storage.set(
                "load",
                lua.create_function(|lua, (module, key): (LuaString, LuaString)| {
                    let native: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("storage_native")?;
                    let load_text: LuaFunction = native.get("load_text")?;
                    let result: LuaResult<LuaValue> = load_text.call((module.clone(), key.clone()));
                    match result {
                        Ok(LuaValue::Nil) => Ok(LuaNil),
                        Ok(LuaValue::String(text)) => {
                            let text_str = text.to_str()?.to_string();
                            // Check if it already starts with "return"
                            let chunk = if text_str.trim_start().starts_with("return") {
                                text_str.clone()
                            } else {
                                format!("return {text_str}")
                            };
                            let name = format!("@storage[{}:{}]", module.to_str()?, key.to_str()?);
                            match lua.load(&chunk).set_name(&name).eval::<LuaValue>() {
                                Ok(val) => Ok(val),
                                Err(e) => {
                                    let core: LuaTable = lua
                                        .globals()
                                        .get::<LuaTable>("package")?
                                        .get::<LuaTable>("loaded")?
                                        .get("core")?;
                                    let error_fn: LuaFunction = core.get("error")?;
                                    error_fn.call::<LuaValue>((
                                        "error decoding storage file for %s[%s]: %s",
                                        module.clone(),
                                        key.clone(),
                                        e.to_string(),
                                    ))?;
                                    Ok(LuaNil)
                                }
                            }
                        }
                        Ok(other) => Ok(other),
                        Err(e) => {
                            let core: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("core")?;
                            let error_fn: LuaFunction = core.get("error")?;
                            error_fn.call::<LuaValue>((
                                "error loading storage file for %s[%s]: %s",
                                module.clone(),
                                key.clone(),
                                e.to_string(),
                            ))?;
                            Ok(LuaNil)
                        }
                    }
                })?,
            )?;

            // storage.save(module, key, value)
            storage.set(
                "save",
                lua.create_function(
                    |lua, (module, key, value): (LuaString, LuaString, LuaValue)| {
                        let common: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("core.common")?;
                        let serialize: LuaFunction = common.get("serialize")?;
                        let serialized: LuaString = serialize.call(value)?;

                        let native: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("storage_native")?;
                        let save_text: LuaFunction = native.get("save_text")?;
                        let result: LuaResult<()> =
                            save_text.call((module.clone(), key.clone(), serialized));
                        if let Err(e) = result {
                            let core: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("core")?;
                            let error_fn: LuaFunction = core.get("error")?;
                            error_fn.call::<LuaValue>((
                                "error opening storage file for writing: %s",
                                e.to_string(),
                            ))?;
                        }
                        Ok(())
                    },
                )?,
            )?;

            // storage.keys(module) -> table
            storage.set(
                "keys",
                lua.create_function(|lua, module: LuaString| {
                    let native: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("storage_native")?;
                    let keys_fn: LuaFunction = native.get("keys")?;
                    let result: LuaResult<LuaTable> = keys_fn.call(module);
                    match result {
                        Ok(t) => Ok(t),
                        Err(_) => lua.create_table(),
                    }
                })?,
            )?;

            // storage.clear(module, key?)
            storage.set(
                "clear",
                lua.create_function(|lua, (module, key): (LuaString, Option<LuaString>)| {
                    let native: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("storage_native")?;
                    let clear_fn: LuaFunction = native.get("clear")?;
                    let result: LuaResult<()> = clear_fn.call((module.clone(), key.clone()));
                    if let Err(e) = result {
                        let core: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("core")?;
                        let error_fn: LuaFunction = core.get("error")?;
                        error_fn
                            .call::<LuaValue>(("error clearing storage file: %s", e.to_string()))?;
                    }
                    Ok(())
                })?,
            )?;

            Ok(LuaValue::Table(storage))
        })?,
    )
}
