use mlua::prelude::*;

/// Register `core.config` as a native Rust preload that builds the config table directly.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.config", lua.create_function(loader)?)
}

fn loader(lua: &Lua, _: ()) -> LuaResult<LuaValue> {
    let globals = lua.globals();
    let scale: f64 = globals.get("SCALE")?;
    let platform: String = globals.get("PLATFORM")?;
    let datadir: String = globals.get("DATADIR")?;

    let config = lua.create_table()?;

    config.set("fps", 60)?;
    config.set("max_log_items", 800)?;
    config.set("message_timeout", 5)?;
    config.set("mouse_wheel_scroll", 50.0 * scale)?;
    config.set("animate_drag_scroll", false)?;
    config.set("scroll_past_end", true)?;
    config.set("force_scrollbar_status", false)?;
    config.set("file_size_limit", 10)?;

    // large_file
    let large_file = lua.create_table()?;
    large_file.set("soft_limit_mb", 20)?;
    large_file.set("hard_limit_mb", 128)?;
    large_file.set("read_only", true)?;
    large_file.set("plain_text", true)?;
    large_file.set("disable_lsp", true)?;
    large_file.set("disable_autocomplete", true)?;
    config.set("large_file", large_file)?;

    // project_scan
    let project_scan = lua.create_table()?;
    project_scan.set("max_files", 50000)?;
    let exclude_dirs = lua.create_table()?;
    exclude_dirs.set(1, "__pycache__")?;
    project_scan.set("exclude_dirs", exclude_dirs)?;
    config.set("project_scan", project_scan)?;

    // ignore_files
    let ignore_files = lua.create_sequence_from([
        // folders
        "^%.svn/",
        "^%.git/",
        "^%.hg/",
        "^CVS/",
        "^%.Trash/",
        "^%.Trash%-.*/",
        "^node_modules/",
        "^%.cache/",
        "^__pycache__/",
        // files
        "%.pyc$",
        "%.pyo$",
        "%.exe$",
        "%.dll$",
        "%.obj$",
        "%.o$",
        "%.a$",
        "%.lib$",
        "%.so$",
        "%.dylib$",
        "%.ncb$",
        "%.sdf$",
        "%.suo$",
        "%.pdb$",
        "%.idb$",
        "%.class$",
        "%.psd$",
        "%.db$",
        "^desktop%.ini$",
        "^%.DS_Store$",
        "^%.directory$",
    ])?;
    config.set("ignore_files", ignore_files)?;

    config.set("symbol_pattern", "[%a_][%w_]*")?;
    config.set("non_word_chars", " \t\n/\\()\"':,.;<>~!@#$%^&*|+=[]{}`?-")?;
    config.set("undo_merge_timeout", 0.3)?;
    config.set("max_undos", 10000)?;
    config.set("max_tabs", 8)?;
    config.set("max_visible_commands", 10)?;
    config.set("always_show_tabs", true)?;
    config.set("highlight_current_line", true)?;
    config.set("line_height", 1.2)?;
    config.set("indent_size", 2)?;
    config.set("tab_type", "soft")?;
    config.set("keep_newline_whitespace", false)?;

    let line_endings = if platform == "Windows" { "crlf" } else { "lf" };
    config.set("line_endings", line_endings)?;

    config.set("line_limit", 80)?;
    config.set("theme", "dark_default")?;

    // gitignore
    let gitignore = lua.create_table()?;
    gitignore.set("enabled", true)?;
    gitignore.set("additional_patterns", lua.create_table()?)?;
    config.set("gitignore", gitignore)?;

    // lsp
    let lsp = lua.create_table()?;
    lsp.set("load_on_startup", true)?;
    lsp.set("semantic_highlighting", true)?;
    lsp.set("inline_diagnostics", true)?;
    lsp.set("format_on_save", true)?;
    config.set("lsp", lsp)?;

    // native_tokenizer
    let native_tokenizer = lua.create_table()?;
    native_tokenizer.set("enabled", true)?;
    config.set("native_tokenizer", native_tokenizer)?;

    // terminal
    let terminal = lua.create_table()?;
    terminal.set("placement", "bottom")?;
    terminal.set("reuse_mode", "pane")?;
    config.set("terminal", terminal)?;

    // ui
    let ui = lua.create_table()?;
    ui.set("divider_size", 1)?;
    ui.set("scrollbar_size", 4)?;
    ui.set("expanded_scrollbar_size", 12)?;
    ui.set("minimum_thumb_size", 20)?;
    ui.set("contracted_scrollbar_margin", 8)?;
    ui.set("expanded_scrollbar_margin", 12)?;
    ui.set("caret_width", 2)?;
    ui.set("tab_width", 170)?;
    ui.set("padding_x", 14)?;
    ui.set("padding_y", 7)?;
    config.set("ui", ui)?;

    // fonts
    let fonts = lua.create_table()?;
    {
        let ui_font = lua.create_table()?;
        ui_font.set("path", format!("{datadir}/fonts/Lilex-Regular.ttf"))?;
        ui_font.set("size", 15)?;
        ui_font.set("options", lua.create_table()?)?;
        fonts.set("ui", ui_font)?;

        let code_font = lua.create_table()?;
        code_font.set("path", format!("{datadir}/fonts/Lilex-Medium.ttf"))?;
        code_font.set("size", 15)?;
        code_font.set("options", lua.create_table()?)?;
        fonts.set("code", code_font)?;

        let big_font = lua.create_table()?;
        big_font.set("size", 46)?;
        big_font.set("options", lua.create_table()?)?;
        fonts.set("big", big_font)?;

        let icon_font = lua.create_table()?;
        icon_font.set("path", format!("{datadir}/fonts/icons.ttf"))?;
        icon_font.set("size", 16)?;
        let icon_opts = lua.create_table()?;
        icon_opts.set("antialiasing", "grayscale")?;
        icon_opts.set("hinting", "full")?;
        icon_font.set("options", icon_opts)?;
        fonts.set("icon", icon_font)?;

        let icon_big_font = lua.create_table()?;
        icon_big_font.set("size", 23)?;
        icon_big_font.set("options", lua.create_table()?)?;
        fonts.set("icon_big", icon_big_font)?;

        fonts.set("syntax", lua.create_table()?)?;
    }
    config.set("fonts", fonts)?;

    // colors
    let colors = lua.create_table()?;
    colors.set("syntax", lua.create_table()?)?;
    colors.set("log", lua.create_table()?)?;
    colors.set("lint", lua.create_table()?)?;
    config.set("colors", colors)?;

    config.set("long_line_indicator", false)?;
    config.set("long_line_indicator_width", 1)?;
    config.set("transitions", true)?;

    // disabled_transitions
    let disabled_transitions = lua.create_table()?;
    disabled_transitions.set("scroll", false)?;
    disabled_transitions.set("commandview", false)?;
    disabled_transitions.set("contextmenu", false)?;
    disabled_transitions.set("logview", false)?;
    disabled_transitions.set("nagbar", false)?;
    disabled_transitions.set("tabs", false)?;
    disabled_transitions.set("tab_drag", false)?;
    disabled_transitions.set("statusbar", false)?;
    config.set("disabled_transitions", disabled_transitions)?;

    config.set("animation_rate", 1.0)?;
    config.set("blink_period", 0.8)?;
    config.set("disable_blink", false)?;
    config.set("draw_whitespace", false)?;
    config.set("borderless", false)?;
    config.set("tab_close_button", true)?;
    config.set("max_clicks", 3)?;
    config.set("skip_plugins_version", false)?;
    config.set("stonks", true)?;

    // use_system_file_picker = system.get_sandbox() ~= "none"
    let system_table: LuaTable = globals.get("system")?;
    let get_sandbox: LuaFunction = system_table.get("get_sandbox")?;
    let sandbox: String = get_sandbox.call(())?;
    config.set("use_system_file_picker", sandbox != "none")?;

    // config.plugins with metatable
    build_plugins_table(lua, &config)?;

    Ok(LuaValue::Table(config))
}

