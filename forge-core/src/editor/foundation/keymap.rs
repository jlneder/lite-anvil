use mlua::prelude::*;

/// Registers `core.keymap` — keybinding tables, stroke normalization, and input dispatch.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.keymap",
        lua.create_function(|lua, ()| {
            let platform: String = lua.globals().get("PLATFORM")?;
            let macos = platform == "Mac OS X";

            let keymap = lua.create_table()?;
            keymap.set("modkeys", lua.create_table()?)?;
            keymap.set("map", lua.create_table()?)?;
            keymap.set("reverse_map", lua.create_table()?)?;

            let modkeys_mod: LuaTable = {
                let require: LuaFunction = lua.globals().get("require")?;
                let mod_name = if macos {
                    "core.modkeys-macos"
                } else {
                    "core.modkeys-generic"
                };
                require.call(mod_name)?
            };
            let modkey_map: LuaTable = modkeys_mod.get("map")?;
            let modkeys_list: LuaTable = modkeys_mod.get("keys")?;

            let display_names = lua.create_table()?;
            display_names.set("ctrl", if macos { "Command" } else { "Ctrl" })?;
            display_names.set("cmd", "Command")?;
            display_names.set("alt", "Alt")?;
            display_names.set("altgr", "AltGr")?;
            display_names.set("option", "Option")?;
            display_names.set("shift", "Shift")?;
            display_names.set("super", "Super")?;

            // Build normalize_stroke as a Lua function
            let normalize_stroke_fn = lua.create_function({
                let modkeys_ref = lua.create_registry_value(modkeys_list.clone())?;
                move |lua, stroke: String| {
                    let modkeys: LuaTable = lua.registry_value(&modkeys_ref)?;
                    normalize_stroke(&stroke, &modkeys)
                }
            })?;

            // Build macos_cmd_alias as a Lua function (or nil)
            let macos_cmd_alias_fn: LuaValue = if macos {
                LuaValue::Function(lua.create_function({
                    let norm_ref = lua.create_registry_value(normalize_stroke_fn.clone())?;
                    move |lua, stroke: String| -> LuaResult<LuaValue> {
                        let parts = split_stroke(&stroke);
                        let has_ctrl = parts.iter().any(|p| p == "ctrl");
                        let has_cmd = parts.iter().any(|p| p == "cmd");
                        if !has_ctrl || has_cmd {
                            return Ok(LuaValue::Nil);
                        }
                        let new_parts: Vec<&str> = parts
                            .iter()
                            .map(|p| if p == "ctrl" { "cmd" } else { p.as_str() })
                            .collect();
                        let joined = new_parts.join("+");
                        let normalize: LuaFunction = lua.registry_value(&norm_ref)?;
                        normalize.call(joined)
                    }
                })?)
            } else {
                LuaValue::Nil
            };

            // Build with_macos_aliases as a Lua function
            let with_macos_aliases_fn = lua.create_function({
                let alias_ref = lua.create_registry_value(macos_cmd_alias_fn)?;
                move |lua, map: LuaTable| -> LuaResult<LuaTable> {
                    let alias_val: LuaValue = lua.registry_value(&alias_ref)?;
                    let alias_fn = match alias_val {
                        LuaValue::Function(f) => f,
                        _ => return Ok(map),
                    };
                    let expanded = lua.create_table()?;
                    for pair in map.pairs::<String, LuaValue>() {
                        let (stroke, commands) = pair?;
                        expanded.set(stroke.clone(), commands.clone())?;
                        let alias: LuaValue = alias_fn.call(stroke)?;
                        if let LuaValue::String(alias_str) = alias {
                            let alias_s = alias_str.to_str()?.to_string();
                            let existing: LuaValue = map.get(alias_s.clone())?;
                            if existing == LuaValue::Nil {
                                expanded.set(alias_s, commands)?;
                            }
                        }
                    }
                    Ok(expanded)
                }
            })?;

            // Build remove_only as a Lua function
            let remove_only_fn =
                lua.create_function(|_lua, (tbl, k, v): (LuaTable, String, LuaValue)| {
                    let arr: LuaValue = tbl.get(k.clone())?;
                    if let LuaValue::Table(arr_tbl) = arr {
                        if let LuaValue::String(v_str) = v {
                            let v_s = v_str.to_str()?.to_string();
                            let mut j = 0usize;
                            let len = arr_tbl.raw_len();
                            for i in 1..=len {
                                while i + j <= len {
                                    let val: LuaValue = arr_tbl.get(i + j)?;
                                    if let LuaValue::String(ref s) = val {
                                        if s.to_str()? == v_s {
                                            j += 1;
                                            continue;
                                        }
                                    }
                                    break;
                                }
                                if i + j <= len {
                                    let val: LuaValue = arr_tbl.get(i + j)?;
                                    arr_tbl.set(i, val)?;
                                } else {
                                    arr_tbl.set(i, LuaValue::Nil)?;
                                }
                            }
                        } else {
                            tbl.set(k, LuaValue::Nil)?;
                        }
                    }
                    Ok(())
                })?;

            // Build remove_duplicates as a Lua function
            let remove_duplicates_fn = lua.create_function({
                let km_ref = lua.create_registry_value(keymap.clone())?;
                let norm_ref = lua.create_registry_value(normalize_stroke_fn.clone())?;
                move |lua, map: LuaTable| -> LuaResult<()> {
                    let km: LuaTable = lua.registry_value(&km_ref)?;
                    let km_map: LuaTable = km.get("map")?;
                    let normalize: LuaFunction = lua.registry_value(&norm_ref)?;

                    for pair in map.clone().pairs::<String, LuaValue>() {
                        let (stroke, commands_val) = pair?;
                        let normalized: String = normalize.call(stroke.clone())?;
                        let commands = ensure_table(lua, commands_val)?;

                        let existing: LuaValue = km_map.get(normalized)?;
                        if let LuaValue::Table(existing_tbl) = existing {
                            for epair in existing_tbl.pairs::<LuaInteger, String>() {
                                let (_, registered_cmd) = epair?;
                                let mut j = 0usize;
                                let len = commands.raw_len();
                                for i in 1..=len {
                                    while i + j <= len {
                                        let val: LuaValue = commands.get(i + j)?;
                                        if let LuaValue::String(ref s) = val {
                                            if s.to_str()? == registered_cmd {
                                                j += 1;
                                                continue;
                                            }
                                        }
                                        break;
                                    }
                                    if i + j <= len {
                                        let val: LuaValue = commands.get(i + j)?;
                                        commands.set(i, val)?;
                                    } else {
                                        commands.set(i, LuaValue::Nil)?;
                                    }
                                }
                            }
                        }
                        if commands.raw_len() < 1 {
                            map.set(stroke, LuaValue::Nil)?;
                        } else {
                            map.set(stroke, commands)?;
                        }
                    }
                    Ok(())
                }
            })?;

            // keymap.add_direct(map)
            keymap.set(
                "add_direct",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    let norm_ref = lua.create_registry_value(normalize_stroke_fn.clone())?;
                    let aliases_ref = lua.create_registry_value(with_macos_aliases_fn.clone())?;
                    let ro_ref = lua.create_registry_value(remove_only_fn.clone())?;
                    move |lua, map: LuaTable| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let km_map: LuaTable = km.get("map")?;
                        let reverse_map: LuaTable = km.get("reverse_map")?;
                        let normalize: LuaFunction = lua.registry_value(&norm_ref)?;
                        let with_aliases: LuaFunction = lua.registry_value(&aliases_ref)?;
                        let remove_only: LuaFunction = lua.registry_value(&ro_ref)?;

                        let map: LuaTable = with_aliases.call(map)?;

                        for pair in map.pairs::<String, LuaValue>() {
                            let (stroke, commands_val) = pair?;
                            let stroke: String = normalize.call(stroke)?;
                            let commands = ensure_table(lua, commands_val)?;

                            let existing: LuaValue = km_map.get(stroke.clone())?;
                            if let LuaValue::Table(existing_tbl) = existing {
                                for epair in existing_tbl.pairs::<LuaInteger, LuaValue>() {
                                    let (_, cmd) = epair?;
                                    remove_only.call::<()>((
                                        reverse_map.clone(),
                                        cmd,
                                        stroke.clone(),
                                    ))?;
                                }
                            }

                            km_map.set(stroke.clone(), commands.clone())?;

                            for cpair in commands.pairs::<LuaInteger, LuaValue>() {
                                let (_, cmd) = cpair?;
                                let cmd_key = match &cmd {
                                    LuaValue::String(s) => s.to_str()?.to_string(),
                                    _ => continue,
                                };
                                let rev: LuaValue = reverse_map.get(cmd_key.clone())?;
                                let rev_tbl = match rev {
                                    LuaValue::Table(t) => t,
                                    _ => {
                                        let t = lua.create_table()?;
                                        reverse_map.set(cmd_key, t.clone())?;
                                        t
                                    }
                                };
                                let next_idx = rev_tbl.raw_len() + 1;
                                rev_tbl.set(next_idx, stroke.clone())?;
                            }
                        }
                        Ok(())
                    }
                })?,
            )?;

            // keymap.add(map, overwrite?)
            keymap.set(
                "add",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    let norm_ref = lua.create_registry_value(normalize_stroke_fn.clone())?;
                    let aliases_ref = lua.create_registry_value(with_macos_aliases_fn.clone())?;
                    let ro_ref = lua.create_registry_value(remove_only_fn.clone())?;
                    let rd_ref = lua.create_registry_value(remove_duplicates_fn.clone())?;
                    move |lua, (map, overwrite): (LuaTable, Option<bool>)| {
                        let overwrite = overwrite.unwrap_or(false);
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let km_map: LuaTable = km.get("map")?;
                        let reverse_map: LuaTable = km.get("reverse_map")?;
                        let normalize: LuaFunction = lua.registry_value(&norm_ref)?;
                        let with_aliases: LuaFunction = lua.registry_value(&aliases_ref)?;
                        let remove_only: LuaFunction = lua.registry_value(&ro_ref)?;
                        let remove_dups: LuaFunction = lua.registry_value(&rd_ref)?;

                        let map: LuaTable = with_aliases.call(map)?;
                        remove_dups.call::<()>(map.clone())?;

                        for pair in map.pairs::<String, LuaValue>() {
                            let (stroke, commands_val) = pair?;
                            let stroke: String = normalize.call(stroke)?;
                            let commands = ensure_table(lua, commands_val)?;

                            if overwrite {
                                let existing: LuaValue = km_map.get(stroke.clone())?;
                                if let LuaValue::Table(existing_tbl) = existing {
                                    for epair in existing_tbl.pairs::<LuaInteger, LuaValue>() {
                                        let (_, cmd) = epair?;
                                        remove_only.call::<()>((
                                            reverse_map.clone(),
                                            cmd,
                                            stroke.clone(),
                                        ))?;
                                    }
                                }
                                km_map.set(stroke.clone(), commands.clone())?;
                            } else {
                                let existing: LuaValue = km_map.get(stroke.clone())?;
                                let target = match existing {
                                    LuaValue::Table(t) => t,
                                    _ => {
                                        let t = lua.create_table()?;
                                        km_map.set(stroke.clone(), t.clone())?;
                                        t
                                    }
                                };
                                let cmd_len = commands.raw_len();
                                for i in (1..=cmd_len).rev() {
                                    let cmd: LuaValue = commands.get(i)?;
                                    table_insert_at(&target, 1, cmd)?;
                                }
                            }

                            for cpair in commands.pairs::<LuaInteger, LuaValue>() {
                                let (_, cmd) = cpair?;
                                let cmd_key = match &cmd {
                                    LuaValue::String(s) => s.to_str()?.to_string(),
                                    _ => continue,
                                };
                                let rev: LuaValue = reverse_map.get(cmd_key.clone())?;
                                let rev_tbl = match rev {
                                    LuaValue::Table(t) => t,
                                    _ => {
                                        let t = lua.create_table()?;
                                        reverse_map.set(cmd_key, t.clone())?;
                                        t
                                    }
                                };
                                let next_idx = rev_tbl.raw_len() + 1;
                                rev_tbl.set(next_idx, stroke.clone())?;
                            }
                        }
                        Ok(())
                    }
                })?,
            )?;

            // keymap.unbind(shortcut, cmd)
            keymap.set(
                "unbind",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    let norm_ref = lua.create_registry_value(normalize_stroke_fn.clone())?;
                    let ro_ref = lua.create_registry_value(remove_only_fn.clone())?;
                    move |lua, (shortcut, cmd): (String, String)| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let km_map: LuaTable = km.get("map")?;
                        let reverse_map: LuaTable = km.get("reverse_map")?;
                        let normalize: LuaFunction = lua.registry_value(&norm_ref)?;
                        let remove_only: LuaFunction = lua.registry_value(&ro_ref)?;

                        let shortcut: String = normalize.call(shortcut)?;
                        remove_only.call::<()>((km_map, shortcut.clone(), cmd.clone()))?;
                        remove_only.call::<()>((reverse_map, cmd, shortcut))?;
                        Ok(())
                    }
                })?,
            )?;

            // keymap.get_binding(cmd) -> ...
            keymap.set(
                "get_binding",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, cmd: String| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let reverse_map: LuaTable = km.get("reverse_map")?;
                        let val: LuaValue = reverse_map.get(cmd)?;
                        match val {
                            LuaValue::Table(t) => {
                                let unpack: LuaFunction =
                                    lua.globals().get::<LuaTable>("table")?.get("unpack")?;
                                unpack.call::<LuaMultiValue>(t)
                            }
                            _ => Ok(LuaMultiValue::new()),
                        }
                    }
                })?,
            )?;

            // keymap.get_bindings(cmd) -> table | nil
            keymap.set(
                "get_bindings",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, cmd: String| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let reverse_map: LuaTable = km.get("reverse_map")?;
                        reverse_map.get::<LuaValue>(cmd)
                    }
                })?,
            )?;

            // keymap.format_shortcut(shortcut) -> string | nil
            keymap.set(
                "format_shortcut",
                lua.create_function({
                    let dn_ref = lua.create_registry_value(display_names)?;
                    move |lua, shortcut: LuaValue| -> LuaResult<LuaValue> {
                        let shortcut_str = match shortcut {
                            LuaValue::String(s) => s.to_str()?.to_string(),
                            _ => return Ok(LuaValue::Nil),
                        };
                        let dn: LuaTable = lua.registry_value(&dn_ref)?;
                        let parts = split_stroke(&shortcut_str);
                        let formatted: Vec<String> = parts
                            .iter()
                            .map(|part| {
                                let val: LuaValue = dn.get(part.as_str()).unwrap_or(LuaValue::Nil);
                                match val {
                                    LuaValue::String(s) => s
                                        .to_str()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|_| part.clone()),
                                    _ => capitalize_first_char(part),
                                }
                            })
                            .collect();
                        Ok(LuaValue::String(lua.create_string(formatted.join("+"))?))
                    }
                })?,
            )?;

            // keymap.format_bindings(bindings) -> string
            keymap.set(
                "format_bindings",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, bindings: LuaValue| -> LuaResult<String> {
                        let bindings_tbl = match bindings {
                            LuaValue::Table(t) if t.raw_len() > 0 => t,
                            _ => return Ok(String::new()),
                        };
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let format_shortcut: LuaFunction = km.get("format_shortcut")?;
                        let mut formatted = Vec::new();
                        for pair in bindings_tbl.pairs::<LuaInteger, String>() {
                            let (_, shortcut) = pair?;
                            let f: LuaValue = format_shortcut.call(shortcut)?;
                            if let LuaValue::String(s) = f {
                                formatted.push(s.to_str()?.to_string());
                            }
                        }
                        Ok(formatted.join(", "))
                    }
                })?,
            )?;

            // keymap.get_binding_display(cmd) -> string | nil
            keymap.set(
                "get_binding_display",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, cmd: String| -> LuaResult<LuaValue> {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let get_bindings: LuaFunction = km.get("get_bindings")?;
                        let format_shortcut: LuaFunction = km.get("format_shortcut")?;
                        let bindings: LuaValue = get_bindings.call(cmd)?;
                        let bindings_tbl = match bindings {
                            LuaValue::Table(t) if t.raw_len() > 0 => t,
                            _ => return Ok(LuaValue::Nil),
                        };
                        if macos {
                            for pair in bindings_tbl.clone().pairs::<LuaInteger, String>() {
                                let (_, shortcut) = pair?;
                                let parts = split_stroke(&shortcut);
                                if parts.iter().any(|p| p == "cmd") {
                                    return format_shortcut.call(shortcut);
                                }
                            }
                        }
                        let first: LuaValue = bindings_tbl.get(1)?;
                        match first {
                            LuaValue::String(s) => format_shortcut.call(s.to_str()?.to_string()),
                            _ => Ok(LuaValue::Nil),
                        }
                    }
                })?,
            )?;

            // keymap.get_bindings_display(cmd) -> string
            keymap.set(
                "get_bindings_display",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, cmd: String| -> LuaResult<String> {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let get_bindings: LuaFunction = km.get("get_bindings")?;
                        let format_bindings: LuaFunction = km.get("format_bindings")?;
                        let bindings: LuaValue = get_bindings.call(cmd)?;
                        format_bindings.call(bindings)
                    }
                })?,
            )?;

            // keymap.on_key_pressed(k, ...) -> boolean
            keymap.set(
                "on_key_pressed",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    let mkmap_ref = lua.create_registry_value(modkey_map.clone())?;
                    let mklist_ref = lua.create_registry_value(modkeys_list)?;
                    let norm_ref = lua.create_registry_value(normalize_stroke_fn.clone())?;
                    move |lua, args: LuaMultiValue| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let modkey_map: LuaTable = lua.registry_value(&mkmap_ref)?;
                        let modkeys_list: LuaTable = lua.registry_value(&mklist_ref)?;
                        let normalize: LuaFunction = lua.registry_value(&norm_ref)?;
                        let km_modkeys: LuaTable = km.get("modkeys")?;
                        let km_map: LuaTable = km.get("map")?;

                        let mut args_vec: Vec<LuaValue> = args.into_iter().collect();
                        let k: String = match args_vec.first() {
                            Some(LuaValue::String(s)) => s.to_str()?.to_string(),
                            _ => return Ok(false),
                        };
                        args_vec.remove(0);

                        let mk: LuaValue = modkey_map.get(k.clone())?;
                        if let LuaValue::String(mk_str) = mk {
                            let mk_s = mk_str.to_str()?.to_string();
                            km_modkeys.set(mk_s.clone(), true)?;
                            if mk_s == "altgr" {
                                km_modkeys.set("ctrl", false)?;
                            }
                        } else {
                            let mut keys = vec![k];
                            for pair in modkeys_list.pairs::<LuaInteger, String>() {
                                let (_, mk_name) = pair?;
                                let pressed: bool =
                                    km_modkeys.get(mk_name.clone()).unwrap_or(false);
                                if pressed {
                                    keys.push(mk_name);
                                }
                            }
                            let stroke: String = normalize.call(keys.join("+"))?;

                            let commands: LuaValue = km_map.get(stroke)?;
                            if let LuaValue::Table(cmds) = commands {
                                let core: LuaTable = lua
                                    .globals()
                                    .get::<LuaTable>("package")?
                                    .get::<LuaTable>("loaded")?
                                    .get("core")?;
                                let command: LuaTable = {
                                    let require: LuaFunction = lua.globals().get("require")?;
                                    require.call("core.command")?
                                };

                                for pair in cmds.pairs::<LuaInteger, LuaValue>() {
                                    let (_, cmd) = pair?;
                                    let performed = match &cmd {
                                        LuaValue::Function(f) => {
                                            let core_try: LuaFunction = core.get("try")?;
                                            let rest = LuaMultiValue::from_iter(args_vec.clone());
                                            let try_result: LuaMultiValue = core_try
                                                .call(lua.pack_multi((f.clone(), rest))?)?;
                                            let vals: Vec<LuaValue> =
                                                try_result.into_iter().collect();
                                            let ok =
                                                is_truthy(vals.first().unwrap_or(&LuaValue::Nil));
                                            if ok {
                                                let res = vals.get(1);
                                                !matches!(res, Some(LuaValue::Boolean(false)))
                                            } else {
                                                true
                                            }
                                        }
                                        LuaValue::String(s) => {
                                            let cmd_name = s.to_str()?.to_string();
                                            let perform_fn: LuaFunction = command.get("perform")?;
                                            let rest = LuaMultiValue::from_iter(args_vec.clone());
                                            perform_fn.call(lua.pack_multi((cmd_name, rest))?)?
                                        }
                                        _ => false,
                                    };
                                    if performed {
                                        return Ok(true);
                                    }
                                }
                                return Ok(false);
                            }
                        }
                        Ok(false)
                    }
                })?,
            )?;

            // keymap.on_mouse_wheel(delta_y, delta_x, ...) -> boolean
            keymap.set(
                "on_mouse_wheel",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, args: LuaMultiValue| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let on_key: LuaFunction = km.get("on_key_pressed")?;
                        let args_vec: Vec<LuaValue> = args.into_iter().collect();

                        let delta_y: f64 = match args_vec.first() {
                            Some(LuaValue::Number(n)) => *n,
                            Some(LuaValue::Integer(n)) => *n as f64,
                            _ => 0.0,
                        };
                        let delta_x: f64 = match args_vec.get(1) {
                            Some(LuaValue::Number(n)) => *n,
                            Some(LuaValue::Integer(n)) => *n as f64,
                            _ => 0.0,
                        };
                        let rest: Vec<LuaValue> = if args_vec.len() > 2 {
                            args_vec[2..].to_vec()
                        } else {
                            Vec::new()
                        };

                        let y_dir = if delta_y > 0.0 { "up" } else { "down" };
                        let x_dir = if delta_x > 0.0 { "left" } else { "right" };

                        if delta_y != 0.0 && delta_x != 0.0 {
                            let key = format!("wheel{y_dir}{x_dir}");
                            let mut call_args = vec![
                                LuaValue::String(lua.create_string(&key)?),
                                LuaValue::Number(delta_y),
                                LuaValue::Number(delta_x),
                            ];
                            call_args.extend(rest.clone());
                            let result: bool = on_key.call(LuaMultiValue::from_iter(call_args))?;
                            if !result {
                                let mut call_args2 = vec![
                                    LuaValue::String(lua.create_string("wheelyx")?),
                                    LuaValue::Number(delta_y),
                                    LuaValue::Number(delta_x),
                                ];
                                call_args2.extend(rest.clone());
                                let result2: bool =
                                    on_key.call(LuaMultiValue::from_iter(call_args2))?;
                                if result2 {
                                    return Ok(true);
                                }
                            } else {
                                return Ok(true);
                            }
                        }

                        let mut y_result = false;
                        let mut x_result = false;

                        if delta_y != 0.0 {
                            let key = format!("wheel{y_dir}");
                            let mut call_args = vec![
                                LuaValue::String(lua.create_string(&key)?),
                                LuaValue::Number(delta_y),
                            ];
                            call_args.extend(rest.clone());
                            y_result = on_key.call(LuaMultiValue::from_iter(call_args))?;
                            if !y_result {
                                let mut call_args2 = vec![
                                    LuaValue::String(lua.create_string("wheel")?),
                                    LuaValue::Number(delta_y),
                                ];
                                call_args2.extend(rest.clone());
                                y_result = on_key.call(LuaMultiValue::from_iter(call_args2))?;
                            }
                        }

                        if delta_x != 0.0 {
                            let key = format!("wheel{x_dir}");
                            let mut call_args = vec![
                                LuaValue::String(lua.create_string(&key)?),
                                LuaValue::Number(delta_x),
                            ];
                            call_args.extend(rest.clone());
                            x_result = on_key.call(LuaMultiValue::from_iter(call_args))?;
                            if !x_result {
                                let mut call_args2 = vec![
                                    LuaValue::String(lua.create_string("hwheel")?),
                                    LuaValue::Number(delta_x),
                                ];
                                call_args2.extend(rest);
                                x_result = on_key.call(LuaMultiValue::from_iter(call_args2))?;
                            }
                        }

                        Ok(y_result || x_result)
                    }
                })?,
            )?;

            // keymap.on_mouse_pressed(button, x, y, clicks) -> boolean
            keymap.set(
                "on_mouse_pressed",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    move |lua, (button, x, y, clicks): (String, LuaValue, LuaValue, LuaInteger)| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let on_key: LuaFunction = km.get("on_key_pressed")?;
                        let config: LuaTable = {
                            let require: LuaFunction = lua.globals().get("require")?;
                            require.call("core.config")?
                        };
                        let max_clicks: LuaInteger = config.get("max_clicks")?;
                        let click_number = ((clicks - 1) % max_clicks) + 1;
                        let first_char = &button[..1];

                        let r1: bool = on_key.call((
                            format!("{click_number}{first_char}click"),
                            x.clone(),
                            y.clone(),
                            clicks,
                        ))?;
                        let r2: bool = on_key.call((
                            format!("{first_char}click"),
                            x.clone(),
                            y.clone(),
                            clicks,
                        ))?;
                        let r3: bool = on_key.call((
                            format!("{click_number}click"),
                            x.clone(),
                            y.clone(),
                            clicks,
                        ))?;
                        let r4: bool = on_key.call(("click", x, y, clicks))?;

                        Ok(!(r1 || r2 || r3 || r4))
                    }
                })?,
            )?;

            // keymap.on_key_released(k)
            keymap.set(
                "on_key_released",
                lua.create_function({
                    let km_ref = lua.create_registry_value(keymap.clone())?;
                    let mkmap_ref = lua.create_registry_value(modkey_map.clone())?;
                    move |lua, k: String| {
                        let km: LuaTable = lua.registry_value(&km_ref)?;
                        let modkey_map: LuaTable = lua.registry_value(&mkmap_ref)?;
                        let km_modkeys: LuaTable = km.get("modkeys")?;
                        let mk: LuaValue = modkey_map.get(k)?;
                        if let LuaValue::String(mk_str) = mk {
                            km_modkeys.set(mk_str.to_str()?, false)?;
                        }
                        Ok(())
                    }
                })?,
            )?;

            // Register default bindings
            if macos {
                let require: LuaFunction = lua.globals().get("require")?;
                let keymap_macos_fn: LuaFunction = require.call("core.keymap-macos")?;
                keymap_macos_fn.call::<()>(keymap.clone())?;
                return Ok(LuaValue::Table(keymap));
            }

            let add_direct: LuaFunction = keymap.get("add_direct")?;
            let defaults = build_default_bindings(lua)?;
            add_direct.call::<()>(defaults)?;

            Ok(LuaValue::Table(keymap))
        })?,
    )
}

