use mlua::prelude::*;

/// Register `core.style` as a native Rust preload that builds the style table directly.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set("core.style", lua.create_function(loader)?)
}

/// Deep-copy a Lua value, recursing into tables.
fn copy_value(lua: &Lua, val: LuaValue) -> LuaResult<LuaValue> {
    match val {
        LuaValue::Table(t) => {
            let out = lua.create_table()?;
            for pair in t.pairs::<LuaValue, LuaValue>() {
                let (k, v) = pair?;
                out.set(k, copy_value(lua, v)?)?;
            }
            Ok(LuaValue::Table(out))
        }
        other => Ok(other),
    }
}

/// Deep-merge `override_tbl` into a copy of `base`, returning the merged table.
fn merge_tables(lua: &Lua, base: LuaValue, override_val: LuaValue) -> LuaResult<LuaTable> {
    let out = match copy_value(lua, base)? {
        LuaValue::Table(t) => t,
        _ => lua.create_table()?,
    };
    if let LuaValue::Table(ov) = override_val {
        for pair in ov.pairs::<LuaValue, LuaValue>() {
            let (k, v) = pair?;
            let existing = out.get::<LuaValue>(k.clone())?;
            if let (LuaValue::Table(_), LuaValue::Table(_)) = (&existing, &v) {
                let merged = merge_tables(lua, existing, v)?;
                out.set(k, merged)?;
            } else {
                out.set(k, copy_value(lua, v)?)?;
            }
        }
    }
    Ok(out)
}

