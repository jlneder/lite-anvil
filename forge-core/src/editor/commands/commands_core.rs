use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Returns the root project path or falls back to the absolute path of ".".
fn current_project_path(lua: &Lua) -> LuaResult<String> {
    let core: LuaTable = require_table(lua, "core")?;
    let root_project: LuaValue = core.get("root_project")?;
    if let LuaValue::Function(f) = root_project {
        let project: LuaValue = f.call(())?;
        if let LuaValue::Table(p) = project {
            let path: Option<String> = p.get("path")?;
            if let Some(path) = path {
                return Ok(path);
            }
        }
    }
    let system: LuaTable = lua.globals().get("system")?;
    system.call_function("absolute_path", ".")
}

/// Builds a suggest function for directory path completion.
fn create_suggest_directory(lua: &Lua) -> LuaResult<LuaFunction> {
    lua.create_function(|lua, text: String| -> LuaResult<LuaValue> {
        let common: LuaTable = require_table(lua, "core.common")?;
        let core: LuaTable = require_table(lua, "core")?;

        let expanded: String = common.call_function("home_expand", text.clone())?;
        let project_path = current_project_path(lua)?;
        let basedir: LuaValue = common.call_function("dirname", project_path.clone())?;

        let pathsep: String = lua.globals().get("PATHSEP")?;
        let check = if let LuaValue::String(ref bd) = basedir {
            let bd_str = bd.to_str()?.to_string();
            expanded == format!("{bd_str}{pathsep}") || expanded.is_empty()
        } else {
            expanded.is_empty()
        };

        let list: LuaValue = if check {
            core.get("recent_projects")?
        } else {
            common.call_function("dir_path_suggest", (expanded, project_path))?
        };

        common.call_function("home_encode_list", list)
    })
}

