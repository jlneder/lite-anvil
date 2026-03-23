use mlua::prelude::*;

/// Registers `core.ime` — Input Method Editor event handling.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.ime",
        lua.create_function(|lua, ()| {
            let ime = lua.create_table()?;

            // ime.reset()
            let ime_ref = lua.create_registry_value(ime.clone())?;
            ime.set(
                "reset",
                lua.create_function({
                    let ime_ref = lua.create_registry_value(ime.clone())?;
                    move |lua, ()| {
                        let ime: LuaTable = lua.registry_value(&ime_ref)?;
                        ime.set("editing", false)?;
                        let loc = lua.create_table()?;
                        loc.set("x", 0)?;
                        loc.set("y", 0)?;
                        loc.set("w", 0)?;
                        loc.set("h", 0)?;
                        ime.set("last_location", loc)?;
                        Ok(())
                    }
                })?,
            )?;

            // ime.ingest(text, start, length) -> text, start_byte, length_bytes
            ime.set(
                "ingest",
                lua.create_function({
                    let ime_ref = lua.create_registry_value(ime.clone())?;
                    move |lua, (text, start, length): (LuaString, i64, i64)| {
                        let ime: LuaTable = lua.registry_value(&ime_ref)?;
                        let text_str = text.to_str()?.to_string();

                        if text_str.is_empty() {
                            let reset_fn: LuaFunction = ime.get("reset")?;
                            reset_fn.call::<()>(())?;
                            return Ok((lua.create_string("")?, 0i64, 0i64));
                        }

                        ime.set("editing", true)?;

                        if start < 0 {
                            let len = text_str.len() as i64;
                            return Ok((text, len, 0));
                        }

                        // Convert utf-8 codepoint offset to byte offset
                        let utf8_mod: LuaTable = lua.globals().get("utf8")?;
                        let offset_fn: LuaFunction = utf8_mod.get("offset")?;
                        let start_byte_result: LuaValue =
                            offset_fn.call((text.clone(), start + 1))?;

                        let text_len = text_str.len() as i64;
                        let start_byte = match start_byte_result {
                            LuaValue::Integer(n) => (n - 1).min(text_len),
                            LuaValue::Number(n) => ((n as i64) - 1).min(text_len),
                            _ => text_len,
                        };

                        if length < 0 {
                            return Ok((text, start_byte, 0));
                        }

                        let end_byte_result: LuaValue =
                            offset_fn.call((text.clone(), start + length + 1))?;
                        let end_byte = match end_byte_result {
                            LuaValue::Integer(n) => {
                                let v = n - 1;
                                if v < start_byte {
                                    return Ok((text, start_byte, 0));
                                }
                                v.min(text_len)
                            }
                            LuaValue::Number(n) => {
                                let v = (n as i64) - 1;
                                if v < start_byte {
                                    return Ok((text, start_byte, 0));
                                }
                                v.min(text_len)
                            }
                            _ => return Ok((text, start_byte, 0)),
                        };

                        Ok((text, start_byte, end_byte - start_byte))
                    }
                })?,
            )?;

            // ime.on_text_editing(text, start, length, ...)
            ime.set(
                "on_text_editing",
                lua.create_function({
                    let ime_ref = lua.create_registry_value(ime.clone())?;
                    move |lua, args: LuaMultiValue| {
                        let ime: LuaTable = lua.registry_value(&ime_ref)?;
                        let vals: Vec<LuaValue> = args.into_vec();
                        let text = match vals.first() {
                            Some(LuaValue::String(s)) => s.clone(),
                            _ => return Ok(()),
                        };
                        let editing: bool = ime.get("editing").unwrap_or(false);
                        let text_len = text.to_str()?.len();
                        if editing || text_len > 0 {
                            let ingest_fn: LuaFunction = ime.get("ingest")?;
                            let result: LuaMultiValue =
                                ingest_fn.call(LuaMultiValue::from_vec(vals))?;
                            let core: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("core")?;
                            let rv: LuaTable = core.get("root_view")?;
                            rv.call_method::<()>("on_ime_text_editing", result)?;
                        }
                        Ok(())
                    }
                })?,
            )?;

            // ime.stop()
            ime.set(
                "stop",
                lua.create_function({
                    let ime_ref = lua.create_registry_value(ime.clone())?;
                    move |lua, ()| {
                        let ime: LuaTable = lua.registry_value(&ime_ref)?;
                        let editing: bool = ime.get("editing").unwrap_or(false);
                        if editing {
                            let system: LuaTable = lua.globals().get("system")?;
                            let clear_ime: LuaFunction = system.get("clear_ime")?;
                            let core: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("core")?;
                            let window: LuaValue = core.get("window")?;
                            clear_ime.call::<()>(window)?;
                            let on_text_editing: LuaFunction = ime.get("on_text_editing")?;
                            on_text_editing.call::<()>((lua.create_string("")?, 0, 0))?;
                        }
                        Ok(())
                    }
                })?,
            )?;

            // ime.set_location(x, y, w, h)
            ime.set(
                "set_location",
                lua.create_function({
                    let ime_ref = lua.create_registry_value(ime.clone())?;
                    move |lua, (x, y, w, h): (f64, f64, f64, f64)| {
                        let ime: LuaTable = lua.registry_value(&ime_ref)?;
                        let last_loc: LuaValue = ime.get("last_location")?;
                        let need_update = match &last_loc {
                            LuaValue::Table(loc) => {
                                let lx: f64 = loc.get("x").unwrap_or(0.0);
                                let ly: f64 = loc.get("y").unwrap_or(0.0);
                                let lw: f64 = loc.get("w").unwrap_or(0.0);
                                let lh: f64 = loc.get("h").unwrap_or(0.0);
                                lx != x || ly != y || lw != w || lh != h
                            }
                            _ => true,
                        };
                        if need_update {
                            let loc = match last_loc {
                                LuaValue::Table(t) => t,
                                _ => {
                                    let t = lua.create_table()?;
                                    ime.set("last_location", t.clone())?;
                                    t
                                }
                            };
                            loc.set("x", x)?;
                            loc.set("y", y)?;
                            loc.set("w", w)?;
                            loc.set("h", h)?;
                            let system: LuaTable = lua.globals().get("system")?;
                            let set_rect: LuaFunction = system.get("set_text_input_rect")?;
                            let core: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("core")?;
                            let window: LuaValue = core.get("window")?;
                            set_rect.call::<()>((window, x, y, w, h))?;
                        }
                        Ok(())
                    }
                })?,
            )?;

            // Initialize: call reset
            let reset_fn: LuaFunction = ime.get("reset")?;
            reset_fn.call::<()>(())?;

            let _ = ime_ref;
            Ok(LuaValue::Table(ime))
        })?,
    )
}