/// Build the `config.plugins` table with its `__index`/`__newindex`/`__pairs` metatable.
fn build_plugins_table(lua: &Lua, config: &LuaTable) -> LuaResult<()> {
    let plugins_config = lua.create_table()?;
    lua.set_named_registry_value("_config_plugins_inner", plugins_config)?;

    let plugins = lua.create_table()?;

    let mt = lua.create_table()?;

    // __index: lazy-create plugin entry, return config sub-table or false
    mt.set(
        "__index",
        lua.create_function(|lua, (_t, k): (LuaTable, String)| {
            let inner: LuaTable = lua.named_registry_value("_config_plugins_inner")?;
            if inner.get::<LuaValue>(k.as_str())?.is_nil() {
                let entry = lua.create_table()?;
                entry.set("enabled", true)?;
                entry.set("config", lua.create_table()?)?;
                inner.set(k.as_str(), entry)?;
            }
            let entry: LuaTable = inner.get(k.as_str())?;
            let enabled: LuaValue = entry.get("enabled")?;
            if enabled == LuaValue::Boolean(false) {
                return Ok(LuaValue::Boolean(false));
            }
            entry.get("config")
        })?,
    )?;

    // __newindex: handle plugin enable/disable and config merging
    mt.set(
        "__newindex",
        lua.create_function(|lua, (_t, k, v): (LuaTable, String, LuaValue)| {
            let inner: LuaTable = lua.named_registry_value("_config_plugins_inner")?;
            if inner.get::<LuaValue>(k.as_str())?.is_nil() {
                let entry = lua.create_table()?;
                entry.set("enabled", LuaValue::Nil)?;
                entry.set("config", lua.create_table()?)?;
                inner.set(k.as_str(), entry)?;
            }
            let entry: LuaTable = inner.get(k.as_str())?;

            if v == LuaValue::Boolean(false) {
                // check if plugin is already loaded
                let loaded: LuaTable = lua.globals().get::<LuaTable>("package")?.get("loaded")?;
                let key = format!("plugins.{k}");
                if !loaded.get::<LuaValue>(key.as_str())?.is_nil() {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let warn: LuaFunction = core.get("warn")?;
                    warn.call::<()>((format!(
                        "[{k}] is already enabled, restart the editor for the change to take effect"
                    ),))?;
                    return Ok(());
                }
                entry.set("enabled", false)?;
            } else {
                let cur_enabled: LuaValue = entry.get("enabled")?;
                if cur_enabled == LuaValue::Boolean(false) && v != LuaValue::Boolean(false) {
                    entry.set("enabled", true)?;
                }
                if let LuaValue::Table(tbl) = v {
                    entry.set("enabled", true)?;
                    let common: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core.common")?;
                    let merge: LuaFunction = common.get("merge")?;
                    let existing: LuaTable = entry.get("config")?;
                    let merged: LuaTable = merge.call((existing, tbl))?;
                    entry.set("config", merged)?;
                }
            }
            Ok(())
        })?,
    )?;

    // __pairs: iterate over all known plugin configs
    mt.set(
        "__pairs",
        lua.create_function(|lua, _t: LuaTable| {
            let inner: LuaTable = lua.named_registry_value("_config_plugins_inner")?;
            let inner_key = lua.create_registry_value(inner)?;
            let next_key_reg = std::cell::RefCell::new(lua.create_registry_value(LuaValue::Nil)?);
            let iter = lua.create_function(move |lua, ()| {
                let inner: LuaTable = lua.registry_value(&inner_key)?;
                let current_key: LuaValue = lua.registry_value(&next_key_reg.borrow())?;
                let next_fn: LuaFunction = lua.globals().get("next")?;
                let result: LuaMultiValue = next_fn.call((inner, current_key))?;
                let vals: Vec<LuaValue> = result.into_iter().collect();
                let key = vals.first().cloned().unwrap_or(LuaValue::Nil);
                if key.is_nil() {
                    return Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]));
                }
                let value = vals.get(1).cloned().unwrap_or(LuaValue::Nil);
                lua.replace_registry_value(&mut next_key_reg.borrow_mut(), key.clone())?;
                let config: LuaValue = if let LuaValue::Table(ref entry) = value {
                    entry.get("config")?
                } else {
                    LuaValue::Nil
                };
                Ok(LuaMultiValue::from_vec(vec![key, config]))
            })?;
            Ok(iter)
        })?,
    )?;

    plugins.set_metatable(Some(mt))?;
    config.set("plugins", plugins)?;
    Ok(())
}