fn split_stroke(stroke: &str) -> Vec<String> {
    stroke.split('+').map(|s| s.to_string()).collect()
}

fn normalize_stroke(stroke: &str, modkeys: &LuaTable) -> LuaResult<String> {
    let mut parts: Vec<String> = stroke.split('+').map(|s| s.to_string()).collect();

    let mod_order: Vec<String> = modkeys
        .clone()
        .pairs::<LuaInteger, String>()
        .filter_map(|r| r.ok())
        .map(|(_, v)| v)
        .collect();

    parts.sort_by(|a, b| {
        if a == b {
            return std::cmp::Ordering::Equal;
        }
        for m in &mod_order {
            if a == m || b == m {
                return if a == m {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
        }
        a.cmp(b)
    });

    Ok(parts.join("+"))
}

fn ensure_table(lua: &Lua, val: LuaValue) -> LuaResult<LuaTable> {
    match val {
        LuaValue::Table(t) => Ok(t),
        other => {
            let t = lua.create_table()?;
            t.set(1, other)?;
            Ok(t)
        }
    }
}

fn table_insert_at(tbl: &LuaTable, pos: usize, val: LuaValue) -> LuaResult<()> {
    let len = tbl.raw_len();
    for i in (pos..=len).rev() {
        let v: LuaValue = tbl.get(i)?;
        tbl.set(i + 1, v)?;
    }
    tbl.set(pos, val)?;
    Ok(())
}

fn is_truthy(val: &LuaValue) -> bool {
    !matches!(val, LuaValue::Nil | LuaValue::Boolean(false))
}

fn capitalize_first_char(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut result = String::with_capacity(s.len());
            for upper in c.to_uppercase() {
                result.push(upper);
            }
            result.push_str(chars.as_str());
            result
        }
    }
}

