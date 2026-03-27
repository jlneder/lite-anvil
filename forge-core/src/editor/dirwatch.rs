use mlua::prelude::*;

/// Registers `core.dirwatch` — directory monitoring using the native project_fs backend.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.dirwatch",
        lua.create_function(|lua, ()| {
            let dirwatch = lua.create_table()?;

            // dirwatch.__index
            let dw_ref = lua.create_registry_value(dirwatch.clone())?;
            dirwatch.set(
                "__index",
                lua.create_function(move |lua, (self_tbl, idx): (LuaTable, LuaValue)| {
                    let val: LuaValue = self_tbl.raw_get(idx.clone())?;
                    if val != LuaNil {
                        return Ok(val);
                    }
                    let dw: LuaTable = lua.registry_value(&dw_ref)?;
                    dw.raw_get::<LuaValue>(idx)
                })?,
            )?;

            // dirwatch.new() -> dirwatch instance
            dirwatch.set(
                "new",
                lua.create_function(|lua, ()| {
                    let t = lua.create_table()?;
                    t.set("scanned", lua.create_table()?)?;
                    t.set("watched", lua.create_table()?)?;
                    t.set("reverse_watched", lua.create_table()?)?;
                    t.set("native_watches", lua.create_table()?)?;
                    let dirmonitor: LuaTable = lua.globals().get("dirmonitor")?;
                    let new_fn: LuaFunction = dirmonitor.get("new")?;
                    let monitor: LuaValue = new_fn.call(())?;
                    t.set("monitor", monitor)?;
                    t.set("single_watch_top", LuaNil)?;
                    t.set("single_watch_count", 0)?;
                    let dw: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core.dirwatch")?;
                    t.set_metatable(Some(dw))?;
                    Ok(t)
                })?,
            )?;

            // dirwatch:scan(path, unwatch?)
            dirwatch.set(
                "scan",
                lua.create_function(
                    |_lua, (self_tbl, path, unwatch): (LuaTable, LuaString, Option<bool>)| {
                        if unwatch == Some(false) {
                            self_tbl.call_method::<LuaValue>("unwatch", path)?;
                        } else {
                            self_tbl.call_method::<LuaValue>("watch", (path, LuaNil))?;
                        }
                        Ok(())
                    },
                )?,
            )?;

            // dirwatch:watch(path, unwatch?)
            dirwatch.set(
                "watch",
                lua.create_function(
                    |lua, (self_tbl, path, unwatch): (LuaTable, LuaString, Option<bool>)| {
                        if unwatch == Some(false) {
                            return self_tbl.call_method::<LuaValue>("unwatch", path);
                        }
                        let system: LuaTable = lua.globals().get("system")?;
                        let get_file_info: LuaFunction = system.get("get_file_info")?;
                        let info: LuaValue = get_file_info.call(path.clone())?;
                        let info = match info {
                            LuaValue::Table(t) => t,
                            _ => return Ok(LuaNil),
                        };
                        let native_watches: LuaTable = self_tbl.get("native_watches")?;
                        let existing: LuaValue = native_watches.get(path.clone())?;
                        if existing == LuaNil {
                            let project_fs: LuaTable = lua
                                .globals()
                                .get::<LuaTable>("package")?
                                .get::<LuaTable>("loaded")?
                                .get("project_fs")?;
                            let watch_fn: LuaFunction = project_fs.get("watch_project")?;
                            let result: LuaResult<LuaValue> = watch_fn.call(path.clone());
                            if let Ok(watch_id) = result {
                                if watch_id != LuaNil {
                                    let entry = lua.create_table()?;
                                    entry.set("id", watch_id)?;
                                    let file_type: LuaValue = info.get("type")?;
                                    entry.set("type", file_type)?;
                                    native_watches.set(path, entry)?;
                                } else {
                                    let scanned: LuaTable = self_tbl.get("scanned")?;
                                    let modified: LuaValue = info.get("modified")?;
                                    scanned.set(path, modified)?;
                                }
                            } else {
                                let scanned: LuaTable = self_tbl.get("scanned")?;
                                let modified: LuaValue = info.get("modified")?;
                                scanned.set(path, modified)?;
                            }
                        }
                        Ok(LuaNil)
                    },
                )?,
            )?;

            // dirwatch:unwatch(directory)
            dirwatch.set(
                "unwatch",
                lua.create_function(|lua, (self_tbl, directory): (LuaTable, LuaString)| {
                    let native_watches: LuaTable = self_tbl.get("native_watches")?;
                    let existing: LuaValue = native_watches.get(directory.clone())?;
                    if let LuaValue::Table(entry) = existing {
                        let project_fs: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("project_fs")?;
                        let cancel_fn: LuaFunction = project_fs.get("cancel_watch")?;
                        let id: LuaValue = entry.get("id")?;
                        cancel_fn.call::<LuaValue>(id)?;
                        native_watches.set(directory, LuaNil)?;
                        return Ok(());
                    }
                    let scanned: LuaTable = self_tbl.get("scanned")?;
                    let val: LuaValue = scanned.get(directory.clone())?;
                    if val != LuaNil {
                        scanned.set(directory, LuaNil)?;
                    }
                    Ok(())
                })?,
            )?;

            // dirwatch:check(change_callback, scan_time?, wait_time?) -> bool
            dirwatch.set(
                "check",
                lua.create_function(
                    |lua,
                     (self_tbl, change_callback, _scan_time, _wait_time): (
                        LuaTable,
                        LuaFunction,
                        Option<f64>,
                        Option<f64>,
                    )| {
                        let mut had_change = false;
                        let delivered = lua.create_table()?;
                        let native_watches: LuaTable = self_tbl.get("native_watches")?;
                        let project_fs: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("project_fs")?;
                        let poll_fn: LuaFunction = project_fs.get("poll_changes")?;

                        for pair in native_watches.pairs::<LuaValue, LuaTable>() {
                            let (path, watch) = pair?;
                            let id: LuaValue = watch.get("id")?;
                            let result: LuaResult<LuaValue> = poll_fn.call(id);
                            if let Ok(LuaValue::Table(changes)) = result {
                                let watch_type: String =
                                    watch.get::<String>("type").unwrap_or_default();
                                for entry in changes.sequence_values::<LuaValue>() {
                                    let changed = entry?;
                                    let target = path.clone();
                                    if watch_type == "file" {
                                        // For files, only deliver if the changed path matches
                                        let already: bool =
                                            delivered.get(target.clone()).unwrap_or(false);
                                        if !already {
                                            delivered.set(target.clone(), true)?;
                                            change_callback.call::<()>(target)?;
                                            had_change = true;
                                        }
                                    } else {
                                        let already: bool =
                                            delivered.get(target.clone()).unwrap_or(false);
                                        if !already {
                                            delivered.set(target.clone(), true)?;
                                            change_callback.call::<()>(target)?;
                                            had_change = true;
                                        }
                                    }
                                    let _ = changed;
                                }
                            }
                        }

                        let system: LuaTable = lua.globals().get("system")?;
                        let scanned: LuaTable = self_tbl.get("scanned")?;
                        let get_file_info: LuaFunction = system.get("get_file_info")?;

                        // Process all scanned entries without yielding.
                        let entries: Vec<(LuaValue, LuaValue)> = scanned
                            .pairs::<LuaValue, LuaValue>()
                            .filter_map(|r| r.ok())
                            .collect();

                        for (directory, old_modified) in entries {
                            if old_modified != LuaNil {
                                let info: LuaValue = get_file_info.call(directory.clone())?;
                                let new_modified = if let LuaValue::Table(ref t) = info {
                                    t.get::<LuaValue>("modified")?
                                } else {
                                    LuaNil
                                };
                                if old_modified != new_modified {
                                    change_callback.call::<()>(directory.clone())?;
                                    had_change = true;
                                    scanned.set(directory, new_modified)?;
                                }
                            }
                        }

                        Ok(had_change)
                    },
                )?,
            )?;

            Ok(LuaValue::Table(dirwatch))
        })?,
    )
}