/// Normalize a color value: accepts "#rrggbb", "#rrggbbaa", {r,g,b,a}, or {r=,g=,b=,a=}.
fn color_value(lua: &Lua, value: LuaValue) -> LuaResult<Option<LuaTable>> {
    match value {
        LuaValue::String(s) => {
            let common: LuaTable = {
                let require: LuaFunction = lua.globals().get("require")?;
                require.call("core.common")?
            };
            let color_fn: LuaFunction = common.get("color")?;
            let multi: LuaMultiValue = color_fn.call(s)?;
            let vals: Vec<LuaValue> = multi.into_vec();
            let t = lua.create_table()?;
            for (i, v) in vals.into_iter().enumerate() {
                t.set(i + 1, v)?;
            }
            Ok(Some(t))
        }
        LuaValue::Table(t) => {
            let first: LuaValue = t.get(1)?;
            if let LuaValue::Number(_) | LuaValue::Integer(_) = first {
                let r: LuaValue = t.get(1)?;
                let g: LuaValue = t.get(2)?;
                let b: LuaValue = t.get(3)?;
                let a: LuaValue = t.get(4)?;
                let a = match a {
                    LuaValue::Nil => LuaValue::Integer(0xff),
                    other => other,
                };
                let out = lua.create_table()?;
                out.set(1, r)?;
                out.set(2, g)?;
                out.set(3, b)?;
                out.set(4, a)?;
                return Ok(Some(out));
            }
            let r: LuaValue = t.get("r")?;
            if !r.is_nil() {
                let g: LuaValue = t.get("g")?;
                let b: LuaValue = t.get("b")?;
                let a: LuaValue = t.get("a")?;
                let a = match a {
                    LuaValue::Nil => LuaValue::Integer(0xff),
                    other => other,
                };
                let out = lua.create_table()?;
                out.set(1, r)?;
                out.set(2, g)?;
                out.set(3, b)?;
                out.set(4, a)?;
                return Ok(Some(out));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Set `target[key]` to the normalized color if value is a valid color.
fn apply_color(lua: &Lua, target: &LuaTable, key: &str, value: LuaValue) -> LuaResult<()> {
    if let Some(normalized) = color_value(lua, value)? {
        target.set(key, normalized)?;
    }
    Ok(())
}

/// Build the default UI dimensions table.
fn build_default_ui(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("divider_size", 1)?;
    t.set("scrollbar_size", 4)?;
    t.set("expanded_scrollbar_size", 12)?;
    t.set("minimum_thumb_size", 20)?;
    t.set("contracted_scrollbar_margin", 8)?;
    t.set("expanded_scrollbar_margin", 12)?;
    t.set("caret_width", 2)?;
    t.set("tab_width", 170)?;
    t.set("padding_x", 14)?;
    t.set("padding_y", 7)?;
    Ok(t)
}

/// Build the default font spec table.
fn build_default_fonts(lua: &Lua, datadir: &str) -> LuaResult<LuaTable> {
    let fonts = lua.create_table()?;

    // ui
    let ui = lua.create_table()?;
    ui.set("path", format!("{datadir}/fonts/Lilex-Regular.ttf"))?;
    ui.set("size", 15)?;
    ui.set("options", lua.create_table()?)?;
    fonts.set("ui", ui)?;

    // code
    let code = lua.create_table()?;
    code.set("path", format!("{datadir}/fonts/Lilex-Medium.ttf"))?;
    code.set("size", 15)?;
    code.set("options", lua.create_table()?)?;
    fonts.set("code", code)?;

    // big
    let big = lua.create_table()?;
    big.set("size", 46)?;
    big.set("options", lua.create_table()?)?;
    fonts.set("big", big)?;

    // icon
    let icon = lua.create_table()?;
    icon.set("path", format!("{datadir}/fonts/icons.ttf"))?;
    icon.set("size", 16)?;
    let icon_opts = lua.create_table()?;
    icon_opts.set("antialiasing", "grayscale")?;
    icon_opts.set("hinting", "full")?;
    icon.set("options", icon_opts)?;
    fonts.set("icon", icon)?;

    // icon_big
    let icon_big = lua.create_table()?;
    icon_big.set("size", 23)?;
    icon_big.set("options", lua.create_table()?)?;
    fonts.set("icon_big", icon_big)?;

    // syntax (empty)
    fonts.set("syntax", lua.create_table()?)?;

    Ok(fonts)
}

/// Load a single font via `renderer.font.load(path, size, options)`.
fn load_single_font(
    lua: &Lua,
    path: &str,
    size: f64,
    options: LuaTable,
) -> LuaResult<Option<LuaValue>> {
    let renderer: LuaTable = lua.globals().get("renderer")?;
    let font_mod: LuaTable = renderer.get("font")?;
    let load_fn: LuaFunction = font_mod.get("load")?;
    match load_fn.call::<LuaValue>((path, size, options)) {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None),
    }
}

/// Load a font from a spec merged with an optional fallback.
fn load_font_from_spec(lua: &Lua, spec: LuaValue, fallback: LuaValue) -> LuaResult<LuaValue> {
    let resolved = merge_tables(
        lua,
        copy_value(lua, fallback.clone())?,
        copy_value(lua, spec)?,
    )?;

    let scale: f64 = lua.globals().get("SCALE")?;
    let size_raw: f64 = resolved.get::<Option<f64>>("size")?.unwrap_or(14.0);
    let size = size_raw * scale;
    let options: LuaTable = match resolved.get::<LuaValue>("options")? {
        LuaValue::Table(t) => t,
        _ => lua.create_table()?,
    };

    let paths_val: LuaValue = resolved.get("paths")?;
    let path_list: Vec<String> = match paths_val {
        LuaValue::Table(t) => {
            let mut v = Vec::new();
            for val in t.sequence_values::<LuaValue>() {
                if let LuaValue::String(s) = val? {
                    let s = s.to_str()?.to_string();
                    if !s.is_empty() {
                        v.push(s);
                    }
                }
            }
            v
        }
        _ => {
            let single: LuaValue = resolved.get("path")?;
            if let LuaValue::String(s) = single {
                let s = s.to_str()?.to_string();
                if s.is_empty() { vec![] } else { vec![s] }
            } else {
                vec![]
            }
        }
    };

    let mut fonts: Vec<LuaValue> = Vec::new();
    for path in &path_list {
        let opts_copy = match copy_value(lua, LuaValue::Table(options.clone()))? {
            LuaValue::Table(t) => t,
            _ => lua.create_table()?,
        };
        if let Some(font) = load_single_font(lua, path, size, opts_copy)? {
            fonts.push(font);
        }
    }

    if fonts.is_empty() {
        if let LuaValue::Table(fb) = fallback {
            let fb_paths_val: LuaValue = fb.get("paths")?;
            let fb_paths: Vec<String> = match fb_paths_val {
                LuaValue::Table(t) => {
                    let mut v = Vec::new();
                    for val in t.sequence_values::<LuaValue>() {
                        if let LuaValue::String(s) = val? {
                            let s = s.to_str()?.to_string();
                            if !s.is_empty() {
                                v.push(s);
                            }
                        }
                    }
                    v
                }
                _ => {
                    let single: LuaValue = fb.get("path")?;
                    if let LuaValue::String(s) = single {
                        let s = s.to_str()?.to_string();
                        if s.is_empty() { vec![] } else { vec![s] }
                    } else {
                        vec![]
                    }
                }
            };
            for path in &fb_paths {
                let opts_copy = match copy_value(lua, LuaValue::Table(options.clone()))? {
                    LuaValue::Table(t) => t,
                    _ => lua.create_table()?,
                };
                if let Some(font) = load_single_font(lua, path, size, opts_copy)? {
                    fonts.push(font);
                }
            }
        }
    }

    if fonts.is_empty() {
        return Err(LuaError::runtime("unable to load configured font"));
    }

    if fonts.len() == 1 {
        Ok(fonts.into_iter().next().unwrap_or(LuaValue::Nil))
    } else {
        let renderer: LuaTable = lua.globals().get("renderer")?;
        let font_mod: LuaTable = renderer.get("font")?;
        let group_fn: LuaFunction = font_mod.get("group")?;
        let font_tbl = lua.create_table()?;
        for (i, f) in fonts.into_iter().enumerate() {
            font_tbl.set(i + 1, f)?;
        }
        group_fn.call(font_tbl)
    }
}

/// Resolve a lazy font spec, loading and caching on first access.
fn get_lazy_font(lua: &Lua, style: &LuaTable, name: &str) -> LuaResult<LuaValue> {
    let lazy_specs: LuaTable = style.get("_lazy_font_specs")?;
    let spec_entry: LuaValue = lazy_specs.get(name)?;
    if let LuaValue::Table(entry) = spec_entry {
        let spec: LuaValue = entry.get("spec")?;
        let fallback: LuaValue = entry.get("fallback")?;
        let font = load_font_from_spec(lua, spec, fallback)?;
        style.set(name, font.clone())?;
        lazy_specs.set(name, LuaValue::Nil)?;
        Ok(font)
    } else {
        style.get(name)
    }
}

const STYLE_COLOR_KEYS: &[&str] = &[
    "background",
    "background2",
    "background3",
    "text",
    "caret",
    "accent",
    "dim",
    "divider",
    "selection",
    "line_number",
    "line_number2",
    "line_highlight",
    "scrollbar",
    "scrollbar2",
    "scrollbar_track",
    "nagbar",
    "nagbar_text",
    "nagbar_dim",
    "drag_overlay",
    "drag_overlay_tab",
    "good",
    "warn",
    "error",
    "modified",
    "guide",
];

/// Apply theme colors to the style table from config and registered themes.
fn apply_theme_colors(lua: &Lua, style: &LuaTable) -> LuaResult<()> {
    let config: LuaTable = {
        let require: LuaFunction = lua.globals().get("require")?;
        require.call("core.config")?
    };
    let themes: LuaTable = style.get("themes")?;

    let theme_name: String = config
        .get::<Option<String>>("theme")?
        .unwrap_or_else(|| "default".to_string());

    // Try to load the theme module if not already registered
    let has_theme: bool = !themes.get::<LuaValue>(theme_name.as_str())?.is_nil();
    if !has_theme && theme_name != "default" {
        let require: LuaFunction = lua.globals().get("require")?;
        let _ = require.call::<LuaValue>(format!("colors.{theme_name}"));
    }

    let effective_name = if themes.get::<LuaValue>(theme_name.as_str())?.is_nil() {
        "default".to_string()
    } else {
        theme_name
    };

    let default_palette: LuaValue = themes.get("default")?;
    let theme_palette: LuaValue = themes.get(effective_name.as_str())?;
    let palette = merge_tables(lua, default_palette, theme_palette)?;

    let config_colors: LuaValue = config.get("colors")?;
    let colors = merge_tables(lua, LuaValue::Table(palette), config_colors)?;

    for key in STYLE_COLOR_KEYS {
        let val: LuaValue = colors.get(*key)?;
        apply_color(lua, style, key, val)?;
    }

    let syntax_tbl: LuaTable = style.get("syntax")?;
    let colors_syntax: LuaValue = colors.get("syntax")?;
    if let LuaValue::Table(cs) = colors_syntax {
        for pair in cs.pairs::<String, LuaValue>() {
            let (key, value) = pair?;
            apply_color(lua, &syntax_tbl, &key, value)?;
        }
    }

    let lint_tbl: LuaTable = style.get("lint")?;
    let colors_lint: LuaValue = colors.get("lint")?;
    if let LuaValue::Table(cl) = colors_lint {
        for pair in cl.pairs::<String, LuaValue>() {
            let (key, value) = pair?;
            apply_color(lua, &lint_tbl, &key, value)?;
        }
    }

    // Log levels
    let log_tbl: LuaTable = style.get("log")?;
    let style_text: LuaValue = style.get("text")?;
    let style_warn: LuaValue = style.get("warn")?;
    let style_error: LuaValue = style.get("error")?;

    let log_defaults: Vec<(&str, &str, LuaValue)> = vec![
        ("INFO", "i", style_text.clone()),
        ("WARN", "!", style_warn),
        ("ERROR", "!", style_error),
    ];

    let log_config: LuaValue = colors.get("log")?;
    let log_config_tbl = match &log_config {
        LuaValue::Table(t) => Some(t.clone()),
        _ => None,
    };

    for (level, default_icon, default_color) in &log_defaults {
        let default_entry = lua.create_table()?;
        default_entry.set("icon", *default_icon)?;
        default_entry.set("color", copy_value(lua, default_color.clone())?)?;

        let override_entry: LuaValue = if let Some(ref lc) = log_config_tbl {
            lc.get(*level)?
        } else {
            LuaValue::Nil
        };

        let entry = merge_tables(lua, LuaValue::Table(default_entry), override_entry)?;
        let result = lua.create_table()?;
        let icon: String = entry
            .get::<Option<String>>("icon")?
            .unwrap_or_else(|| default_icon.to_string());
        let entry_color: LuaValue = entry.get("color")?;
        let resolved_color = color_value(lua, entry_color)?
            .map(LuaValue::Table)
            .unwrap_or_else(|| default_color.clone());
        result.set("icon", icon)?;
        result.set("color", resolved_color)?;
        log_tbl.set(*level, result)?;
    }

    // Extra log levels from config not in defaults
    if let Some(lc) = log_config_tbl {
        for pair in lc.pairs::<String, LuaValue>() {
            let (level, entry_val) = pair?;
            if level == "INFO" || level == "WARN" || level == "ERROR" {
                continue;
            }
            if let LuaValue::Table(entry) = entry_val {
                let result = lua.create_table()?;
                let icon: String = entry
                    .get::<Option<String>>("icon")?
                    .unwrap_or_else(|| "?".to_string());
                let entry_color: LuaValue = entry.get("color")?;
                let resolved_color = color_value(lua, entry_color)?
                    .map(LuaValue::Table)
                    .unwrap_or_else(|| style_text.clone());
                result.set("icon", icon)?;
                result.set("color", resolved_color)?;
                log_tbl.set(level, result)?;
            }
        }
    }

    Ok(())
}

/// Round using the same logic as `common.round`.
fn round_scaled(val: f64, scale: f64) -> f64 {
    let n = val * scale;
    if n >= 0.0 {
        (n + 0.5).floor()
    } else {
        (n - 0.5).ceil()
    }
}

fn loader(lua: &Lua, _: ()) -> LuaResult<LuaValue> {
    let style = lua.create_table()?;

    style.set("themes", lua.create_table()?)?;
    style.set("syntax", lua.create_table()?)?;
    style.set("syntax_fonts", lua.create_table()?)?;
    style.set("log", lua.create_table()?)?;
    style.set("lint", lua.create_table()?)?;
    style.set("_lazy_font_specs", lua.create_table()?)?;

    let datadir: String = lua.globals().get("DATADIR")?;
    let default_fonts = build_default_fonts(lua, &datadir)?;
    let default_ui = build_default_ui(lua)?;

    // style.get_big_font()
    let style_ref = style.clone();
    style.set(
        "get_big_font",
        lua.create_function(move |lua, ()| get_lazy_font(lua, &style_ref, "big_font"))?,
    )?;

    // style.get_icon_big_font()
    let style_ref = style.clone();
    style.set(
        "get_icon_big_font",
        lua.create_function(move |lua, ()| get_lazy_font(lua, &style_ref, "icon_big_font"))?,
    )?;

    // style.register_theme(name, palette)
    let style_ref = style.clone();
    style.set(
        "register_theme",
        lua.create_function(move |_, (name, palette): (String, LuaTable)| {
            let themes: LuaTable = style_ref.get("themes")?;
            themes.set(name, palette)?;
            Ok(())
        })?,
    )?;

    // style.apply_theme()
    let style_ref = style.clone();
    style.set(
        "apply_theme",
        lua.create_function(move |lua, ()| {
            apply_theme_colors(lua, &style_ref)?;
            Ok(style_ref.clone())
        })?,
    )?;

    // style.apply_config()
    let style_ref = style.clone();
    let default_ui_key = lua.create_registry_value(default_ui)?;
    let default_fonts_key = lua.create_registry_value(default_fonts)?;
    style.set(
        "apply_config",
        lua.create_function(move |lua, ()| {
            let config: LuaTable = {
                let require: LuaFunction = lua.globals().get("require")?;
                require.call("core.config")?
            };
            let scale: f64 = lua.globals().get("SCALE")?;

            let default_ui: LuaTable = lua.registry_value(&default_ui_key)?;
            let config_ui: LuaValue = config.get("ui")?;
            let ui = merge_tables(lua, LuaValue::Table(default_ui), config_ui)?;

            let divider_size_raw: f64 = ui.get::<Option<f64>>("divider_size")?.unwrap_or(1.0);
            let divider_size = round_scaled(divider_size_raw, scale);
            style_ref.set("divider_size", divider_size)?;
            style_ref.set(
                "scrollbar_size",
                round_scaled(
                    ui.get::<Option<f64>>("scrollbar_size")?.unwrap_or(4.0),
                    scale,
                ),
            )?;
            style_ref.set(
                "expanded_scrollbar_size",
                round_scaled(
                    ui.get::<Option<f64>>("expanded_scrollbar_size")?
                        .unwrap_or(12.0),
                    scale,
                ),
            )?;
            style_ref.set(
                "minimum_thumb_size",
                round_scaled(
                    ui.get::<Option<f64>>("minimum_thumb_size")?.unwrap_or(20.0),
                    scale,
                ),
            )?;
            style_ref.set(
                "contracted_scrollbar_margin",
                round_scaled(
                    ui.get::<Option<f64>>("contracted_scrollbar_margin")?
                        .unwrap_or(8.0),
                    scale,
                ),
            )?;
            style_ref.set(
                "expanded_scrollbar_margin",
                round_scaled(
                    ui.get::<Option<f64>>("expanded_scrollbar_margin")?
                        .unwrap_or(12.0),
                    scale,
                ),
            )?;
            style_ref.set(
                "caret_width",
                round_scaled(ui.get::<Option<f64>>("caret_width")?.unwrap_or(2.0), scale),
            )?;
            style_ref.set(
                "tab_width",
                round_scaled(ui.get::<Option<f64>>("tab_width")?.unwrap_or(170.0), scale),
            )?;

            let padding = lua.create_table()?;
            padding.set(
                "x",
                round_scaled(ui.get::<Option<f64>>("padding_x")?.unwrap_or(14.0), scale),
            )?;
            padding.set(
                "y",
                round_scaled(ui.get::<Option<f64>>("padding_y")?.unwrap_or(7.0), scale),
            )?;
            style_ref.set("padding", padding)?;

            let margin = lua.create_table()?;
            let tab_margin = lua.create_table()?;
            let tab_top_margin: LuaValue = ui.get("tab_top_margin")?;
            let top_val = match tab_top_margin {
                LuaValue::Number(n) => n,
                LuaValue::Integer(n) => n as f64,
                _ => -divider_size_raw,
            };
            tab_margin.set("top", round_scaled(top_val, scale))?;
            margin.set("tab", tab_margin)?;
            style_ref.set("margin", margin)?;

            // Fonts
            let default_fonts: LuaTable = lua.registry_value(&default_fonts_key)?;
            let config_fonts: LuaValue = config.get("fonts")?;
            let fonts = merge_tables(lua, LuaValue::Table(default_fonts.clone()), config_fonts)?;

            let fonts_ui: LuaValue = fonts.get("ui")?;
            let default_ui_font: LuaValue = default_fonts.get("ui")?;
            style_ref.set("font", load_font_from_spec(lua, fonts_ui, default_ui_font)?)?;

            let fonts_code: LuaValue = fonts.get("code")?;
            let default_code_font: LuaValue = default_fonts.get("code")?;
            style_ref.set(
                "code_font",
                load_font_from_spec(lua, fonts_code, default_code_font)?,
            )?;

            let fonts_icon: LuaValue = fonts.get("icon")?;
            let default_icon_font: LuaValue = default_fonts.get("icon")?;
            style_ref.set(
                "icon_font",
                load_font_from_spec(lua, fonts_icon, default_icon_font)?,
            )?;

            style_ref.set("big_font", LuaValue::Nil)?;
            style_ref.set("icon_big_font", LuaValue::Nil)?;

            let lazy_specs: LuaTable = style_ref.get("_lazy_font_specs")?;

            let big_spec = lua.create_table()?;
            big_spec.set("spec", {
                let v: LuaValue = fonts.get("big")?;
                v
            })?;
            big_spec.set("fallback", {
                let v: LuaValue = fonts.get("ui")?;
                v
            })?;
            lazy_specs.set("big_font", big_spec)?;

            let icon_big_spec = lua.create_table()?;
            icon_big_spec.set("spec", {
                let v: LuaValue = fonts.get("icon_big")?;
                v
            })?;
            icon_big_spec.set("fallback", {
                let v: LuaValue = fonts.get("icon")?;
                v
            })?;
            lazy_specs.set("icon_big_font", icon_big_spec)?;

            // Syntax fonts
            let syntax_fonts_tbl: LuaTable = style_ref.get("syntax_fonts")?;
            let fonts_syntax: LuaValue = fonts.get("syntax")?;
            if let LuaValue::Table(fs) = fonts_syntax {
                let default_code: LuaValue = fonts.get("code")?;
                for pair in fs.pairs::<String, LuaValue>() {
                    let (token, font_spec) = pair?;
                    let font = load_font_from_spec(
                        lua,
                        font_spec,
                        copy_value(lua, default_code.clone())?,
                    )?;
                    syntax_fonts_tbl.set(token, font)?;
                }
            }

            apply_theme_colors(lua, &style_ref)?;

            Ok(style_ref.clone())
        })?,
    )?;

    Ok(LuaValue::Table(style))
}
