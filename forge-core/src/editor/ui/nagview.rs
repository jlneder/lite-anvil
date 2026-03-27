use mlua::prelude::*;

use std::sync::Arc;

/// Require a module by name, returning the loaded table.
fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers `core.nagview` — modal dialog with buttons for user confirmation.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.nagview",
        lua.create_function(|lua, ()| {
            let view_class: LuaTable = require_table(lua, "core.view")?;
            let nag_view = view_class.call_method::<LuaTable>("extend", ())?;

            nag_view.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("NagView"))?,
            )?;

            let class_key = Arc::new(lua.create_registry_value(nag_view.clone())?);

            // NagView:new()
            nag_view.set("new", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let class: LuaTable = lua.registry_value(&k)?;
                    let super_tbl: LuaTable = class.get("super")?;
                    let super_new: LuaFunction = super_tbl.get("new")?;
                    super_new.call::<()>(this.clone())?;

                    let size: LuaTable = this.get("size")?;
                    size.set("y", 0.0)?;
                    this.set("show_height", 0.0)?;
                    this.set("force_focus", false)?;
                    this.set("queue", lua.create_table()?)?;
                    this.set("scrollable", true)?;
                    this.set("target_height", 0.0)?;
                    this.set("on_mouse_pressed_root", LuaValue::Nil)?;
                    this.set("dim_alpha", 0.0)?;
                    Ok(())
                })?
            })?;

            // NagView:get_title()
            nag_view.set(
                "get_title",
                lua.create_function(|_lua, this: LuaTable| {
                    let title: LuaValue = this.get("title")?;
                    Ok(title)
                })?,
            )?;

            // NagView:get_line_height()
            nag_view.set(
                "get_line_height",
                lua.create_function(|lua, _this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let config: LuaTable = require_table(lua, "core.config")?;
                    let font: LuaValue = style.get("font")?;
                    let fh: f64 = match &font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let lh: f64 = config.get("line_height")?;
                    Ok((fh * lh).floor())
                })?,
            )?;

            // NagView:get_line_text_y_offset()
            nag_view.set(
                "get_line_text_y_offset",
                lua.create_function(|lua, this: LuaTable| {
                    let lh: f64 = this.call_method("get_line_height", ())?;
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let font: LuaValue = style.get("font")?;
                    let th: f64 = match &font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    Ok((lh - th) / 2.0)
                })?,
            )?;

            // NagView:get_buttons_height()
            nag_view.set(
                "get_buttons_height",
                lua.create_function(|lua, _this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let scale: f64 = lua.globals().get("SCALE")?;
                    let border_width = (1.0 * scale).round();
                    let font: LuaValue = style.get("font")?;
                    let lh: f64 = match &font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let bt_padding = lh / 2.0;
                    Ok(lh + 2.0 * border_width + 2.0 * bt_padding)
                })?,
            )?;

            // NagView:get_target_height()
            nag_view.set(
                "get_target_height",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let padding: LuaTable = style.get("padding")?;
                    let py: f64 = padding.get("y")?;
                    let th: f64 = this.get("target_height")?;
                    Ok(th + 2.0 * py)
                })?,
            )?;

            // NagView:get_scrollable_size()
            nag_view.set(
                "get_scrollable_size",
                lua.create_function(|lua, this: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let system: LuaTable = lua.globals().get("system")?;
                    let window: LuaValue = core.get("window")?;
                    let (_, h): (f64, f64) = system.call_function("get_window_size", window)?;
                    let visible: bool = this
                        .get::<LuaValue>("visible")?
                        .as_boolean()
                        .unwrap_or(false);
                    if visible {
                        let target_h: f64 = this.call_method("get_target_height", ())?;
                        if target_h > h {
                            let size: LuaTable = this.get("size")?;
                            size.set("y", h)?;
                            return Ok(target_h);
                        }
                    }
                    let size: LuaTable = this.get("size")?;
                    size.set("y", 0.0)?;
                    Ok(0.0)
                })?,
            )?;

            // NagView:dim_window_content()
            nag_view.set(
                "dim_window_content",
                lua.create_function(|lua, this: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let (ox, oy): (f64, f64) = this.call_method("get_content_offset", ())?;
                    let show_height: f64 = this.get("show_height")?;
                    let oy = oy + show_height;
                    let root_view: LuaTable = core.get("root_view")?;
                    let rv_size: LuaTable = root_view.get("size")?;
                    let w: f64 = rv_size.get("x")?;
                    let rv_y: f64 = rv_size.get("y")?;
                    let h: f64 = rv_y - oy;

                    let nagbar_dim: LuaTable = style.get("nagbar_dim")?;
                    let dim_alpha: f64 = this.get("dim_alpha")?;

                    let this_key = lua.create_registry_value(this)?;
                    let draw_fn = lua.create_function(move |lua, ()| {
                        let this: LuaTable = lua.registry_value(&this_key)?;
                        let style: LuaTable = require_table(lua, "core.style")?;
                        let nagbar_dim: LuaTable = style.get("nagbar_dim")?;
                        let dim_alpha: f64 = this.get("dim_alpha")?;
                        let dim_color = lua.create_table()?;
                        for i in 1..=nagbar_dim.raw_len() as i64 {
                            let v: LuaValue = nagbar_dim.raw_get(i)?;
                            dim_color.raw_set(i, v)?;
                        }
                        let a4: f64 = nagbar_dim.raw_get(4)?;
                        dim_color.raw_set(4, a4 * dim_alpha)?;
                        let renderer: LuaTable = lua.globals().get("renderer")?;
                        let draw_rect: LuaFunction = renderer.get("draw_rect")?;

                        let (ox2, oy2): (f64, f64) = this.call_method("get_content_offset", ())?;
                        let show_height2: f64 = this.get("show_height")?;
                        let oy2 = oy2 + show_height2;
                        let core2: LuaTable = require_table(lua, "core")?;
                        let rv2: LuaTable = core2.get("root_view")?;
                        let rv_size2: LuaTable = rv2.get("size")?;
                        let w2: f64 = rv_size2.get("x")?;
                        let rv_y2: f64 = rv_size2.get("y")?;
                        let h2: f64 = rv_y2 - oy2;

                        draw_rect.call::<()>((ox2, oy2, w2, h2, dim_color))?;
                        Ok(())
                    })?;

                    // We still need valid w, h, dim_alpha, nagbar_dim for the closure capture
                    let _ = (w, h, dim_alpha, nagbar_dim);
                    let _ = ox;
                    root_view.call_method::<()>("defer_draw", draw_fn)?;
                    Ok(())
                })?,
            )?;

            // NagView:change_hovered(i)
            nag_view.set(
                "change_hovered",
                lua.create_function(|lua, (this, i): (LuaTable, LuaValue)| {
                    let hovered: LuaValue = this.get("hovered_item")?;
                    let changed = match (&hovered, &i) {
                        (LuaValue::Integer(a), LuaValue::Integer(b)) => a != b,
                        (LuaValue::Number(a), LuaValue::Number(b)) => (a - b).abs() > 0.001,
                        _ => true,
                    };
                    if changed {
                        this.set("hovered_item", i)?;
                        this.set("underline_progress", 0.0)?;
                        let core: LuaTable = require_table(lua, "core")?;
                        core.set("redraw", true)?;
                    }
                    Ok(())
                })?,
            )?;

            // NagView:each_option()
            // NagView:each_option() - stateful iterator, no coroutine.yield.
            nag_view.set(
                "each_option",
                lua.create_function(|lua, this: LuaTable| {
                    let options: LuaValue = this.get("options")?;
                    let options = match options {
                        LuaValue::Table(t) => t,
                        _ => {
                            let empty =
                                lua.create_function(|_, ()| -> LuaResult<LuaMultiValue> {
                                    Ok(LuaMultiValue::new())
                                })?;
                            return Ok(empty);
                        }
                    };
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let scale: f64 = lua.globals().get("SCALE")?;
                    let border_width = (1.0 * scale).round();
                    let bh: f64 = this.call_method("get_buttons_height", ())?;
                    let (mut ox, oy): (f64, f64) = this.call_method("get_content_offset", ())?;
                    let size: LuaTable = this.get("size")?;
                    let size_x: f64 = size.get("x")?;
                    ox += size_x;
                    let show_height: f64 = this.get("show_height")?;
                    let padding: LuaTable = style.get("padding")?;
                    let pad_x: f64 = padding.get("x")?;
                    let pad_y: f64 = padding.get("y")?;
                    let oy = oy + show_height - bh - pad_y;

                    let font: LuaValue = style.get("font")?;
                    let len = options.raw_len() as i64;

                    let results = lua.create_table()?;
                    let mut count = 0i64;
                    for idx in (1..=len).rev() {
                        let opt: LuaTable = options.raw_get(idx)?;
                        let text: String = opt.get("text")?;
                        let fw: f64 = match &font {
                            LuaValue::Table(t) => t.call_method("get_width", text)?,
                            LuaValue::UserData(ud) => ud.call_method("get_width", text)?,
                            _ => 40.0,
                        };
                        let bw = fw + 2.0 * border_width + pad_x;
                        ox -= bw + pad_x;
                        count += 1;
                        let entry = lua.create_table()?;
                        entry.raw_set(1, idx)?;
                        entry.raw_set(2, opt)?;
                        entry.raw_set(3, ox)?;
                        entry.raw_set(4, oy)?;
                        entry.raw_set(5, bw)?;
                        entry.raw_set(6, bh)?;
                        results.raw_set(count, entry)?;
                    }

                    let state = lua.create_table()?;
                    state.set("idx", 0i64)?;
                    state.set("len", count)?;
                    let results_key = lua.create_registry_value(results)?;

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
                            let entry: LuaTable = results.raw_get(next)?;
                            let i: LuaValue = entry.raw_get(1)?;
                            let opt: LuaValue = entry.raw_get(2)?;
                            let ex: LuaValue = entry.raw_get(3)?;
                            let ey: LuaValue = entry.raw_get(4)?;
                            let ew: LuaValue = entry.raw_get(5)?;
                            let eh: LuaValue = entry.raw_get(6)?;
                            Ok(LuaMultiValue::from_vec(vec![i, opt, ex, ey, ew, eh]))
                        })?;
                    Ok(iterator)
                })?,
            )?;

            // NagView:on_mouse_moved(mx, my, ...)
            nag_view.set("on_mouse_moved", {
                let k = Arc::clone(&class_key);
                lua.create_function(
                    move |lua, (this, mx, my, rest): (LuaTable, f64, f64, LuaMultiValue)| {
                        let visible: bool = this
                            .get::<LuaValue>("visible")?
                            .as_boolean()
                            .unwrap_or(false);
                        if !visible {
                            return Ok(());
                        }
                        let core: LuaTable = require_table(lua, "core")?;
                        core.call_function::<()>("set_active_view", this.clone())?;
                        let class: LuaTable = lua.registry_value(&k)?;
                        let super_tbl: LuaTable = class.get("super")?;
                        let super_fn: LuaFunction = super_tbl.get("on_mouse_moved")?;
                        let mut args = vec![
                            LuaValue::Table(this.clone()),
                            LuaValue::Number(mx),
                            LuaValue::Number(my),
                        ];
                        for v in rest {
                            args.push(v);
                        }
                        super_fn.call::<()>(LuaMultiValue::from_vec(args))?;
                        let iter: LuaFunction = this.call_method("each_option", ())?;
                        loop {
                            let results: LuaMultiValue = iter.call(())?;
                            let mut vals = results.into_iter();
                            let i_val = match vals.next() {
                                Some(v) if !matches!(v, LuaValue::Nil) => v,
                                _ => break,
                            };
                            let _ = vals.next(); // opt
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
                            if mx >= x && my >= y && mx < x + w && my < y + h {
                                this.call_method::<()>("change_hovered", i_val)?;
                                break;
                            }
                        }
                        Ok(())
                    },
                )?
            })?;

            // NagView:on_mouse_pressed(button, mx, my, clicks)
            nag_view.set("on_mouse_pressed", {
                let k = Arc::clone(&class_key);
                lua.create_function(
                    move |lua,
                          (this, button, mx, my, clicks): (
                        LuaTable,
                        LuaValue,
                        f64,
                        f64,
                        LuaValue,
                    )| {
                        let visible: bool = this
                            .get::<LuaValue>("visible")?
                            .as_boolean()
                            .unwrap_or(false);
                        if !visible {
                            return Ok(LuaValue::Boolean(false));
                        }
                        let class: LuaTable = lua.registry_value(&k)?;
                        let super_tbl: LuaTable = class.get("super")?;
                        let super_fn: LuaFunction = super_tbl.get("on_mouse_pressed")?;
                        let caught: LuaValue =
                            super_fn.call((this.clone(), button, mx, my, clicks))?;
                        if !matches!(caught, LuaValue::Nil | LuaValue::Boolean(false)) {
                            return Ok(LuaValue::Boolean(true));
                        }
                        let iter: LuaFunction = this.call_method("each_option", ())?;
                        loop {
                            let results: LuaMultiValue = iter.call(())?;
                            let mut vals = results.into_iter();
                            let i_val = match vals.next() {
                                Some(v) if !matches!(v, LuaValue::Nil) => v,
                                _ => break,
                            };
                            let _ = vals.next(); // opt
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
                            if mx >= x && my >= y && mx < x + w && my < y + h {
                                this.call_method::<()>("change_hovered", i_val)?;
                                let command: LuaTable = require_table(lua, "core.command")?;
                                command.call_function::<()>("perform", "dialog:select")?;
                            }
                        }
                        Ok(LuaValue::Boolean(true))
                    },
                )?
            })?;

            // NagView:on_text_input(text)
            nag_view.set(
                "on_text_input",
                lua.create_function(|lua, (this, text): (LuaTable, String)| {
                    let visible: bool = this
                        .get::<LuaValue>("visible")?
                        .as_boolean()
                        .unwrap_or(false);
                    if !visible {
                        return Ok(());
                    }
                    let lower = text.to_lowercase();
                    let command: LuaTable = require_table(lua, "core.command")?;
                    if lower == "y" {
                        command.call_function::<()>("perform", "dialog:select-yes")?;
                    } else if lower == "n" {
                        command.call_function::<()>("perform", "dialog:select-no")?;
                    } else if lower.len() == 1 {
                        let options: LuaValue = this.get("options")?;
                        if let LuaValue::Table(opts) = options {
                            let mut matched: Option<i64> = None;
                            let len = opts.raw_len() as i64;
                            for i in 1..=len {
                                let opt: LuaTable = opts.raw_get(i)?;
                                let opt_text: String = opt
                                    .get::<LuaValue>("text")?
                                    .as_string()
                                    .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                                    .unwrap_or_default();
                                if !opt_text.is_empty() {
                                    let initial = opt_text[..1].to_lowercase();
                                    if initial == lower {
                                        if matched.is_some() {
                                            matched = None;
                                            break;
                                        }
                                        matched = Some(i);
                                    }
                                }
                            }
                            if let Some(m) = matched {
                                this.call_method::<()>("change_hovered", m)?;
                                command.call_function::<()>("perform", "dialog:select")?;
                            }
                        }
                    }
                    Ok(())
                })?,
            )?;

            // NagView:update()
            nag_view.set("update", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let visible: bool = this
                        .get::<LuaValue>("visible")?
                        .as_boolean()
                        .unwrap_or(false);
                    let show_height: f64 = this.get("show_height")?;
                    if !visible && show_height <= 0.0 {
                        return Ok(());
                    }
                    let class: LuaTable = lua.registry_value(&k)?;
                    let super_tbl: LuaTable = class.get("super")?;
                    let super_update: LuaFunction = super_tbl.get("update")?;
                    super_update.call::<()>(this.clone())?;

                    let core: LuaTable = require_table(lua, "core")?;
                    let active_view: LuaValue = core.get("active_view")?;
                    let title: LuaValue = this.get("title")?;
                    let is_active = match &active_view {
                        LuaValue::Table(t) => *t == this,
                        _ => false,
                    };
                    if visible && is_active && !matches!(title, LuaValue::Nil) {
                        let target_height: f64 = this.call_method("get_target_height", ())?;
                        this.call_method::<()>(
                            "move_towards",
                            (this.clone(), "show_height", target_height),
                        )?;
                        this.call_method::<()>(
                            "move_towards",
                            (this.clone(), "underline_progress", 1.0),
                        )?;
                        let sh: f64 = this.get("show_height")?;
                        this.call_method::<()>(
                            "move_towards",
                            (this.clone(), "dim_alpha", sh / target_height),
                        )?;
                    } else {
                        this.call_method::<()>("move_towards", (this.clone(), "show_height", 0.0))?;
                        this.call_method::<()>("move_towards", (this.clone(), "dim_alpha", 0.0))?;
                        let sh: f64 = this.get("show_height")?;
                        if sh <= 0.0 {
                            this.set("title", LuaValue::Nil)?;
                            this.set("message", LuaValue::Nil)?;
                            this.set("options", LuaValue::Nil)?;
                            this.set("on_selected", LuaValue::Nil)?;
                        }
                    }
                    Ok(())
                })?
            })?;

            // NagView:draw()
            nag_view.set("draw", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let visible: bool = this
                        .get::<LuaValue>("visible")?
                        .as_boolean()
                        .unwrap_or(false);
                    let show_height: f64 = this.get("show_height")?;
                    let title: LuaValue = this.get("title")?;
                    if (!visible && show_height <= 0.0) || matches!(title, LuaValue::Nil) {
                        return Ok(());
                    }
                    let core: LuaTable = require_table(lua, "core")?;
                    let root_view: LuaTable = core.get("root_view")?;

                    let this_key = lua.create_registry_value(this.clone())?;
                    let class_key2 =
                        lua.create_registry_value(lua.registry_value::<LuaTable>(&k)?)?;
                    let draw_fn = lua.create_function(move |lua, this: LuaTable| {
                        let style: LuaTable = require_table(lua, "core.style")?;
                        let common: LuaTable = require_table(lua, "core.common")?;
                        let core: LuaTable = require_table(lua, "core")?;
                        let renderer: LuaTable = lua.globals().get("renderer")?;
                        let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                        let draw_text: LuaFunction = renderer.get("draw_text")?;
                        let scale: f64 = lua.globals().get("SCALE")?;
                        let border_width = (1.0 * scale).round();
                        let underline_width = (2.0 * scale).round();
                        let underline_margin = (1.0 * scale).round();

                        this.call_method::<()>("dim_window_content", ())?;

                        let (ox, oy): (f64, f64) = this.call_method("get_content_offset", ())?;
                        let size: LuaTable = this.get("size")?;
                        let size_x: f64 = size.get("x")?;
                        let show_height: f64 = this.get("show_height")?;
                        let nagbar: LuaValue = style.get("nagbar")?;
                        draw_rect.call::<()>((ox, oy, size_x, show_height, nagbar.clone()))?;

                        let mut ox = ox;
                        let padding: LuaTable = style.get("padding")?;
                        let pad_x: f64 = padding.get("x")?;
                        let pad_y: f64 = padding.get("y")?;
                        ox += pad_x;

                        let push_clip: LuaFunction = core.get("push_clip_rect")?;
                        push_clip.call::<()>((ox, oy, size_x, show_height))?;

                        let queue: LuaTable = this.get("queue")?;
                        let queue_len = queue.raw_len() as i64;
                        if queue_len > 0 {
                            let nagbar_text: LuaValue = style.get("nagbar_text")?;
                            let str_val = format!("[{}]", queue_len);
                            let font: LuaValue = style.get("font")?;
                            let result: f64 = common.call_function(
                                "draw_text",
                                (
                                    font.clone(),
                                    nagbar_text.clone(),
                                    str_val,
                                    "left",
                                    ox,
                                    oy,
                                    size_x,
                                    show_height,
                                ),
                            )?;
                            ox = result + pad_x;
                        }

                        let config: LuaTable = require_table(lua, "core.config")?;
                        let font: LuaValue = style.get("font")?;
                        let font_h: f64 = match &font {
                            LuaValue::Table(t) => t.call_method("get_height", ())?,
                            LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                            _ => 14.0,
                        };
                        let line_h: f64 = config.get("line_height")?;
                        let lh = font_h * line_h;
                        let target_height: f64 = this.get("target_height")?;
                        let message_height: f64 = this.call_method("get_message_height", ())?;
                        let mut oy2 = oy + pad_y + (target_height - message_height) / 2.0;

                        let message: String = this.get("message")?;
                        let nagbar_text: LuaValue = style.get("nagbar_text")?;
                        for line in message.split('\n') {
                            if line.is_empty() && !message.ends_with('\n') {
                                continue;
                            }
                            if line.is_empty() {
                                continue;
                            }
                            let ty: f64 = this.call_method("get_line_text_y_offset", ())?;
                            let ty = oy2 + ty;
                            draw_text.call::<()>((
                                font.clone(),
                                line.to_string(),
                                ox,
                                ty,
                                nagbar_text.clone(),
                            ))?;
                            oy2 += lh;
                        }

                        let iter: LuaFunction = this.call_method("each_option", ())?;
                        let hovered_item: LuaValue = this.get("hovered_item")?;
                        let underline_progress: f64 = this
                            .get::<LuaValue>("underline_progress")?
                            .as_number()
                            .unwrap_or(0.0);
                        loop {
                            let results: LuaMultiValue = iter.call(())?;
                            let mut vals = results.into_iter();
                            let i_val = match vals.next() {
                                Some(v) if !matches!(v, LuaValue::Nil) => v,
                                _ => break,
                            };
                            let opt = match vals.next() {
                                Some(LuaValue::Table(t)) => t,
                                _ => break,
                            };
                            let bx: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            let by: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            let bw: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };
                            let bh: f64 = match vals.next() {
                                Some(LuaValue::Number(n)) => n,
                                _ => break,
                            };

                            let fw = bw - 2.0 * border_width;
                            let fh = bh - 2.0 * border_width;
                            let fx = bx + border_width;
                            let fy = by + border_width;

                            draw_rect.call::<()>((bx, by, bw, bh, nagbar_text.clone()))?;
                            draw_rect.call::<()>((fx, fy, fw, fh, nagbar.clone()))?;

                            let is_hovered = match (&i_val, &hovered_item) {
                                (LuaValue::Integer(a), LuaValue::Integer(b)) => a == b,
                                (LuaValue::Number(a), LuaValue::Number(b)) => (a - b).abs() < 0.001,
                                (LuaValue::Integer(a), LuaValue::Number(b)) => {
                                    (*a as f64 - b).abs() < 0.001
                                }
                                (LuaValue::Number(a), LuaValue::Integer(b)) => {
                                    (a - *b as f64).abs() < 0.001
                                }
                                _ => false,
                            };
                            if is_hovered {
                                let uw = fw - 2.0 * underline_margin;
                                let halfuw = uw / 2.0;
                                let lx =
                                    fx + underline_margin + halfuw - (halfuw * underline_progress);
                                let ly = fy + fh - underline_margin - underline_width;
                                let uw = uw * underline_progress;
                                draw_rect.call::<()>((
                                    lx,
                                    ly,
                                    uw,
                                    underline_width,
                                    nagbar_text.clone(),
                                ))?;
                            }

                            let opt_text: String = opt.get("text")?;
                            common.call_function::<()>(
                                "draw_text",
                                (
                                    font.clone(),
                                    nagbar_text.clone(),
                                    opt_text,
                                    "center",
                                    fx,
                                    fy,
                                    fw,
                                    fh,
                                ),
                            )?;
                        }

                        this.call_method::<()>("draw_scrollbar", ())?;
                        let pop_clip: LuaFunction = core.get("pop_clip_rect")?;
                        pop_clip.call::<()>(())?;
                        Ok(())
                    })?;

                    let _ = class_key2;
                    let this2: LuaTable = lua.registry_value(&this_key)?;
                    root_view.call_method::<()>("defer_draw", (draw_fn, this2))?;
                    Ok(())
                })?
            })?;

            // NagView:on_scale_change(new_scale, old_scale)
            nag_view.set(
                "on_scale_change",
                lua.create_function(|lua, (this, new_scale, _old_scale): (LuaTable, f64, f64)| {
                    let _ = (1.0 * new_scale).round(); // border_width
                    let _ = (2.0 * new_scale).round(); // underline_width
                    let _ = (1.0 * new_scale).round(); // underline_margin
                    let msg_h: f64 = this.call_method("get_message_height", ())?;
                    let btn_h: f64 = this.call_method("get_buttons_height", ())?;
                    let _ = lua;
                    this.set("target_height", msg_h.max(btn_h))
                })?,
            )?;

            // NagView:get_message_height()
            nag_view.set(
                "get_message_height",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let config: LuaTable = require_table(lua, "core.config")?;
                    let font: LuaValue = style.get("font")?;
                    let font_h: f64 = match &font {
                        LuaValue::Table(t) => t.call_method("get_height", ())?,
                        LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                        _ => 14.0,
                    };
                    let line_h: f64 = config.get("line_height")?;
                    let message: LuaValue = this.get("message")?;
                    let msg = match &message {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Ok(0.0),
                    };
                    // gmatch("(.-)\n") only matches lines ending in \n
                    let mut h = 0.0;
                    for _ in msg.matches('\n') {
                        h += font_h * line_h;
                    }
                    Ok(h)
                })?,
            )?;

            // NagView:next()
            nag_view.set(
                "next",
                lua.create_function(|lua, this: LuaTable| {
                    let queue: LuaTable = this.get("queue")?;
                    let table_remove: LuaFunction =
                        lua.globals().get::<LuaTable>("table")?.get("remove")?;
                    let opts: LuaValue = table_remove.call((queue, 1))?;
                    let opts = match opts {
                        LuaValue::Table(t) => t,
                        _ => lua.create_table()?,
                    };
                    let has_title = !matches!(opts.get::<LuaValue>("title")?, LuaValue::Nil);
                    let has_message = !matches!(opts.get::<LuaValue>("message")?, LuaValue::Nil);
                    let has_options = !matches!(opts.get::<LuaValue>("options")?, LuaValue::Nil);

                    if has_title && has_message && has_options {
                        this.set("visible", true)?;
                        let title: LuaValue = opts.get("title")?;
                        this.set("title", title)?;
                        let msg: String = opts
                            .get::<LuaValue>("message")?
                            .as_string()
                            .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                            .unwrap_or_default();
                        this.set("message", format!("{}\n", msg))?;
                        let options: LuaValue = opts.get("options")?;
                        this.set("options", options)?;
                        let on_selected: LuaValue = opts.get("on_selected")?;
                        this.set("on_selected", on_selected)?;

                        let msg_h: f64 = this.call_method("get_message_height", ())?;
                        let btn_h: f64 = this.call_method("get_buttons_height", ())?;
                        this.set("target_height", msg_h.max(btn_h))?;
                        let target_h: f64 = this.call_method("get_target_height", ())?;
                        this.set("show_height", target_h)?;
                        this.set("dim_alpha", 1.0)?;

                        let common: LuaTable = require_table(lua, "core.common")?;
                        let options_tbl: LuaTable = this.get("options")?;
                        let idx: LuaValue =
                            common.call_function("find_index", (options_tbl, "default_yes"))?;
                        this.call_method::<()>("change_hovered", idx)?;

                        this.set("force_focus", true)?;
                        let core: LuaTable = require_table(lua, "core")?;
                        core.call_function::<()>("set_active_view", this.clone())?;

                        // register_mouse_pressed inline
                        let on_mpr: LuaValue = this.get("on_mouse_pressed_root")?;
                        if matches!(on_mpr, LuaValue::Nil) {
                            let root_view_class: LuaTable = require_table(lua, "core.rootview")?;
                            let orig_fn: LuaFunction = root_view_class.get("on_mouse_pressed")?;
                            this.set("on_mouse_pressed_root", orig_fn.clone())?;
                            let this_key = lua.create_registry_value(this.clone())?;
                            let orig_key = lua.create_registry_value(orig_fn)?;
                            let new_fn = lua.create_function(
                                move |lua,
                                      (rv, button, x, y, clicks): (
                                    LuaTable,
                                    LuaValue,
                                    f64,
                                    f64,
                                    LuaValue,
                                )| {
                                    let nv: LuaTable = lua.registry_value(&this_key)?;
                                    let result: LuaValue = nv.call_method(
                                        "on_mouse_pressed",
                                        (button.clone(), x, y, clicks.clone()),
                                    )?;
                                    if matches!(result, LuaValue::Nil | LuaValue::Boolean(false)) {
                                        let orig: LuaFunction = lua.registry_value(&orig_key)?;
                                        return orig.call((rv, button, x, y, clicks));
                                    }
                                    Ok(LuaValue::Boolean(true))
                                },
                            )?;
                            root_view_class.set("on_mouse_pressed", new_fn.clone())?;
                            this.set("new_on_mouse_pressed_root", new_fn)?;
                        }
                    } else {
                        this.set("force_focus", false)?;
                        let core: LuaTable = require_table(lua, "core")?;
                        let next_av: LuaValue = core.get("next_active_view")?;
                        let target = if !matches!(next_av, LuaValue::Nil) {
                            next_av
                        } else {
                            core.get("last_active_view")?
                        };
                        core.call_function::<()>("set_active_view", target)?;
                        this.set("visible", false)?;

                        // unregister_mouse_pressed inline
                        let on_mpr: LuaValue = this.get("on_mouse_pressed_root")?;
                        if !matches!(on_mpr, LuaValue::Nil) {
                            let root_view_class: LuaTable = require_table(lua, "core.rootview")?;
                            let new_mpr: LuaValue = this.get("new_on_mouse_pressed_root")?;
                            let current: LuaValue = root_view_class.get("on_mouse_pressed")?;
                            let same = match (&new_mpr, &current) {
                                (LuaValue::Function(a), LuaValue::Function(b)) => a == b,
                                _ => false,
                            };
                            if same {
                                root_view_class.set("on_mouse_pressed", on_mpr)?;
                                this.set("on_mouse_pressed_root", LuaValue::Nil)?;
                                this.set("new_on_mouse_pressed_root", LuaValue::Nil)?;
                            }
                        }
                    }
                    Ok(())
                })?,
            )?;

            // NagView:show(title, message, options, on_select)
            nag_view.set(
                "show",
                lua.create_function(
                    |lua,
                     (this, title, message, options, on_select): (
                        LuaTable,
                        LuaValue,
                        LuaValue,
                        LuaValue,
                        LuaValue,
                    )| {
                        if matches!(title, LuaValue::Nil) {
                            return Err(LuaError::RuntimeError("No title".into()));
                        }
                        if matches!(message, LuaValue::Nil) {
                            return Err(LuaError::RuntimeError("No message".into()));
                        }
                        if matches!(options, LuaValue::Nil) {
                            return Err(LuaError::RuntimeError("No options".into()));
                        }
                        let opts = lua.create_table()?;
                        opts.set("title", title)?;
                        opts.set("message", message)?;
                        opts.set("options", options)?;
                        let on_selected = if matches!(on_select, LuaValue::Nil) {
                            lua.create_function(|_lua, ()| Ok(()))?
                        } else {
                            match on_select {
                                LuaValue::Function(f) => f,
                                _ => lua.create_function(|_lua, ()| Ok(()))?,
                            }
                        };
                        opts.set("on_selected", on_selected)?;
                        let queue: LuaTable = this.get("queue")?;
                        let table_insert: LuaFunction =
                            lua.globals().get::<LuaTable>("table")?.get("insert")?;
                        table_insert.call::<()>((queue, opts))?;
                        let visible: bool = this
                            .get::<LuaValue>("visible")?
                            .as_boolean()
                            .unwrap_or(false);
                        if !visible {
                            this.call_method::<()>("next", ())?;
                        }
                        Ok(())
                    },
                )?,
            )?;

            Ok(LuaValue::Table(nag_view))
        })?,
    )
}
