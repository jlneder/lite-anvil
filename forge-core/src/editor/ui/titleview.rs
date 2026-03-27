use mlua::prelude::*;

use std::sync::Arc;

/// Require a module by name, returning the loaded table.
fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers `core.titleview` — window title bar with controls.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.titleview",
        lua.create_function(|lua, ()| {
            let view_class: LuaTable = require_table(lua, "core.view")?;
            let title_view = view_class.call_method::<LuaTable>("extend", ())?;

            title_view.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("TitleView"))?,
            )?;

            // icon_colors table — common.color returns multivalue (r,g,b,a); package as table.
            let icon_colors = lua.create_table()?;
            let common: LuaTable = require_table(lua, "core.common")?;
            let color_fn: LuaFunction = common.get("color")?;
            let pack_color = |lua: &Lua, hex: &str| -> LuaResult<LuaTable> {
                let mv: LuaMultiValue = color_fn.call(hex)?;
                let t = lua.create_table()?;
                for (i, v) in mv.into_vec().into_iter().enumerate() {
                    t.set(i + 1, v)?;
                }
                Ok(t)
            };
            icon_colors.set("bg", pack_color(lua, "#2e2e32ff")?)?;
            icon_colors.set("color6", pack_color(lua, "#e1e1e6ff")?)?;
            icon_colors.set("color7", pack_color(lua, "#ffa94dff")?)?;
            icon_colors.set("color8", pack_color(lua, "#93ddfaff")?)?;
            icon_colors.set("color9", pack_color(lua, "#f7c95cff")?)?;

            let icon_colors_key = Arc::new(lua.create_registry_value(icon_colors)?);

            // restore_command and maximize_command
            let restore_command = lua.create_table()?;
            restore_command.set("symbol", "w")?;
            restore_command.set(
                "action",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let window: LuaValue = core.get("window")?;
                    system.call_function::<()>("set_window_mode", (window, "normal"))
                })?,
            )?;

            let maximize_command = lua.create_table()?;
            maximize_command.set("symbol", "W")?;
            maximize_command.set(
                "action",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let window: LuaValue = core.get("window")?;
                    system.call_function::<()>("set_window_mode", (window, "maximized"))
                })?,
            )?;

            // title_commands table
            let title_commands = lua.create_table()?;
            let minimize = lua.create_table()?;
            minimize.set("symbol", "_")?;
            minimize.set(
                "action",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let window: LuaValue = core.get("window")?;
                    system.call_function::<()>("set_window_mode", (window, "minimized"))
                })?,
            )?;
            title_commands.raw_set(1, minimize)?;
            title_commands.raw_set(2, maximize_command.clone())?;
            let close = lua.create_table()?;
            close.set("symbol", "X")?;
            close.set(
                "action",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = require_table(lua, "core")?;
                    core.call_function::<()>("quit", ())
                })?,
            )?;
            title_commands.raw_set(3, close)?;

            let cmds_key = Arc::new(lua.create_registry_value(title_commands)?);
            let restore_key = Arc::new(lua.create_registry_value(restore_command)?);
            let maximize_key = Arc::new(lua.create_registry_value(maximize_command)?);
            let class_key = Arc::new(lua.create_registry_value(title_view.clone())?);

            // TitleView:new()
            title_view.set("new", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let class: LuaTable = lua.registry_value(&k)?;
                    let super_tbl: LuaTable = class.get("super")?;
                    let super_new: LuaFunction = super_tbl.get("new")?;
                    super_new.call::<()>(this.clone())?;
                    this.set("visible", true)?;
                    this.set("_control_metrics", LuaValue::Nil)?;
                    Ok(())
                })?
            })?;

            // title_view_height helper (stored as method for convenience)
            let title_view_height = lua.create_function(|lua, ()| {
                let style: LuaTable = require_table(lua, "core.style")?;
                let font: LuaValue = style.get("font")?;
                let fh: f64 = match &font {
                    LuaValue::Table(t) => t.call_method("get_height", ())?,
                    LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                    _ => 14.0,
                };
                let padding: LuaTable = style.get("padding")?;
                let py: f64 = padding.get("y")?;
                Ok(fh + py * 2.0)
            })?;
            let tvh_key = Arc::new(lua.create_registry_value(title_view_height)?);

            // title_separator_inset helper
            let title_sep_inset = lua.create_function(|lua, ()| {
                let style: LuaTable = require_table(lua, "core.style")?;
                let padding: LuaTable = style.get("padding")?;
                let px: f64 = padding.get("x")?;
                Ok(10.0_f64.max(px - 2.0))
            })?;
            let tsi_key = Arc::new(lua.create_registry_value(title_sep_inset)?);

            // TitleView:get_control_metrics()
            title_view.set("get_control_metrics", {
                lua.create_function(|lua, this: LuaTable| {
                    let cached: LuaValue = this.get("_control_metrics")?;
                    if let LuaValue::Table(ref m) = cached {
                        let style: LuaTable = require_table(lua, "core.style")?;
                        let icon_font: LuaValue = style.get("icon_font")?;
                        let cached_font: LuaValue = m.get("font")?;
                        let same = match (&cached_font, &icon_font) {
                            (LuaValue::Table(a), LuaValue::Table(b)) => a == b,
                            (LuaValue::UserData(a), LuaValue::UserData(b)) => {
                                a.to_pointer() == b.to_pointer()
                            }
                            _ => false,
                        };
                        if same {
                            return Ok(m.clone());
                        }
                    }
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let icon_font: LuaValue = style.get("icon_font")?;
                    let icon_w: f64 = match &icon_font {
                        LuaValue::Table(t) => t.call_method("get_width", "_".to_string())?,
                        LuaValue::UserData(ud) => ud.call_method("get_width", "_".to_string())?,
                        _ => 10.0,
                    };
                    let icon_h: f64 = match &icon_font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let padding: LuaTable = style.get("padding")?;
                    let px: f64 = padding.get("x")?;
                    let font: LuaValue = style.get("font")?;
                    let font_h: f64 = match &font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let spacing = (px * 0.75).max((icon_w * 0.7).floor());
                    let hit_width = (icon_w + px).max(font_h);

                    let metrics = lua.create_table()?;
                    metrics.set("font", icon_font)?;
                    metrics.set("width", icon_w)?;
                    metrics.set("height", icon_h)?;
                    metrics.set("spacing", spacing)?;
                    metrics.set("hit_width", hit_width)?;
                    this.set("_control_metrics", metrics.clone())?;
                    Ok(metrics)
                })?
            })?;

            // TitleView:configure_hit_test(borderless)
            title_view.set("configure_hit_test", {
                let tvh = Arc::clone(&tvh_key);
                let cmds = Arc::clone(&cmds_key);
                lua.create_function(move |lua, (this, borderless): (LuaTable, bool)| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let window: LuaValue = core.get("window")?;
                    if borderless {
                        let tvh_fn: LuaFunction = lua.registry_value(&tvh)?;
                        let title_height: f64 = tvh_fn.call(())?;
                        let metrics: LuaTable = this.call_method("get_control_metrics", ())?;
                        let hit_width: f64 = metrics.get("hit_width")?;
                        let spacing: f64 = metrics.get("spacing")?;
                        let cmds_tbl: LuaTable = lua.registry_value(&cmds)?;
                        let n = cmds_tbl.raw_len() as f64;
                        let controls_width = hit_width * n + spacing;
                        system.call_function::<()>(
                            "set_window_hit_test",
                            (window, title_height, controls_width, spacing),
                        )?;
                    } else {
                        system.call_function::<()>("set_window_hit_test", window)?;
                    }
                    Ok(())
                })?
            })?;

            // TitleView:on_scale_change()
            title_view.set(
                "on_scale_change",
                lua.create_function(|_lua, this: LuaTable| {
                    this.set("_control_metrics", LuaValue::Nil)?;
                    let visible: bool = this
                        .get::<LuaValue>("visible")?
                        .as_boolean()
                        .unwrap_or(false);
                    this.call_method::<()>("configure_hit_test", visible)
                })?,
            )?;

            // TitleView:update()
            title_view.set("update", {
                let k = Arc::clone(&class_key);
                let tvh = Arc::clone(&tvh_key);
                let cmds = Arc::clone(&cmds_key);
                let restore = Arc::clone(&restore_key);
                let maximize = Arc::clone(&maximize_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let visible: bool = this
                        .get::<LuaValue>("visible")?
                        .as_boolean()
                        .unwrap_or(false);
                    let size: LuaTable = this.get("size")?;
                    if visible {
                        let tvh_fn: LuaFunction = lua.registry_value(&tvh)?;
                        let h: f64 = tvh_fn.call(())?;
                        size.set("y", h)?;
                    } else {
                        size.set("y", 0.0)?;
                    }
                    let core: LuaTable = require_table(lua, "core")?;
                    let window_mode: String = core
                        .get::<LuaValue>("window_mode")?
                        .as_string()
                        .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                        .unwrap_or_default();
                    let cmds_tbl: LuaTable = lua.registry_value(&cmds)?;
                    if window_mode == "maximized" {
                        let rc: LuaTable = lua.registry_value(&restore)?;
                        cmds_tbl.raw_set(2, rc)?;
                    } else {
                        let mc: LuaTable = lua.registry_value(&maximize)?;
                        cmds_tbl.raw_set(2, mc)?;
                    }
                    let class: LuaTable = lua.registry_value(&k)?;
                    let super_tbl: LuaTable = class.get("super")?;
                    let super_update: LuaFunction = super_tbl.get("update")?;
                    super_update.call::<()>(this)
                })?
            })?;

            // TitleView:draw_window_title()
            title_view.set("draw_window_title", {
                let ic_key = Arc::clone(&icon_colors_key);
                let cmds = Arc::clone(&cmds_key);
                let tsi = Arc::clone(&tsi_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let renderer: LuaTable = lua.globals().get("renderer")?;

                    let font: LuaValue = style.get("font")?;
                    let fh: f64 = match &font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let color: LuaValue = style.get("text")?;
                    let icon_font: LuaValue = style.get("icon_font")?;
                    let icon_colors: LuaTable = lua.registry_value(&ic_key)?;
                    let padding: LuaTable = style.get("padding")?;
                    let py: f64 = padding.get("y")?;

                    let metrics: LuaTable = this.call_method("get_control_metrics", ())?;
                    let hit_width: f64 = metrics.get("hit_width")?;
                    let spacing: f64 = metrics.get("spacing")?;
                    let cmds_tbl: LuaTable = lua.registry_value(&cmds)?;
                    let n = cmds_tbl.raw_len() as f64;
                    let controls_width = hit_width * n + spacing;

                    let (ox, oy): (f64, f64) = this.call_method("get_content_offset", ())?;
                    let tsi_fn: LuaFunction = lua.registry_value(&tsi)?;
                    let inset: f64 = tsi_fn.call(())?;
                    let mut x = ox + inset;
                    let y = oy + py;

                    let draw_text: LuaFunction = common.get("draw_text")?;
                    for (color_key, symbol) in &[
                        ("bg", "5"),
                        ("color6", "6"),
                        ("color7", "7"),
                        ("color8", "8"),
                    ] {
                        let c: LuaValue = icon_colors.get(*color_key)?;
                        draw_text.call::<LuaValue>((
                            icon_font.clone(),
                            c,
                            *symbol,
                            LuaValue::Nil,
                            x,
                            y,
                            0.0,
                            fh,
                        ))?;
                    }
                    let c9: LuaValue = icon_colors.get("color9")?;
                    let x_after: f64 = draw_text.call((
                        icon_font.clone(),
                        c9,
                        "9 ",
                        LuaValue::Nil,
                        x,
                        y,
                        0.0,
                        fh,
                    ))?;
                    x = x_after;

                    let title: String = core.call_function(
                        "compose_window_title",
                        core.get::<LuaValue>("window_title")?,
                    )?;
                    let size: LuaTable = this.get("size")?;
                    let size_x: f64 = size.get("x")?;
                    let size_y: f64 = size.get("y")?;
                    let title_width = (size_x - controls_width - x - inset).max(0.0);
                    let position: LuaTable = this.get("position")?;
                    let pos_y: f64 = position.get("y")?;

                    let push_clip: LuaFunction = core.get("push_clip_rect")?;
                    push_clip.call::<()>((x, pos_y, title_width, size_y))?;
                    draw_text.call::<LuaValue>((
                        font,
                        color,
                        title,
                        LuaValue::Nil,
                        x,
                        y,
                        title_width,
                        fh,
                    ))?;
                    let pop_clip: LuaFunction = core.get("pop_clip_rect")?;
                    pop_clip.call::<()>(())?;

                    let _ = renderer;
                    Ok(())
                })?
            })?;

            // TitleView:each_control_item()
            title_view.set("each_control_item", {
                let cmds = Arc::clone(&cmds_key);
                let tsi = Arc::clone(&tsi_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let metrics: LuaTable = this.call_method("get_control_metrics", ())?;
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let padding: LuaTable = style.get("padding")?;
                    let py: f64 = padding.get("y")?;
                    let hit_width: f64 = metrics.get("hit_width")?;
                    let height: f64 = metrics.get("height")?;
                    let (ox, oy): (f64, f64) = this.call_method("get_content_offset", ())?;
                    let tsi_fn: LuaFunction = lua.registry_value(&tsi)?;
                    let inset: f64 = tsi_fn.call(())?;
                    let size: LuaTable = this.get("size")?;
                    let size_x: f64 = size.get("x")?;
                    let ox = ox + size_x - inset;
                    let cmds_tbl: LuaTable = lua.registry_value(&cmds)?;
                    let n = cmds_tbl.raw_len() as i64;

                    let cmds_key2 = lua.create_registry_value(cmds_tbl)?;
                    let i = Arc::new(parking_lot::Mutex::new(0i64));
                    let iter = lua.create_function(move |lua, ()| {
                        let mut idx = i.lock();
                        *idx += 1;
                        let ci = *idx;
                        if ci > n {
                            return Ok(LuaMultiValue::new());
                        }
                        let cmds_tbl: LuaTable = lua.registry_value(&cmds_key2)?;
                        let item: LuaTable = cmds_tbl.raw_get(ci)?;
                        let dx = -(hit_width * (n - ci + 1) as f64);
                        let x = ox + dx;
                        let y = oy + py;
                        Ok(LuaMultiValue::from_vec(vec![
                            LuaValue::Table(item),
                            LuaValue::Number(x),
                            LuaValue::Number(y),
                            LuaValue::Number(hit_width),
                            LuaValue::Number(height),
                        ]))
                    })?;
                    Ok(iter)
                })?
            })?;

            // TitleView:draw_window_controls()
            title_view.set(
                "draw_window_controls",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let renderer: LuaTable = lua.globals().get("renderer")?;
                    let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                    let draw_text: LuaFunction = common.get("draw_text")?;
                    let icon_font: LuaValue = style.get("icon_font")?;
                    let style_text: LuaValue = style.get("text")?;
                    let style_dim: LuaValue = style.get("dim")?;
                    let line_highlight: LuaTable = style.get("line_highlight")?;
                    let hovered: LuaValue = this.get("hovered_item")?;
                    let position: LuaTable = this.get("position")?;
                    let pos_y: f64 = position.get("y")?;
                    let size: LuaTable = this.get("size")?;
                    let size_y: f64 = size.get("y")?;

                    let iter: LuaFunction = this.call_method("each_control_item", ())?;
                    loop {
                        let results: LuaMultiValue = iter.call(())?;
                        let mut vals = results.into_iter();
                        let item = match vals.next() {
                            Some(LuaValue::Table(t)) => t,
                            _ => break,
                        };
                        let x: f64 = match vals.next() {
                            Some(LuaValue::Number(n)) => n,
                            _ => break,
                        };
                        let y: f64 = match vals.next() {
                            Some(LuaValue::Number(n)) => n,
                            _ => break,
                        };
                        let w: f64 = match vals.next() {
                            Some(LuaValue::Number(n)) => n,
                            _ => break,
                        };
                        let h: f64 = match vals.next() {
                            Some(LuaValue::Number(n)) => n,
                            _ => break,
                        };

                        let is_hovered = match &hovered {
                            LuaValue::Table(t) => *t == item,
                            _ => false,
                        };
                        let color = if is_hovered {
                            style_text.clone()
                        } else {
                            style_dim.clone()
                        };
                        if is_hovered {
                            let hover_bg = lua.create_table()?;
                            for i in 1..=line_highlight.raw_len() as i64 {
                                let v: LuaValue = line_highlight.raw_get(i)?;
                                hover_bg.raw_set(i, v)?;
                            }
                            hover_bg.raw_set(4, 140)?;
                            draw_rect.call::<()>((x, pos_y, w, size_y, hover_bg))?;
                        }
                        let symbol: String = item.get("symbol")?;
                        draw_text.call::<LuaValue>((
                            icon_font.clone(),
                            color,
                            symbol,
                            "center",
                            x,
                            y,
                            w,
                            h,
                        ))?;
                    }
                    Ok(())
                })?,
            )?;

            // TitleView:on_mouse_pressed(button, x, y, clicks)
            title_view.set("on_mouse_pressed", {
                let k = Arc::clone(&class_key);
                lua.create_function(
                    move |lua,
                          (this, button, x, y, clicks): (
                        LuaTable,
                        LuaValue,
                        f64,
                        f64,
                        LuaValue,
                    )| {
                        let class: LuaTable = lua.registry_value(&k)?;
                        let super_tbl: LuaTable = class.get("super")?;
                        let super_fn: LuaFunction = super_tbl.get("on_mouse_pressed")?;
                        let caught: LuaValue =
                            super_fn.call((this.clone(), button, x, y, clicks))?;
                        if !matches!(caught, LuaValue::Nil | LuaValue::Boolean(false)) {
                            return Ok(());
                        }
                        let core: LuaTable = require_table(lua, "core")?;
                        let last_av: LuaValue = core.get("last_active_view")?;
                        core.call_function::<()>("set_active_view", last_av)?;
                        let hovered: LuaValue = this.get("hovered_item")?;
                        if let LuaValue::Table(item) = hovered {
                            let action: LuaFunction = item.get("action")?;
                            action.call::<()>(())?;
                        }
                        Ok(())
                    },
                )?
            })?;

            // TitleView:on_mouse_left()
            title_view.set("on_mouse_left", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let class: LuaTable = lua.registry_value(&k)?;
                    let super_tbl: LuaTable = class.get("super")?;
                    let super_fn: LuaFunction = super_tbl.get("on_mouse_left")?;
                    super_fn.call::<()>(this.clone())?;
                    this.set("hovered_item", LuaValue::Nil)
                })?
            })?;

            // TitleView:on_mouse_moved(px, py, ...)
            title_view.set("on_mouse_moved", {
                let k = Arc::clone(&class_key);
                lua.create_function(
                    move |lua, (this, px, py, rest): (LuaTable, f64, f64, LuaMultiValue)| {
                        let size: LuaTable = this.get("size")?;
                        let size_y: f64 = size.get("y")?;
                        if size_y == 0.0 {
                            return Ok(());
                        }
                        let class: LuaTable = lua.registry_value(&k)?;
                        let super_tbl: LuaTable = class.get("super")?;
                        let super_fn: LuaFunction = super_tbl.get("on_mouse_moved")?;
                        let mut args = vec![
                            LuaValue::Table(this.clone()),
                            LuaValue::Number(px),
                            LuaValue::Number(py),
                        ];
                        for v in rest {
                            args.push(v);
                        }
                        super_fn.call::<()>(LuaMultiValue::from_vec(args))?;
                        this.set("hovered_item", LuaValue::Nil)?;
                        let iter: LuaFunction = this.call_method("each_control_item", ())?;
                        loop {
                            let results: LuaMultiValue = iter.call(())?;
                            let mut vals = results.into_iter();
                            let item = match vals.next() {
                                Some(LuaValue::Table(t)) => t,
                                _ => break,
                            };
                            let x: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            let y: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            let w: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            let h: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            if px > x && py > y && px <= x + w && py <= y + h {
                                this.set("hovered_item", item)?;
                                return Ok(());
                            }
                        }
                        Ok(())
                    },
                )?
            })?;

            // TitleView:draw()
            title_view.set("draw", {
                let tsi = Arc::clone(&tsi_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let bg2: LuaValue = style.get("background2")?;
                    this.call_method::<()>("draw_background", bg2)?;
                    this.call_method::<()>("draw_window_title", ())?;
                    this.call_method::<()>("draw_window_controls", ())?;

                    let renderer: LuaTable = lua.globals().get("renderer")?;
                    let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                    let tsi_fn: LuaFunction = lua.registry_value(&tsi)?;
                    let inset: f64 = tsi_fn.call(())?;
                    let position: LuaTable = this.get("position")?;
                    let pos_x: f64 = position.get("x")?;
                    let pos_y: f64 = position.get("y")?;
                    let size: LuaTable = this.get("size")?;
                    let size_x: f64 = size.get("x")?;
                    let size_y: f64 = size.get("y")?;
                    let divider_size: f64 = style.get("divider_size")?;
                    let divider: LuaValue = style.get("divider")?;
                    draw_rect.call::<()>((
                        pos_x + inset,
                        pos_y + size_y - divider_size,
                        (size_x - inset * 2.0).max(0.0),
                        divider_size,
                        divider,
                    ))
                })?
            })?;

            Ok(LuaValue::Table(title_view))
        })?,
    )
}