/// Builds the default (non-macOS) keybinding table.
fn build_default_bindings(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;

    let command_mod: LuaTable = {
        let require: LuaFunction = lua.globals().get("require")?;
        require.call("core.command")?
    };

    // y -> { "dialog:select-yes", function }
    {
        let y_cmds = lua.create_table()?;
        y_cmds.set(1, "dialog:select-yes")?;
        y_cmds.set(
            2,
            lua.create_function({
                let cmd_ref = lua.create_registry_value(command_mod.clone())?;
                move |lua, ()| -> LuaResult<LuaValue> {
                    let cmd: LuaTable = lua.registry_value(&cmd_ref)?;
                    let perform: LuaFunction = cmd.get("perform")?;
                    perform.call(("dialog:select-initial", "y"))
                }
            })?,
        )?;
        t.set("y", y_cmds)?;
    }

    // n -> { "dialog:select-no", function }
    {
        let n_cmds = lua.create_table()?;
        n_cmds.set(1, "dialog:select-no")?;
        n_cmds.set(
            2,
            lua.create_function({
                let cmd_ref = lua.create_registry_value(command_mod.clone())?;
                move |lua, ()| -> LuaResult<LuaValue> {
                    let cmd: LuaTable = lua.registry_value(&cmd_ref)?;
                    let perform: LuaFunction = cmd.get("perform")?;
                    perform.call(("dialog:select-initial", "n"))
                }
            })?,
        )?;
        t.set("n", n_cmds)?;
    }

    // t -> function
    t.set(
        "t",
        lua.create_function({
            let cmd_ref = lua.create_registry_value(command_mod.clone())?;
            move |lua, ()| -> LuaResult<LuaValue> {
                let cmd: LuaTable = lua.registry_value(&cmd_ref)?;
                let perform: LuaFunction = cmd.get("perform")?;
                perform.call(("dialog:select-initial", "t"))
            }
        })?,
    )?;

    // c -> function
    t.set(
        "c",
        lua.create_function({
            let cmd_ref = lua.create_registry_value(command_mod)?;
            move |lua, ()| -> LuaResult<LuaValue> {
                let cmd: LuaTable = lua.registry_value(&cmd_ref)?;
                let perform: LuaFunction = cmd.get("perform")?;
                perform.call(("dialog:select-initial", "c"))
            }
        })?,
    )?;

    let bindings: &[(&str, &[&str])] = &[
        ("ctrl+p", &["core:find-command"]),
        ("ctrl+q", &["core:quit"]),
        ("ctrl+o", &["core:open-file"]),
        ("ctrl+shift+r", &["core:open-recent-file"]),
        ("ctrl+alt+shift+r", &["core:open-recent-folder"]),
        ("ctrl+n", &["core:new-doc"]),
        ("ctrl+shift+n", &["core:new-window"]),
        ("ctrl+shift+c", &["core:change-project-folder"]),
        ("ctrl+alt+o", &["core:open-project-folder"]),
        ("ctrl+alt+w", &["core:close-project-folder"]),
        ("ctrl+alt+r", &["core:restart"]),
        ("alt+return", &["core:toggle-fullscreen"]),
        ("f11", &["core:toggle-fullscreen"]),
        ("alt+shift+j", &["root:split-left"]),
        ("alt+shift+l", &["root:split-right"]),
        ("alt+shift+i", &["root:split-up"]),
        ("alt+shift+k", &["root:split-down"]),
        ("alt+j", &["root:switch-to-left"]),
        ("alt+l", &["root:switch-to-right"]),
        ("alt+i", &["root:switch-to-up"]),
        ("alt+k", &["root:switch-to-down"]),
        ("ctrl+w", &["root:close"]),
        ("ctrl+tab", &["root:switch-to-next-tab"]),
        ("ctrl+shift+tab", &["root:switch-to-previous-tab"]),
        ("alt+tab", &["root:switch-to-next-tab"]),
        ("alt+shift+tab", &["root:switch-to-previous-tab"]),
        ("ctrl+shift+f", &["root:toggle-focus-mode"]),
        ("ctrl+pageup", &["root:move-tab-left"]),
        ("ctrl+pagedown", &["root:move-tab-right"]),
        ("alt+2", &["root:switch-to-tab-2"]),
        ("alt+3", &["root:switch-to-tab-3"]),
        ("alt+4", &["root:switch-to-tab-4"]),
        ("alt+5", &["root:switch-to-tab-5"]),
        ("alt+6", &["root:switch-to-tab-6"]),
        ("alt+7", &["root:switch-to-tab-7"]),
        ("alt+8", &["root:switch-to-tab-8"]),
        ("alt+9", &["root:switch-to-tab-9"]),
        ("wheel", &["root:scroll"]),
        ("hwheel", &["root:horizontal-scroll"]),
        ("shift+wheel", &["root:horizontal-scroll"]),
        ("wheelup", &["root:scroll-hovered-tabs-backward"]),
        ("wheeldown", &["root:scroll-hovered-tabs-forward"]),
        ("ctrl+f", &["find-replace:find"]),
        ("ctrl+h", &["find-replace:replace"]),
        ("alt+w", &["find-replace:toggle-whole-word"]),
        ("ctrl+r", &["find-replace:replace"]),
        ("f3", &["find-replace:repeat-find"]),
        ("shift+f3", &["find-replace:previous-find"]),
        ("ctrl+i", &["find-replace:toggle-sensitivity"]),
        ("ctrl+shift+i", &["find-replace:toggle-regex"]),
        ("alt+s", &["find-replace:toggle-in-selection"]),
        ("ctrl+g", &["doc:go-to-line"]),
        ("ctrl+s", &["doc:save"]),
        ("ctrl+shift+s", &["doc:save-as"]),
        ("ctrl+z", &["doc:undo"]),
        ("ctrl+shift+z", &["doc:redo"]),
        ("ctrl+y", &["doc:redo"]),
        ("ctrl+x", &["doc:cut"]),
        ("ctrl+c", &["doc:copy"]),
        ("ctrl+v", &["doc:paste"]),
        ("insert", &["doc:toggle-overwrite"]),
        ("ctrl+insert", &["doc:copy"]),
        ("shift+insert", &["doc:paste"]),
        (
            "escape",
            &[
                "root:exit-focus-mode",
                "command:escape",
                "doc:select-none",
                "context-menu:hide",
                "dialog:select-no",
            ],
        ),
        ("tab", &["command:complete", "doc:indent"]),
        ("shift+tab", &["doc:unindent"]),
        ("backspace", &["doc:backspace"]),
        ("shift+backspace", &["doc:backspace"]),
        ("ctrl+backspace", &["doc:delete-to-previous-word-start"]),
        (
            "ctrl+shift+backspace",
            &["doc:delete-to-previous-word-start"],
        ),
        ("delete", &["doc:delete"]),
        ("shift+delete", &["doc:delete"]),
        ("ctrl+delete", &["doc:delete-to-next-word-end"]),
        ("ctrl+shift+delete", &["doc:delete-to-next-word-end"]),
        (
            "return",
            &[
                "command:submit",
                "context-menu:submit",
                "doc:newline",
                "dialog:select",
            ],
        ),
        (
            "keypad enter",
            &["command:submit", "doc:newline", "dialog:select"],
        ),
        ("ctrl+return", &["doc:newline-below"]),
        ("ctrl+shift+return", &["doc:newline-above"]),
        ("ctrl+j", &["doc:join-lines"]),
        ("ctrl+a", &["doc:select-all"]),
        (
            "ctrl+d",
            &["find-replace:select-add-next", "doc:select-word"],
        ),
        ("ctrl+alt+l", &["find-replace:select-all-found"]),
        ("ctrl+f3", &["find-replace:select-next"]),
        ("ctrl+shift+f3", &["find-replace:select-previous"]),
        ("ctrl+l", &["doc:select-lines"]),
        (
            "ctrl+shift+l",
            &["find-replace:select-add-all", "doc:select-word"],
        ),
        ("ctrl+/", &["doc:toggle-line-comments"]),
        ("ctrl+shift+/", &["doc:toggle-block-comments"]),
        ("ctrl+up", &["doc:move-lines-up"]),
        ("ctrl+down", &["doc:move-lines-down"]),
        ("ctrl+shift+d", &["doc:duplicate-lines"]),
        ("ctrl+shift+k", &["doc:delete-lines"]),
        (
            "left",
            &["doc:move-to-previous-char", "dialog:previous-entry"],
        ),
        ("right", &["doc:move-to-next-char", "dialog:next-entry"]),
        (
            "up",
            &[
                "command:select-previous",
                "context-menu:focus-previous",
                "doc:move-to-previous-line",
            ],
        ),
        (
            "down",
            &[
                "command:select-next",
                "context-menu:focus-next",
                "doc:move-to-next-line",
            ],
        ),
        ("ctrl+left", &["doc:move-to-previous-word-start"]),
        ("ctrl+right", &["doc:move-to-next-word-end"]),
        ("ctrl+[", &["doc:move-to-previous-block-start"]),
        ("ctrl+]", &["doc:move-to-next-block-end"]),
        ("home", &["doc:move-to-start-of-indentation"]),
        ("end", &["doc:move-to-end-of-line"]),
        ("ctrl+home", &["doc:move-to-start-of-doc"]),
        ("ctrl+end", &["doc:move-to-end-of-doc"]),
        ("pageup", &["doc:move-to-previous-page"]),
        ("pagedown", &["doc:move-to-next-page"]),
        ("shift+1lclick", &["doc:select-to-cursor"]),
        ("ctrl+1lclick", &["doc:split-cursor"]),
        (
            "1lclick",
            &["context-menu:select", "context-menu:hide", "doc:set-cursor"],
        ),
        (
            "2lclick",
            &["doc:set-cursor-word", "emptyview:new-doc", "tabbar:new-doc"],
        ),
        ("3lclick", &["doc:set-cursor-line"]),
        ("rclick", &["context-menu:show"]),
        ("menu", &["context-menu:show"]),
        ("mclick", &["doc:paste-primary-selection"]),
        ("shift+left", &["doc:select-to-previous-char"]),
        ("shift+right", &["doc:select-to-next-char"]),
        ("shift+up", &["doc:select-to-previous-line"]),
        ("shift+down", &["doc:select-to-next-line"]),
        ("ctrl+shift+left", &["doc:select-to-previous-word-start"]),
        ("ctrl+shift+right", &["doc:select-to-next-word-end"]),
        ("ctrl+shift+[", &["doc:select-to-previous-block-start"]),
        ("ctrl+shift+]", &["doc:select-to-next-block-end"]),
        ("shift+home", &["doc:select-to-start-of-indentation"]),
        ("shift+end", &["doc:select-to-end-of-line"]),
        ("ctrl+shift+home", &["doc:select-to-start-of-doc"]),
        ("ctrl+shift+end", &["doc:select-to-end-of-doc"]),
        ("shift+pageup", &["doc:select-to-previous-page"]),
        ("shift+pagedown", &["doc:select-to-next-page"]),
        ("ctrl+shift+up", &["doc:create-cursor-previous-line"]),
        ("ctrl+shift+down", &["doc:create-cursor-next-line"]),
    ];

    for (key, cmds) in bindings {
        if cmds.len() == 1 {
            t.set(*key, cmds[0])?;
        } else {
            let arr = lua.create_table()?;
            for (i, cmd) in cmds.iter().enumerate() {
                arr.set(i + 1, *cmd)?;
            }
            t.set(*key, arr)?;
        }
    }

    Ok(t)
}
