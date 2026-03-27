use mlua::prelude::*;

use std::sync::Arc;

/// Require a module by name, returning the loaded table.
fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers `core.emptyview` — the "Get Started" splash screen view.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.emptyview",
        lua.create_function(|lua, ()| {
            let view: LuaTable = require_table(lua, "core.view")?;
            let empty_view = view.call_method::<LuaTable>("extend", ())?;

            empty_view.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("EmptyView"))?,
            )?;

            // get_name()
            empty_view.set(
                "get_name",
                lua.create_function(|_lua, _this: LuaTable| Ok("Get Started"))?,
            )?;

            // get_filename()
            empty_view.set(
                "get_filename",
                lua.create_function(|_lua, _this: LuaTable| Ok(""))?,
            )?;

            // commands table
            let commands = lua.create_table()?;
            let cmd_data = [
                ("%s to run a command", "core:find-command"),
                ("%s for shortcuts", "core:show-shortcuts-help"),
                ("%s to open a file", "core:open-file"),
                ("%s to open a file from the project", "core:find-file"),
                ("%s to toggle focus mode", "root:toggle-focus-mode"),
                (
                    "%s to close the project folder",
                    "core:close-project-folder",
                ),
                ("%s to change project folder", "core:change-project-folder"),
                ("%s to open a project folder", "core:open-project-folder"),
            ];
            for (i, (fmt, cmd)) in cmd_data.iter().enumerate() {
                let entry = lua.create_table()?;
                entry.set("fmt", *fmt)?;
                entry.set("cmd", *cmd)?;
                commands.raw_set((i + 1) as i64, entry)?;
            }
            empty_view.set("commands", commands)?;

            let class_key = Arc::new(lua.create_registry_value(empty_view.clone())?);

            // draw(self)
            empty_view.set("draw", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let keymap: LuaTable = require_table(lua, "core.keymap")?;
                    let renderer: LuaTable = lua.globals().get("renderer")?;

                    let bg: LuaValue = style.get("background")?;
                    this.call_method::<()>("draw_background", bg)?;

                    let big_font: LuaValue = style.call_function("get_big_font", ())?;
                    let style_font: LuaValue = style.get("font")?;
                    let style_dim: LuaValue = style.get("dim")?;
                    let style_padding: LuaTable = style.get("padding")?;
                    let pad_x: f64 = style_padding.get("x")?;
                    let pad_y: f64 = style_padding.get("y")?;

                    let position: LuaTable = this.get("position")?;
                    let size: LuaTable = this.get("size")?;
                    let pos_x: f64 = position.get("x")?;
                    let pos_y: f64 = position.get("y")?;
                    let size_x: f64 = size.get("x")?;
                    let size_y: f64 = size.get("y")?;

                    let x = pos_x + size_x / 2.0;
                    let y = pos_y + size_y / 2.0;
                    let scale: f64 = lua.globals().get("SCALE")?;
                    let divider_w = (1.0 * scale).ceil();
                    let cmds_x = x + (divider_w / 2.0).ceil() + pad_x;
                    let logo_right_side = x - (divider_w / 2.0).ceil() - pad_x;

                    // Build displayed_cmds
                    let class: LuaTable = lua.registry_value(&k)?;
                    let cmds_tbl: LuaTable = class.get("commands")?;
                    // Use this instead of the class commands, since instances may have their own
                    let cmds_tbl: LuaTable = this
                        .get::<LuaValue>("commands")?
                        .as_table()
                        .cloned()
                        .unwrap_or(cmds_tbl);

                    let displayed = lua.create_table()?;
                    let mut displayed_count = 0i64;
                    for i in 1..=cmds_tbl.raw_len() as i64 {
                        let entry: LuaTable = cmds_tbl.raw_get(i)?;
                        let cmd: String = entry.get("cmd")?;
                        let keybinding: LuaValue =
                            keymap.call_function("get_binding_display", cmd)?;
                        if !matches!(keybinding, LuaValue::Nil) {
                            displayed_count += 1;
                            let d = lua.create_table()?;
                            let fmt: String = entry.get("fmt")?;
                            d.set("fmt", fmt)?;
                            d.set("keybinding", keybinding)?;
                            displayed.raw_set(displayed_count, d)?;
                        }
                    }

                    let font_h: f64 = match &style_font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let cmd_h = font_h + pad_y;
                    let cmds_y = y - (cmd_h * displayed_count as f64) / 2.0;

                    let draw_text: LuaFunction = renderer.get("draw_text")?;
                    let string_format: LuaFunction =
                        lua.globals().get::<LuaTable>("string")?.get("format")?;

                    for i in 1..=displayed_count {
                        let d: LuaTable = displayed.raw_get(i)?;
                        let fmt: String = d.get("fmt")?;
                        let kb: LuaValue = d.get("keybinding")?;
                        let cmd_text: String = string_format.call((fmt, kb))?;
                        draw_text.call::<()>((
                            style_font.clone(),
                            cmd_text,
                            cmds_x,
                            cmds_y + cmd_h * (i - 1) as f64,
                            style_dim.clone(),
                        ))?;
                    }

                    let title = "Lite-Anvil";
                    let version: String = lua.globals().get("VERSION")?;

                    let big_font_h: f64 = match &big_font {
                        LuaValue::Table(t) => t.call_method("get_height", title.to_string())?,
                        LuaValue::UserData(ud) => {
                            ud.call_method("get_height", title.to_string())?
                        }
                        _ => 28.0,
                    };
                    let logo_y = y - big_font_h + big_font_h / 4.0;

                    let big_font_w: f64 = match &big_font {
                        LuaValue::Table(t) => t.call_method("get_width", title.to_string())?,
                        LuaValue::UserData(ud) => ud.call_method("get_width", title.to_string())?,
                        _ => 100.0,
                    };
                    let logo_x = logo_right_side - big_font_w;

                    let vers_w: f64 = match &style_font {
                        LuaValue::Table(t) => t.call_method("get_width", version.clone())?,
                        LuaValue::UserData(ud) => ud.call_method("get_width", version.clone())?,
                        _ => 40.0,
                    };
                    let vers_x = logo_right_side - vers_w;
                    let vers_y = y + big_font_h / 8.0;

                    draw_text.call::<()>((
                        big_font.clone(),
                        title,
                        logo_x,
                        logo_y,
                        style_dim.clone(),
                    ))?;
                    draw_text.call::<()>((
                        style_font.clone(),
                        version,
                        vers_x,
                        vers_y,
                        style_dim.clone(),
                    ))?;

                    let divider_y = cmds_y.min(logo_y) - pad_y;
                    let divider_h = (y - divider_y) * 2.0;
                    let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                    draw_rect.call::<()>((
                        x - divider_w / 2.0,
                        divider_y,
                        divider_w,
                        divider_h,
                        style_dim,
                    ))?;

                    Ok(())
                })?
            })?;

            Ok(LuaValue::Table(empty_view))
        })?,
    )
}
