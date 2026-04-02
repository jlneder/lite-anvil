use mlua::prelude::*;
use std::sync::Arc;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

fn set_config_defaults(lua: &Lua) -> LuaResult<()> {
    let config = require_table(lua, "core.config")?;
    let plugins: LuaTable = config.get("plugins")?;
    let common = require_table(lua, "core.common")?;

    let defaults = lua.create_table()?;
    defaults.set("enabled", false)?;
    defaults.set("width", 1)?;

    let spec = lua.create_table()?;
    spec.set("name", "Indent Guide")?;

    let enabled_entry = lua.create_table()?;
    enabled_entry.set("label", "Enabled")?;
    enabled_entry.set("description", "Draw vertical lines at each indentation level.")?;
    enabled_entry.set("path", "enabled")?;
    enabled_entry.set("type", "toggle")?;
    enabled_entry.set("default", true)?;
    spec.push(enabled_entry)?;

    let width_entry = lua.create_table()?;
    width_entry.set("label", "Width")?;
    width_entry.set("description", "Width in pixels of the indent guides.")?;
    width_entry.set("path", "width")?;
    width_entry.set("type", "number")?;
    width_entry.set("default", 1)?;
    width_entry.set("min", 1)?;
    spec.push(width_entry)?;

    defaults.set("config_spec", spec)?;

    let merged: LuaTable =
        common.call_function("merge", (defaults, plugins.get::<LuaValue>("indentguide")?))?;
    plugins.set("indentguide", merged)?;
    Ok(())
}

/// Computes the indentation level (number of leading indent units) for a line.
fn line_indent_level(line: &str, indent_size: usize) -> usize {
    let spaces: usize = line
        .chars()
        .take_while(|c| c.is_ascii_whitespace() && *c != '\n')
        .map(|c| if c == '\t' { indent_size } else { 1 })
        .sum();
    if spaces == 0 || indent_size == 0 {
        0
    } else {
        spaces / indent_size
    }
}

fn patch_draw_line_text(lua: &Lua) -> LuaResult<()> {
    let docview = require_table(lua, "core.docview")?;
    let old: LuaFunction = docview.get("draw_line_text")?;
    let old_key = Arc::new(lua.create_registry_value(old)?);

    docview.set(
        "draw_line_text",
        lua.create_function(move |lua, (this, line_idx, x, y): (LuaTable, i64, f64, f64)| {
            let config = require_table(lua, "core.config")?;
            let plugins: LuaTable = config.get("plugins")?;
            let ig: LuaValue = plugins.get("indentguide")?;

            if let LuaValue::Table(ref conf) = ig {
                let enabled: bool = conf.get("enabled").unwrap_or(false);
                let dv_class = require_table(lua, "core.docview")?;
                let getmetatable: LuaFunction = lua.globals().get("getmetatable")?;
                let mt: LuaValue = getmetatable.call(this.clone())?;
                let is_exact_dv = matches!(&mt, LuaValue::Table(t) if *t == dv_class);

                if enabled && is_exact_dv {
                    let doc: LuaTable = this.get("doc")?;
                    let lines: LuaTable = doc.get("lines")?;
                    let line_text: String = lines.get(line_idx).unwrap_or_default();

                    let indent_type: String = doc
                        .get::<String>("indent_info")
                        .or_else(|_| {
                            config
                                .get::<LuaTable>("indent_info")
                                .and_then(|t| t.get::<String>("type"))
                        })
                        .unwrap_or_else(|_| "soft".to_owned());

                    let indent_size: usize = doc
                        .get::<usize>("indent_size")
                        .or_else(|_| {
                            config
                                .get::<LuaTable>("indent_info")
                                .and_then(|t| t.get::<usize>("size"))
                        })
                        .unwrap_or(if indent_type == "hard" { 4 } else { 2 });

                    let levels = line_indent_level(&line_text, indent_size);

                    if levels > 0 {
                        let font: LuaValue = this.call_method("get_font", ())?;
                        let space_w: f64 = match &font {
                            LuaValue::Table(t) => t.call_method("get_width", " ")?,
                            LuaValue::UserData(ud) => ud.call_method("get_width", " ")?,
                            _ => 7.0,
                        };
                        let fh: f64 = match &font {
                            LuaValue::Table(t) => t.call_method("get_height", ())?,
                            LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                            _ => 14.0,
                        };
                        let guide_width: f64 = conf.get::<f64>("width").unwrap_or(1.0);
                        let style = require_table(lua, "core.style")?;
                        let color: LuaValue = {
                            let guide: LuaValue = style.get("guide")?;
                            if matches!(guide, LuaValue::Nil | LuaValue::Boolean(false)) {
                                style.get("selection")?
                            } else {
                                guide
                            }
                        };

                        let renderer: LuaTable = lua.globals().get("renderer")?;
                        let step = space_w * indent_size as f64;
                        for i in 0..levels {
                            let gx = x + step * i as f64;
                            renderer.call_function::<()>(
                                "draw_rect",
                                (gx, y, guide_width, fh, color.clone()),
                            )?;
                        }
                    }
                }
            }

            let old: LuaFunction = lua.registry_value(&old_key)?;
            old.call::<f64>((this, line_idx, x, y))
        })?,
    )?;
    Ok(())
}

fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command = require_table(lua, "core.command")?;
    let cmds = lua.create_table()?;
    cmds.set(
        "indent-guide:toggle",
        lua.create_function(|lua, ()| {
            let config = require_table(lua, "core.config")?;
            let plugins: LuaTable = config.get("plugins")?;
            let ig: LuaTable = plugins.get("indentguide")?;
            let enabled: bool = ig.get("enabled").unwrap_or(false);
            ig.set("enabled", !enabled)?;
            Ok(())
        })?,
    )?;
    command.call_function::<()>("add", (LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers `plugins.indentguide`: vertical indent level guides drawn in the editor.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.indentguide",
        lua.create_function(|lua, ()| {
            set_config_defaults(lua)?;
            patch_draw_line_text(lua)?;
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