/// Checks a directory path, returning the absolute path if valid.
fn check_directory_path(lua: &Lua, path: &str) -> LuaResult<Option<String>> {
    let system: LuaTable = lua.globals().get("system")?;
    let abs: LuaValue = system.call_function("absolute_path", path)?;
    let abs_str = match &abs {
        LuaValue::String(s) => s.to_str()?.to_string(),
        _ => return Ok(None),
    };
    let info: LuaValue = system.call_function("get_file_info", abs_str.clone())?;
    match info {
        LuaValue::Table(t) => {
            let ftype: String = t.get("type")?;
            if ftype == "dir" {
                Ok(Some(abs_str))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

/// Opens a file using either a system dialog or the command view.
fn open_file(lua: &Lua, use_dialog: bool) -> LuaResult<()> {
    let core: LuaTable = require_table(lua, "core")?;
    let common: LuaTable = require_table(lua, "core.common")?;
    let active_view: LuaTable = core.get("active_view")?;

    // Compute default_text from the active view's doc filename
    let default_text: LuaValue = (|| -> LuaResult<LuaValue> {
        let doc: LuaValue = active_view.get("doc")?;
        let doc = match doc {
            LuaValue::Table(d) => d,
            _ => return Ok(LuaValue::Nil),
        };
        let abs_filename: LuaValue = doc.get("abs_filename")?;
        let abs_filename_str = match &abs_filename {
            LuaValue::String(s) => s.to_str()?.to_string(),
            _ => return Ok(LuaValue::Nil),
        };
        // Extract dirname via pattern: (.*)[/\\](.+)$
        let sep_pos = abs_filename_str.rfind(['/', '\\']);
        let dirname = match sep_pos {
            Some(pos) if pos < abs_filename_str.len() - 1 => &abs_filename_str[..pos],
            _ => return Ok(LuaValue::Nil),
        };
        if use_dialog {
            return Ok(LuaValue::String(lua.create_string(dirname)?));
        }
        let root_project: LuaValue = core.get("root_project")?;
        let (normalized, project_path) = if let LuaValue::Function(ref f) = root_project {
            let project: LuaValue = f.call(())?;
            if let LuaValue::Table(ref p) = project {
                let norm: String = p.call_method("normalize_path", dirname)?;
                let pp: String = p.get("path")?;
                (norm, Some(pp))
            } else {
                let norm: String = common.call_function("normalize_path", dirname)?;
                (norm, None)
            }
        } else {
            let norm: String = common.call_function("normalize_path", dirname)?;
            (norm, None)
        };
        if let Some(ref pp) = project_path {
            if normalized == *pp {
                return Ok(LuaValue::String(lua.create_string("")?));
            }
        }
        let pathsep: String = lua.globals().get("PATHSEP")?;
        let encoded: String = common.call_function("home_encode", normalized)?;
        Ok(LuaValue::String(
            lua.create_string(format!("{encoded}{pathsep}"))?,
        ))
    })()?;

    if use_dialog {
        let window: LuaTable = core.get("window")?;
        let callback = lua.create_function(|lua, (status, result): (String, LuaValue)| {
            let core: LuaTable = require_table(lua, "core")?;
            if status == "accept" {
                if let LuaValue::Table(files) = result {
                    let root_view: LuaTable = core.get("root_view")?;
                    for i in 1..=files.raw_len() {
                        let filename: String = files.get(i)?;
                        let doc: LuaTable = core.call_function("open_doc", filename)?;
                        root_view.call_method::<()>("open_doc", doc)?;
                    }
                }
            } else if status == "error" {
                let err_msg = match result {
                    LuaValue::String(s) => s.to_str()?.to_string(),
                    _ => String::new(),
                };
                core.call_function::<()>(
                    "error",
                    format!("Error while opening dialog: {err_msg}"),
                )?;
            }
            Ok(())
        })?;
        let opts = lua.create_table()?;
        opts.set("default_location", default_text)?;
        opts.set("allow_many", true)?;
        core.call_function::<()>("open_file_dialog", (window, callback, opts))?;
        return Ok(());
    }

    let command_view: LuaTable = core.get("command_view")?;
    let opts = lua.create_table()?;
    opts.set("text", default_text)?;
    opts.set(
        "submit",
        lua.create_function(|lua, text: String| {
            let core: LuaTable = require_table(lua, "core")?;
            let common: LuaTable = require_table(lua, "core.common")?;
            let expanded: String = common.call_function("home_expand", text)?;
            let project_path = current_project_path(lua)?;
            let root_project: LuaValue = core.get("root_project")?;
            let filename: String = if let LuaValue::Function(f) = root_project {
                let project: LuaValue = f.call(())?;
                if let LuaValue::Table(p) = project {
                    p.call_method("absolute_path", expanded)?
                } else {
                    let system: LuaTable = lua.globals().get("system")?;
                    system.call_function("absolute_path", expanded)?
                }
            } else {
                let system: LuaTable = lua.globals().get("system")?;
                system.call_function("absolute_path", expanded)?
            };
            let _ = project_path;
            let root_view: LuaTable = core.get("root_view")?;
            let doc: LuaTable = core.call_function("open_doc", filename)?;
            root_view.call_method::<()>("open_doc", doc)
        })?,
    )?;
    opts.set(
        "suggest",
        lua.create_function(|lua, text: String| {
            let common: LuaTable = require_table(lua, "core.common")?;
            if text.is_empty() {
                let core: LuaTable = require_table(lua, "core")?;
                let recent: LuaValue = core
                    .get::<LuaValue>("recent_files")
                    .unwrap_or(LuaValue::Nil);
                if let LuaValue::Table(ref t) = recent {
                    if t.raw_len() > 0 {
                        let ranked = recent_items_fn(lua, recent, String::new())?;
                        return common.call_function::<LuaValue>("home_encode_list", ranked);
                    }
                }
            }
            let expanded: String = common.call_function("home_expand", text)?;
            let project_path = current_project_path(lua)?;
            let suggested: LuaValue =
                common.call_function("path_suggest", (expanded, project_path))?;
            common.call_function::<LuaValue>("home_encode_list", suggested)
        })?,
    )?;
    opts.set(
        "validate",
        lua.create_function(|lua, text: String| {
            if text.is_empty() {
                return Ok(false);
            }
            let core: LuaTable = require_table(lua, "core")?;
            let common: LuaTable = require_table(lua, "core.common")?;
            let system: LuaTable = lua.globals().get("system")?;
            let expanded: String = common.call_function("home_expand", text.clone())?;
            let root_project: LuaValue = core.get("root_project")?;
            let filename: LuaValue = if let LuaValue::Function(f) = root_project {
                let project: LuaValue = f.call(())?;
                if let LuaValue::Table(p) = project {
                    p.call_method("absolute_path", expanded)?
                } else {
                    system.call_function("absolute_path", expanded)?
                }
            } else {
                system.call_function("absolute_path", expanded)?
            };
            let filename_str = match &filename {
                LuaValue::String(s) => s.to_str()?.to_string(),
                _ => return Ok(false),
            };
            let result: LuaMultiValue =
                system.call_function("get_file_info", filename_str.clone())?;
            let mut vals = result.into_iter();
            let info_val = vals.next().unwrap_or(LuaValue::Nil);
            let err_val = vals.next().unwrap_or(LuaValue::Nil);

            match (&info_val, &err_val) {
                (_, LuaValue::String(err)) => {
                    let err_str = err.to_str()?.to_string();
                    if err_str.contains("No such file") {
                        let dirname: LuaValue = common.call_function("dirname", filename_str)?;
                        if let LuaValue::String(dn) = &dirname {
                            let dn_str = dn.to_str()?.to_string();
                            let dir_info: LuaValue =
                                system.call_function("get_file_info", dn_str)?;
                            if let LuaValue::Table(di) = dir_info {
                                let dtype: String = di.get("type")?;
                                if dtype == "dir" {
                                    return Ok(true);
                                }
                            }
                        } else {
                            return Ok(true);
                        }
                    }
                    core.call_function::<()>(
                        "error",
                        format!("Cannot open file {text}: {err_str}"),
                    )?;
                    Ok(false)
                }
                (LuaValue::Table(info), _) => {
                    let ftype: String = info.get("type")?;
                    if ftype == "dir" {
                        core.call_function::<()>(
                            "error",
                            format!("Cannot open {text}, is a folder"),
                        )?;
                        Ok(false)
                    } else {
                        Ok(true)
                    }
                }
                _ => Ok(false),
            }
        })?,
    )?;

    command_view.call_method::<()>("enter", ("Open File", opts))
}

/// Opens a directory using either a system dialog or the command view.
fn open_directory(
    lua: &Lua,
    label: &str,
    use_dialog: bool,
    allow_many: bool,
    callback: LuaFunction,
) -> LuaResult<()> {
    let core: LuaTable = require_table(lua, "core")?;
    let common: LuaTable = require_table(lua, "core.common")?;

    let project_path = current_project_path(lua)?;
    let dirname: LuaValue = common.call_function("dirname", project_path)?;

    let text: LuaValue = if let LuaValue::String(ref dn) = dirname {
        let dn_str = dn.to_str()?.to_string();
        if use_dialog {
            LuaValue::String(lua.create_string(&dn_str)?)
        } else {
            let pathsep: String = lua.globals().get("PATHSEP")?;
            let encoded: String = common.call_function("home_encode", dn_str)?;
            LuaValue::String(lua.create_string(format!("{encoded}{pathsep}"))?)
        }
    } else {
        LuaValue::Nil
    };

    if use_dialog {
        let window: LuaTable = core.get("window")?;
        let dialog_callback = lua.create_function({
            let callback_key = std::sync::Arc::new(lua.create_registry_value(callback)?);
            move |lua, (status, result): (String, LuaValue)| {
                let core: LuaTable = require_table(lua, "core")?;
                let cb: LuaFunction = lua.registry_value(&callback_key)?;
                if status == "accept" {
                    cb.call::<()>(result)?;
                } else if status == "error" {
                    let err_msg = match result {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => String::new(),
                    };
                    core.call_function::<()>(
                        "error",
                        format!("Error while opening dialog: {err_msg}"),
                    )?;
                }
                Ok(())
            }
        })?;
        let opts = lua.create_table()?;
        opts.set("default_location", text)?;
        opts.set("allow_many", allow_many)?;
        opts.set("title", label)?;
        core.call_function::<()>("open_directory_dialog", (window, dialog_callback, opts))?;
        return Ok(());
    }

    let command_view: LuaTable = core.get("command_view")?;
    let opts = lua.create_table()?;
    opts.set("text", text)?;
    let submit_cb_key = std::sync::Arc::new(lua.create_registry_value(callback)?);
    opts.set(
        "submit",
        lua.create_function(move |lua, text: String| {
            let common: LuaTable = require_table(lua, "core.common")?;
            let core: LuaTable = require_table(lua, "core")?;
            let path: String = common.call_function("home_expand", text)?;
            match check_directory_path(lua, &path)? {
                Some(abs_path) => {
                    let cb: LuaFunction = lua.registry_value(&submit_cb_key)?;
                    let result = lua.create_table()?;
                    result.push(abs_path)?;
                    cb.call::<()>(result)
                }
                None => {
                    core.call_function::<()>("error", format!("Cannot open directory {path:?}"))
                }
            }
        })?,
    )?;
    opts.set("suggest", create_suggest_directory(lua)?)?;
    command_view.call_method::<()>("enter", (label, opts))
}

/// Builds a recent-items ranking function.
fn recent_items_fn(lua: &Lua, items: LuaValue, text: String) -> LuaResult<LuaValue> {
    let native_picker: LuaTable = require_table(lua, "picker")?;
    native_picker.call_function("rank_strings", (items, text))
}

/// Registers all core commands.
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    let cmds = lua.create_table()?;

    cmds.set(
        "about:version",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let style: LuaTable = require_table(lua, "core.style")?;
            let version: String = lua.globals().get("VERSION")?;
            let text = format!("Lite-Anvil {version}");
            let sv: LuaTable = core.get("status_view")?;
            let color: LuaValue = style.get("text")?;
            sv.call_method::<()>("show_message", ("i", color, text.clone()))?;
            core.call_function::<()>("log", text)
        })?,
    )?;

    cmds.set(
        "core:quit",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            core.call_function::<()>("quit", ())
        })?,
    )?;

    cmds.set(
        "core:restart",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            core.call_function::<()>("restart", ())
        })?,
    )?;

    cmds.set(
        "core:force-quit",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            core.call_function::<()>("quit", true)
        })?,
    )?;

    cmds.set(
        "core:toggle-fullscreen",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let system: LuaTable = lua.globals().get("system")?;

            // Lazy-init fullscreen state on core table
            let fs_state: LuaValue = core.get("_fullscreen_state")?;
            let state: LuaTable = if let LuaValue::Table(t) = fs_state {
                t
            } else {
                let t = lua.create_table()?;
                t.set("fullscreen", false)?;
                t.set("restore_title_view", false)?;
                core.set("_fullscreen_state", t.clone())?;
                t
            };

            let fullscreen: bool = state.get("fullscreen")?;
            let new_fullscreen = !fullscreen;
            state.set("fullscreen", new_fullscreen)?;

            if new_fullscreen {
                let title_view: LuaTable = core.get("title_view")?;
                let visible: bool = title_view.get("visible")?;
                state.set("restore_title_view", visible)?;
            }

            let window: LuaValue = core.get("window")?;
            let mode = if new_fullscreen {
                "fullscreen"
            } else {
                "normal"
            };
            let set_window_mode: LuaFunction = system.get("set_window_mode")?;
            set_window_mode.call::<()>((window, mode))?;

            let restore: bool = state.get("restore_title_view")?;
            let show = !new_fullscreen && restore;
            let show_title_bar: LuaFunction = core.get("show_title_bar")?;
            show_title_bar.call::<()>(show)?;
            let title_view: LuaTable = core.get("title_view")?;
            title_view.call_method::<()>("configure_hit_test", show)?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "core:reload-module",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function(|lua, (text, item): (String, LuaValue)| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let name = if let LuaValue::Table(ref t) = item {
                        t.get::<String>("text").unwrap_or(text)
                    } else {
                        text
                    };
                    core.call_function::<()>("reload_module", name.clone())?;
                    core.call_function::<()>("log", format!("Reloaded module {name:?}"))
                })?,
            )?;
            opts.set(
                "suggest",
                lua.create_function(|lua, text: String| {
                    let native_picker: LuaTable = require_table(lua, "picker")?;
                    let package: LuaTable = lua.globals().get("package")?;
                    let loaded: LuaTable = package.get("loaded")?;
                    let items = lua.create_table()?;
                    for pair in loaded.pairs::<String, LuaValue>() {
                        let (name, _) = pair?;
                        items.push(name)?;
                    }
                    native_picker.call_function::<LuaValue>("rank_strings", (items, text))
                })?,
            )?;
            command_view.call_method::<()>("enter", ("Reload Module", opts))
        })?,
    )?;

    cmds.set(
        "core:find-command",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command: LuaTable = require_table(lua, "core.command")?;
            let commands: LuaTable = command.call_function("get_all_valid", ())?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function(|lua, (_text, item): (String, LuaValue)| {
                    if let LuaValue::Table(t) = item {
                        let cmd_name: String = t.get("command")?;
                        let command: LuaTable = require_table(lua, "core.command")?;
                        command.call_function::<()>("perform", cmd_name)?;
                    }
                    Ok(())
                })?,
            )?;

            let cmds_key = std::sync::Arc::new(lua.create_registry_value(commands)?);
            opts.set(
                "suggest",
                lua.create_function(move |lua, text: String| {
                    let command: LuaTable = require_table(lua, "core.command")?;
                    let keymap: LuaTable = require_table(lua, "core.keymap")?;
                    let native_picker: LuaTable = require_table(lua, "picker")?;
                    let commands: LuaTable = lua.registry_value(&cmds_key)?;
                    let matched: LuaTable =
                        native_picker.call_function("rank_strings", (commands, text))?;
                    let res = lua.create_table()?;
                    for i in 1..=matched.raw_len() {
                        let name: String = matched.get(i)?;
                        let pretty: String =
                            command.call_function("prettify_name", name.clone())?;
                        let bindings: String =
                            keymap.call_function("get_bindings_display", name.clone())?;
                        let entry = lua.create_table()?;
                        entry.set("text", pretty)?;
                        entry.set("info", bindings)?;
                        entry.set("command", name)?;
                        res.push(entry)?;
                    }
                    Ok(res)
                })?,
            )?;
            command_view.call_method::<()>("enter", ("Do Command", opts))
        })?,
    )?;

    cmds.set(
        "core:show-shortcuts-help",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command: LuaTable = require_table(lua, "core.command")?;
            let keymap: LuaTable = require_table(lua, "core.keymap")?;
            let doc_class: LuaTable = require_table(lua, "core.doc")?;

            // Collect and sort command names
            let cmd_map: LuaTable = command.get("map")?;
            let mut names: Vec<String> = Vec::new();
            for pair in cmd_map.pairs::<String, LuaValue>() {
                let (name, _) = pair?;
                names.push(name);
            }
            names.sort();

            // Compute max pretty-name width
            let mut width: usize = 0;
            let mut prettified: Vec<String> = Vec::with_capacity(names.len());
            for name in &names {
                let pretty: String = command.call_function("prettify_name", name.as_str())?;
                width = width.max(pretty.len());
                prettified.push(pretty);
            }

            // Build formatted lines
            let mut lines: Vec<String> = Vec::with_capacity(names.len());
            for (i, name) in names.iter().enumerate() {
                let pretty = &prettified[i];
                let bindings: String =
                    keymap.call_function("get_bindings_display", name.as_str())?;
                let bindings = if bindings.is_empty() {
                    "Unbound".to_string()
                } else {
                    bindings
                };
                lines.push(format!("{pretty:<width$}  {bindings}"));
            }
            let content = lines.join("\n");

            // Create a doc and open it
            let doc: LuaTable = doc_class.call_function("__call", doc_class.clone())?;
            doc.call_method::<()>("set_filename", "Shortcuts")?;
            doc.call_method::<()>("insert", (1, 1, content))?;
            doc.set("new_file", false)?;
            doc.call_method::<()>("clean", ())?;

            let root_view: LuaTable = core.get("root_view")?;
            let view: LuaTable = root_view.call_method("open_doc", doc)?;
            let scroll: LuaTable = view.get("scroll")?;
            let to: LuaTable = scroll.get("to")?;
            to.set("y", 0)?;
            scroll.set("y", 0)?;
            Ok(())
        })?,
    )?;

    cmds.set(
        "core:new-doc",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            let doc: LuaTable = core.call_function("open_doc", ())?;
            root_view.call_method::<()>("open_doc", doc)
        })?,
    )?;

    cmds.set(
        "core:new-window",
        lua.create_function(|lua, ()| {
            let system: LuaTable = lua.globals().get("system")?;
            let exefile: String = lua.globals().get("EXEFILE")?;
            system.call_function::<()>("exec", format!("{exefile:?}"))
        })?,
    )?;

    cmds.set(
        "core:new-named-doc",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function(|lua, text: String| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let root_view: LuaTable = core.get("root_view")?;
                    let doc: LuaTable = core.call_function("open_doc", text)?;
                    root_view.call_method::<()>("open_doc", doc)
                })?,
            )?;
            command_view.call_method::<()>("enter", ("File name", opts))
        })?,
    )?;

    cmds.set(
        "core:open-file",
        lua.create_function(|lua, ()| {
            let config: LuaTable = require_table(lua, "core.config")?;
            let use_dialog: bool = config.get("use_system_file_picker").unwrap_or(false);
            open_file(lua, use_dialog)
        })?,
    )?;

    cmds.set(
        "core:open-file-picker",
        lua.create_function(|lua, ()| open_file(lua, true))?,
    )?;

    cmds.set(
        "core:open-file-commandview",
        lua.create_function(|lua, ()| open_file(lua, false))?,
    )?;

    cmds.set(
        "core:open-recent-file",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "suggest",
                lua.create_function(|lua, text: String| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let recent: LuaValue = core
                        .get::<LuaValue>("recent_files")
                        .unwrap_or(LuaValue::Nil);
                    let items = if matches!(recent, LuaValue::Table(_)) {
                        recent
                    } else {
                        LuaValue::Table(lua.create_table()?)
                    };
                    let expanded: String = common.call_function("home_expand", text)?;
                    let ranked = recent_items_fn(lua, items, expanded)?;
                    common.call_function::<LuaValue>("home_encode_list", ranked)
                })?,
            )?;
            opts.set(
                "submit",
                lua.create_function(|lua, text: String| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let root_view: LuaTable = core.get("root_view")?;
                    let expanded: String = common.call_function("home_expand", text)?;
                    let doc: LuaTable = core.call_function("open_doc", expanded)?;
                    root_view.call_method::<()>("open_doc", doc)
                })?,
            )?;
            opts.set(
                "validate",
                lua.create_function(|lua, text: String| {
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let expanded: String = common.call_function("home_expand", text)?;
                    let info: LuaValue = system.call_function("get_file_info", expanded)?;
                    match info {
                        LuaValue::Table(t) => {
                            let ftype: String = t.get("type")?;
                            Ok(ftype == "file")
                        }
                        _ => Ok(false),
                    }
                })?,
            )?;
            command_view.call_method::<()>("enter", ("Recent File", opts))
        })?,
    )?;

    cmds.set(
        "core:open-log",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let log_view_class: LuaTable = require_table(lua, "core.logview")?;
            let root_view: LuaTable = core.get("root_view")?;
            let node: LuaTable = root_view.call_method("get_active_node_default", ())?;
            let log_view: LuaTable =
                log_view_class.call_function("__call", log_view_class.clone())?;
            node.call_method::<()>("add_view", log_view)
        })?,
    )?;

    cmds.set(
        "core:open-user-module",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            core.call_function::<()>("ensure_user_directory", ())?;
            let userdir: String = lua.globals().get("USERDIR")?;
            let path = format!("{userdir}/config.lua");
            let doc: LuaValue = core.call_function("open_doc", path)?;
            if let LuaValue::Table(doc) = doc {
                let root_view: LuaTable = core.get("root_view")?;
                root_view.call_method::<()>("open_doc", doc)?;
            }
            Ok(())
        })?,
    )?;

    cmds.set(
        "core:open-project-module",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let system: LuaTable = lua.globals().get("system")?;
            let info: LuaValue = system.call_function("get_file_info", ".lite_project.lua")?;
            if matches!(info, LuaValue::Nil | LuaValue::Boolean(false)) {
                let write_fn: LuaFunction = core.get("write_init_project_module")?;
                core.call_function::<()>("try", (write_fn, ".lite_project.lua"))?;
            }
            let doc: LuaTable = core.call_function("open_doc", ".lite_project.lua")?;
            let root_view: LuaTable = core.get("root_view")?;
            root_view.call_method::<()>("open_doc", doc.clone())?;
            doc.call_method::<()>("save", ())
        })?,
    )?;

    // Change/open/add project directory commands
    for suffix in &["", "-picker", "-commandview"] {
        let use_dialog_fn = match *suffix {
            "" => lua.create_function(|lua, ()| {
                let config: LuaTable = require_table(lua, "core.config")?;
                Ok(config
                    .get::<bool>("use_system_file_picker")
                    .unwrap_or(false))
            })?,
            "-picker" => lua.create_function(|_lua, ()| Ok(true))?,
            "-commandview" => lua.create_function(|_lua, ()| Ok(false))?,
            _ => unreachable!(),
        };
        let udf_key = std::sync::Arc::new(lua.create_registry_value(use_dialog_fn)?);

        // change-project-folder
        let udf_key_c = udf_key.clone();
        cmds.set(
            format!("core:change-project-folder{suffix}"),
            lua.create_function(move |lua, ()| {
                let udf: LuaFunction = lua.registry_value(&udf_key_c)?;
                let use_dialog: bool = udf.call(())?;
                let callback = lua.create_function(|lua, abs_path: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let path: String = abs_path.get(1)?;
                    let root_project: LuaValue = core.get("root_project")?;
                    if let LuaValue::Function(f) = root_project {
                        let project: LuaValue = f.call(())?;
                        if let LuaValue::Table(p) = project {
                            let proj_path: String = p.get("path")?;
                            if path == proj_path {
                                return Ok(());
                            }
                        }
                    }
                    let confirm_fn = lua.create_function(|lua, dirpath: String| {
                        let core: LuaTable = require_table(lua, "core")?;
                        core.call_function::<()>("open_project", dirpath)
                    })?;
                    let docs: LuaTable = core.get("docs")?;
                    core.call_function::<()>("confirm_close_docs", (docs, confirm_fn, path))
                })?;
                open_directory(lua, "Change Project Folder", use_dialog, false, callback)
            })?,
        )?;

        // open-project-folder
        let udf_key_o = udf_key.clone();
        cmds.set(
            format!("core:open-project-folder{suffix}"),
            lua.create_function(move |lua, ()| {
                let udf: LuaFunction = lua.registry_value(&udf_key_o)?;
                let use_dialog: bool = udf.call(())?;
                let callback = lua.create_function(|lua, abs_path: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let path: String = abs_path.get(1)?;
                    let root_project: LuaValue = core.get("root_project")?;
                    if let LuaValue::Function(f) = root_project {
                        let project: LuaValue = f.call(())?;
                        if let LuaValue::Table(p) = project {
                            let proj_path: String = p.get("path")?;
                            if path == proj_path {
                                core.call_function::<()>(
                                    "error",
                                    format!("Directory {path:?} is currently opened"),
                                )?;
                                return Ok(());
                            }
                        }
                    }
                    let system: LuaTable = lua.globals().get("system")?;
                    let exefile: String = lua.globals().get("EXEFILE")?;
                    system.call_function::<()>("exec", format!("{exefile:?} {path:?}"))
                })?;
                open_directory(lua, "Open Project", use_dialog, false, callback)
            })?,
        )?;

        // add-directory
        cmds.set(
            format!("core:add-directory{suffix}"),
            lua.create_function({
                let udf_key_a = udf_key.clone();
                move |lua, ()| {
                    let udf: LuaFunction = lua.registry_value(&udf_key_a)?;
                    let use_dialog: bool = udf.call(())?;
                    let callback = lua.create_function(|lua, abs_path: LuaTable| {
                        let core: LuaTable = require_table(lua, "core")?;
                        let system: LuaTable = lua.globals().get("system")?;
                        for i in 1..=abs_path.raw_len() {
                            let dir: String = abs_path.get(i)?;
                            let abs: String = system.call_function("absolute_path", dir)?;
                            core.call_function::<()>("add_project", abs)?;
                        }
                        Ok(())
                    })?;
                    open_directory(lua, "Add Directory", use_dialog, true, callback)
                }
            })?,
        )?;
    }

    cmds.set(
        "core:close-project-folder",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let callback = lua.create_function(|lua, ()| {
                let core: LuaTable = require_table(lua, "core")?;
                core.call_function::<()>("close_project", ())?;
                let doc: LuaTable = core.call_function("open_doc", ())?;
                let root_view: LuaTable = core.get("root_view")?;
                root_view.call_method::<()>("open_doc", doc)
            })?;
            let docs: LuaTable = core.get("docs")?;
            core.call_function::<()>("confirm_close_docs", (docs, callback))
        })?,
    )?;

    cmds.set(
        "core:open-recent-folder",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "suggest",
                lua.create_function(|lua, text: String| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let recent: LuaValue = core
                        .get::<LuaValue>("recent_projects")
                        .unwrap_or(LuaValue::Nil);
                    let items = if matches!(recent, LuaValue::Table(_)) {
                        recent
                    } else {
                        LuaValue::Table(lua.create_table()?)
                    };
                    let expanded: String = common.call_function("home_expand", text)?;
                    let ranked = recent_items_fn(lua, items, expanded)?;
                    common.call_function::<LuaValue>("home_encode_list", ranked)
                })?,
            )?;
            opts.set(
                "submit",
                lua.create_function(|lua, text: String| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let path: String = common.call_function("home_expand", text)?;
                    let info: LuaValue = system.call_function("get_file_info", path.clone())?;
                    if let LuaValue::Table(t) = info {
                        let ftype: String = t.get("type")?;
                        if ftype == "dir" {
                            return core.call_function::<()>("open_project", path);
                        }
                    }
                    core.call_function::<()>("error", format!("Cannot open directory {path:?}"))
                })?,
            )?;
            opts.set(
                "validate",
                lua.create_function(|lua, text: String| {
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let expanded: String = common.call_function("home_expand", text)?;
                    let info: LuaValue = system.call_function("get_file_info", expanded)?;
                    match info {
                        LuaValue::Table(t) => {
                            let ftype: String = t.get("type")?;
                            Ok(ftype == "dir")
                        }
                        _ => Ok(false),
                    }
                })?,
            )?;
            command_view.call_method::<()>("enter", ("Recent Folder", opts))
        })?,
    )?;

    cmds.set(
        "core:remove-directory",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;

            // Build dir_list from core.projects[2..n] in reverse
            let projects: LuaTable = core.get("projects")?;
            let n = projects.raw_len() as i64;
            let dir_list = lua.create_table()?;
            for i in (2..=n).rev() {
                let project: LuaTable = projects.get(i)?;
                let name: String = project.get("name")?;
                dir_list.push(name)?;
            }

            let dir_list_key = std::sync::Arc::new(lua.create_registry_value(dir_list)?);
            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function({
                    let dk = dir_list_key.clone();
                    move |lua, (text, item): (String, LuaValue)| {
                        let core: LuaTable = require_table(lua, "core")?;
                        let common: LuaTable = require_table(lua, "core.common")?;
                        let _ = dk;
                        let raw = if let LuaValue::Table(ref t) = item {
                            t.get::<String>("text").unwrap_or(text)
                        } else {
                            text
                        };
                        let expanded: String = common.call_function("home_expand", raw)?;
                        let removed: bool =
                            core.call_function("remove_project", expanded.clone())?;
                        if !removed {
                            core.call_function::<()>(
                                "error",
                                format!("No directory {expanded:?} to be removed"),
                            )?;
                        }
                        Ok(())
                    }
                })?,
            )?;
            opts.set(
                "suggest",
                lua.create_function(move |lua, text: String| {
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let dir_list: LuaTable = lua.registry_value(&dir_list_key)?;
                    let expanded: String = common.call_function("home_expand", text)?;
                    let suggested: LuaValue =
                        common.call_function("dir_list_suggest", (expanded, dir_list))?;
                    common.call_function::<LuaValue>("home_encode_list", suggested)
                })?,
            )?;
            command_view.call_method::<()>("enter", ("Remove Directory", opts))
        })?,
    )?;

    add_fn.call::<()>((LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers the `core.commands.core` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.core",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
