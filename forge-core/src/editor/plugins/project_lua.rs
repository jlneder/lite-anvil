use mlua::prelude::*;

/// Registers `core.project` -- Project class with file filtering and gitignore integration.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.project",
        lua.create_function(|lua, ()| {
            let object: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("core.object")?;
            let extend: LuaFunction = object.get("extend")?;
            let project: LuaTable = extend.call(&object)?;

            let common: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("core.common")?;
            let config: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("core.config")?;
            let gitignore: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("core.gitignore")?;
            let native_project_model: LuaTable = lua
                .globals()
                .get::<LuaFunction>("require")?
                .call("project_model")?;

            // ── compile_ignore_files ─────────────────────────────────────
            let compile_ignore_files = lua.create_function({
                let config = config.clone();
                move |lua, ()| {
                    let ipatterns_val: LuaValue = config.get("ignore_files")?;
                    let ipatterns: LuaTable = match ipatterns_val {
                        LuaValue::Table(t) => t,
                        LuaValue::String(s) => {
                            let t = lua.create_table()?;
                            t.raw_set(1, s)?;
                            t
                        }
                        _ => lua.create_table()?,
                    };
                    let compiled = lua.create_table()?;
                    let pcall_fn: LuaFunction = lua.globals().get("pcall")?;
                    let string_match: LuaFunction =
                        lua.globals().get::<LuaTable>("string")?.get("match")?;
                    let mut idx = 1i64;
                    for pair in ipatterns.sequence_values::<String>() {
                        let pattern = pair?;
                        let ok: bool =
                            pcall_fn.call((string_match.clone(), "a", pattern.clone()))?;
                        if ok {
                            let entry = lua.create_table()?;
                            // use_path: pattern contains a slash not at the end
                            let has_slash_not_end = pattern.contains('/') && {
                                let trimmed = pattern.trim_end_matches('$');
                                let trimmed = trimmed.trim_end_matches('/');
                                trimmed.contains('/')
                            };
                            entry.set("use_path", has_slash_not_end)?;
                            // match_dir: pattern ends with / or /$
                            let match_dir = pattern.ends_with('/') || pattern.ends_with("/$");
                            entry.set("match_dir", match_dir)?;
                            entry.set("pattern", pattern)?;
                            compiled.raw_set(idx, entry)?;
                            idx += 1;
                        }
                    }
                    Ok(compiled)
                }
            })?;

            // ── Project:new ──────────────────────────────────────────────
            project.set(
                "new",
                lua.create_function({
                    let common = common.clone();
                    let gitignore = gitignore.clone();
                    let compile_fn = compile_ignore_files.clone();
                    move |_, (this, path): (LuaTable, String)| {
                        this.set("path", path.clone())?;
                        let basename: String = common.call_function("basename", path.clone())?;
                        this.set("name", basename)?;
                        let compiled: LuaTable = compile_fn.call(())?;
                        this.set("compiled", compiled)?;
                        let git_root: LuaValue =
                            gitignore.call_function("find_root", path.clone())?;
                        if git_root.is_nil() {
                            this.set("git_root", path)?;
                        } else {
                            this.set("git_root", git_root)?;
                        }
                        Ok(this)
                    }
                })?,
            )?;

            // ── Project:absolute_path ────────────────────────────────────
            project.set(
                "absolute_path",
                lua.create_function({
                    let common = common.clone();
                    let native_project_model = native_project_model.clone();
                    move |lua, (this, filename): (LuaValue, String)| -> LuaResult<String> {
                        // this can be a table (self) or nil-like
                        let self_path: Option<String> = match &this {
                            LuaValue::Table(t) => t.get::<Option<String>>("path").ok().flatten(),
                            _ => None,
                        };
                        if let Some(ref path) = self_path {
                            let result: String = native_project_model
                                .call_function("absolute_path", (path.clone(), filename))?;
                            return Ok(result);
                        }
                        let is_abs: bool =
                            common.call_function("is_absolute_path", filename.clone())?;
                        if is_abs {
                            return common.call_function("normalize_path", filename);
                        }
                        if self_path.is_none() {
                            let system: LuaTable = lua.globals().get("system")?;
                            let abs_path: LuaFunction = system.get("absolute_path")?;
                            let cwd: String = abs_path.call(".")?;
                            let pathsep: String = lua.globals().get("PATHSEP")?;
                            let normalized: String =
                                common.call_function("normalize_path", filename)?;
                            return Ok(format!("{}{}{}", cwd, pathsep, normalized));
                        }
                        let path = self_path.unwrap_or_default();
                        let pathsep: String = lua.globals().get("PATHSEP")?;
                        Ok(format!("{}{}{}", path, pathsep, filename))
                    }
                })?,
            )?;

            // ── Project:normalize_path ───────────────────────────────────
            project.set(
                "normalize_path",
                lua.create_function({
                    let common = common.clone();
                    let native_project_model = native_project_model.clone();
                    move |_, (this, filename): (LuaTable, String)| -> LuaResult<String> {
                        let normalized: String =
                            common.call_function("normalize_path", filename)?;
                        let self_path: String = this.get("path")?;
                        let belongs: bool = common.call_function(
                            "path_belongs_to",
                            (normalized.clone(), self_path.clone()),
                        )?;
                        let final_name = if belongs {
                            common
                                .call_function("relative_path", (self_path.clone(), normalized))?
                        } else {
                            normalized
                        };
                        native_project_model
                            .call_function("normalize_path", (self_path, final_name))
                    }
                })?,
            )?;

            // ── fileinfo_pass_filter (helper) ────────────────────────────
            let fileinfo_pass_filter = lua.create_function({
                let config = config.clone();
                let common = common.clone();
                move |lua, (info, ignore_compiled): (LuaTable, LuaTable)| -> LuaResult<bool> {
                    let file_size_limit: f64 = config.get("file_size_limit")?;
                    let size: f64 = info.get::<Option<f64>>("size")?.unwrap_or(0.0);
                    if size >= file_size_limit * 1e6 {
                        return Ok(false);
                    }
                    let filename: String = info.get("filename")?;
                    let basename: String = common.call_function("basename", filename.clone())?;
                    let fullname = format!("/{}", filename.replace('\\', "/"));
                    let string_match: LuaFunction =
                        lua.globals().get::<LuaTable>("string")?.get("match")?;
                    let info_type: Option<String> = info.get("type")?;

                    for entry in ignore_compiled.sequence_values::<LuaTable>() {
                        let entry = entry?;
                        let use_path: bool =
                            entry.get::<Option<bool>>("use_path")?.unwrap_or(false);
                        let match_dir: bool =
                            entry.get::<Option<bool>>("match_dir")?.unwrap_or(false);
                        let pattern: String = entry.get("pattern")?;

                        let test = if use_path {
                            fullname.clone()
                        } else {
                            basename.clone()
                        };

                        if match_dir {
                            if info_type.as_deref() == Some("dir") {
                                let test_with_slash = format!("{}/", test);
                                let matched: LuaValue =
                                    string_match.call((test_with_slash, pattern))?;
                                if !matched.is_nil() {
                                    return Ok(false);
                                }
                            }
                        } else {
                            let matched: LuaValue = string_match.call((test, pattern.clone()))?;
                            if !matched.is_nil() {
                                return Ok(false);
                            }
                        }
                    }
                    Ok(true)
                }
            })?;
            lua.set_named_registry_value("project.fileinfo_pass_filter", fileinfo_pass_filter)?;

            // ── Project:is_ignored ───────────────────────────────────────
            project.set(
                "is_ignored",
                lua.create_function({
                    let config = config.clone();
                    let gitignore = gitignore.clone();
                    move |lua,
                          (this, info, path): (LuaTable, LuaValue, Option<String>)|
                          -> LuaResult<bool> {
                        let info_table = match info {
                            LuaValue::Table(ref t) => t,
                            _ => return Ok(false),
                        };
                        let info_type: Option<String> = info_table.get("type")?;
                        if info_type.is_none() {
                            return Ok(false);
                        }
                        if let Some(ref p) = path {
                            info_table.set("filename", p.clone())?;
                        }
                        let pass_filter: LuaFunction =
                            lua.named_registry_value("project.fileinfo_pass_filter")?;
                        let compiled: LuaTable = this.get("compiled")?;
                        let passes: bool = pass_filter.call((info_table, compiled))?;
                        if !passes {
                            return Ok(true);
                        }
                        let gitignore_config: LuaTable = config.get("gitignore")?;
                        let enabled: LuaValue = gitignore_config.get("enabled")?;
                        let gitignore_enabled = enabled != LuaValue::Boolean(false);
                        if let (true, Some(p)) = (gitignore_enabled, path) {
                            let git_root: LuaValue = this.get("git_root")?;
                            let self_path: String = this.get("path")?;
                            let root = if git_root.is_nil() {
                                self_path
                            } else {
                                match git_root {
                                    LuaValue::String(s) => s.to_str()?.to_string(),
                                    _ => self_path,
                                }
                            };
                            let matched: bool =
                                gitignore.call_function("match", (root, p, info_table))?;
                            return Ok(matched);
                        }
                        Ok(false)
                    }
                })?,
            )?;

            // ── Project:get_file_info ────────────────────────────────────
            project.set(
                "get_file_info",
                lua.create_function(
                    |lua, (this, path): (LuaTable, String)| -> LuaResult<LuaValue> {
                        let system: LuaTable = lua.globals().get("system")?;
                        let get_file_info: LuaFunction = system.get("get_file_info")?;
                        let info: LuaValue = get_file_info.call(path.clone())?;
                        let is_ignored: LuaFunction = this.get("is_ignored")?;
                        let ignored: bool = is_ignored.call((&this, &info, path))?;
                        if ignored { Ok(LuaValue::Nil) } else { Ok(info) }
                    },
                )?,
            )?;

            // ── Project:files ────────────────────────────────────────────
            // Returns a stateful iterator over (project, info) pairs.
            // Collects all files upfront to avoid yielding from Rust.
            project.set(
                "files",
                lua.create_function({
                    let config = config.clone();
                    let native_project_model = native_project_model.clone();
                    move |lua, this: LuaTable| {
                        let is_ignored: LuaFunction = this.get("is_ignored")?;
                        let self_path: String = this.get("path")?;

                        let file_size_limit: f64 = config.get("file_size_limit")?;
                        let project_scan: LuaTable = config.get("project_scan")?;
                        let max_files: LuaValue = project_scan.get("max_files")?;
                        let exclude_dirs: LuaValue = project_scan.get("exclude_dirs")?;

                        let opts = lua.create_table()?;
                        opts.set("max_size_bytes", file_size_limit * 1e6)?;
                        opts.set("max_files", max_files)?;
                        opts.set("exclude_dirs", exclude_dirs)?;

                        let get_files: LuaFunction = native_project_model.get("get_files")?;
                        let cached: LuaTable = get_files.call((self_path, opts))?;

                        // Build a table of non-ignored file info entries.
                        let results = lua.create_table()?;
                        let mut count = 0i64;
                        for pair in cached.sequence_values::<String>() {
                            let filename = pair?;
                            let info = lua.create_table()?;
                            info.set("type", "file")?;
                            info.set("size", 0)?;
                            info.set("filename", filename.clone())?;

                            let ignored: bool = is_ignored.call((&this, &info, filename))?;
                            if !ignored {
                                count += 1;
                                results.raw_set(count, info)?;
                            }
                        }

                        // Stateful iterator: index stored in a registry-backed table.
                        let state = lua.create_table()?;
                        state.set("idx", 0i64)?;
                        state.set("len", count)?;
                        let results_key = lua.create_registry_value(results)?;
                        let this_key = lua.create_registry_value(this)?;

                        let iterator =
                            lua.create_function(move |lua, ()| -> LuaResult<LuaMultiValue> {
                                let idx: i64 = state.get("idx")?;
                                let len: i64 = state.get("len")?;
                                let next = idx + 1;
                                if next > len {
                                    return Ok(LuaMultiValue::new());
                                }
                                state.set("idx", next)?;
                                let results: LuaTable = lua.registry_value(&results_key)?;
                                let info: LuaValue = results.raw_get(next)?;
                                let this: LuaTable = lua.registry_value(&this_key)?;
                                Ok(LuaMultiValue::from_vec(vec![LuaValue::Table(this), info]))
                            })?;
                        Ok(iterator)
                    }
                })?,
            )?;

            Ok(LuaValue::Table(project))
        })?,
    )
}
