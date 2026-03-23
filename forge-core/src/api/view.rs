use mlua::prelude::*;

use std::sync::Arc;

/// Require a module by name, returning the loaded table.
fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers `core.view` — base View class with scroll state, scrollbar, and event hooks.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.view",
        lua.create_function(|lua, ()| {
            let object: LuaTable = require_table(lua, "core.object")?;
            let scrollbar_class: LuaTable = require_table(lua, "core.scrollbar")?;
            let view = object.call_method::<LuaTable>("extend", ())?;

            view.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("View"))?,
            )?;

            view.set("context", "application")?;

            let class_key = Arc::new(lua.create_registry_value(view.clone())?);
            let scrollbar_key = Arc::new(lua.create_registry_value(scrollbar_class)?);

            // View:new()
            view.set("new", {
                let sb_key = Arc::clone(&scrollbar_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let pos = lua.create_table()?;
                    pos.set("x", 0.0)?;
                    pos.set("y", 0.0)?;
                    this.set("position", pos)?;

                    let size = lua.create_table()?;
                    size.set("x", 0.0)?;
                    size.set("y", 0.0)?;
                    this.set("size", size)?;

                    let scroll_to = lua.create_table()?;
                    scroll_to.set("x", 0.0)?;
                    scroll_to.set("y", 0.0)?;
                    let scroll = lua.create_table()?;
                    scroll.set("x", 0.0)?;
                    scroll.set("y", 0.0)?;
                    scroll.set("to", scroll_to)?;
                    this.set("scroll", scroll)?;

                    this.set("cursor", "arrow")?;
                    this.set("scrollable", false)?;

                    let sb_class: LuaTable = lua.registry_value(&sb_key)?;
                    let v_opts = lua.create_table()?;
                    v_opts.set("direction", "v")?;
                    v_opts.set("alignment", "e")?;
                    let v_sb: LuaTable = sb_class.call((v_opts,))?;
                    this.set("v_scrollbar", v_sb)?;

                    let h_opts = lua.create_table()?;
                    h_opts.set("direction", "h")?;
                    h_opts.set("alignment", "e")?;
                    let h_sb: LuaTable = sb_class.call((h_opts,))?;
                    this.set("h_scrollbar", h_sb)?;

                    let scale: f64 = lua.globals().get("SCALE")?;
                    this.set("current_scale", scale)?;

                    Ok(())
                })?
            })?;

            // View:move_towards(t, k, dest, rate, name)
            // Also supports: View:move_towards(k, dest, rate, name) where t=self
            view.set(
                "move_towards",
                lua.create_function(|lua, (this, arg1, arg2, arg3, arg4, arg5): (LuaTable, LuaValue, LuaValue, LuaValue, LuaValue, LuaValue)| {
                    let (t, k, dest, rate, name) = if let LuaValue::Table(_) = &arg1 {
                        // move_towards(t, k, dest, rate, name)
                        (arg1, arg2, arg3, arg4, arg5)
                    } else {
                        // move_towards(k, dest, rate, name) -> move_towards(self, k, dest, rate, name)
                        (LuaValue::Table(this.clone()), arg1, arg2, arg3, arg4)
                    };
                    let t = match t {
                        LuaValue::Table(t) => t,
                        _ => return Ok(()),
                    };
                    let k_str = match &k {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Ok(()),
                    };
                    let val: f64 = t.get(k_str.as_str())?;
                    let dest = match dest {
                        LuaValue::Number(n) => n,
                        LuaValue::Integer(n) => n as f64,
                        _ => return Ok(()),
                    };
                    let diff = (val - dest).abs();

                    let config: LuaTable = require_table(lua, "core.config")?;
                    let transitions: bool = config.get::<LuaValue>("transitions")?.as_boolean().unwrap_or(true);
                    let disabled: LuaTable = config.get("disabled_transitions")?;
                    let name_str = match &name {
                        LuaValue::String(s) => Some(s.to_str()?.to_string()),
                        _ => None,
                    };
                    let is_disabled = if let Some(ref n) = name_str {
                        disabled.get::<LuaValue>(n.as_str())?.as_boolean().unwrap_or(false)
                    } else {
                        false
                    };

                    if !transitions || diff < 0.5 || is_disabled {
                        t.set(k_str.as_str(), dest)?;
                    } else {
                        let mut r = match rate {
                            LuaValue::Number(n) => n,
                            LuaValue::Integer(n) => n as f64,
                            _ => 0.5,
                        };
                        let fps: f64 = config.get("fps")?;
                        let anim_rate: f64 = config.get("animation_rate")?;
                        if fps != 60.0 || anim_rate != 1.0 {
                            let dt = 60.0 / fps;
                            r = 1.0 - (1.0 - r).clamp(1e-8, 1.0 - 1e-8).powf(anim_rate * dt);
                        }
                        let common: LuaTable = require_table(lua, "core.common")?;
                        let lerped: f64 = common.call_function("lerp", (val, dest, r))?;
                        t.set(k_str.as_str(), lerped)?;
                    }
                    if diff > 1e-8 {
                        let core: LuaTable = require_table(lua, "core")?;
                        core.set("redraw", true)?;
                    }
                    Ok(())
                })?,
            )?;

            // View:try_close(do_close)
            view.set(
                "try_close",
                lua.create_function(|_lua, (_this, do_close): (LuaTable, LuaFunction)| {
                    do_close.call::<()>(())
                })?,
            )?;

            // View:get_name()
            view.set(
                "get_name",
                lua.create_function(|_lua, _this: LuaTable| Ok("---"))?,
            )?;

            // View:get_scrollable_size()
            view.set(
                "get_scrollable_size",
                lua.create_function(|_lua, _this: LuaTable| Ok(f64::INFINITY))?,
            )?;

            // View:get_h_scrollable_size()
            view.set(
                "get_h_scrollable_size",
                lua.create_function(|_lua, _this: LuaTable| Ok(0.0))?,
            )?;

            // View:supports_text_input()
            view.set(
                "supports_text_input",
                lua.create_function(|_lua, _this: LuaTable| Ok(false))?,
            )?;

            // View:scrollbar_overlaps_point(x, y)
            view.set(
                "scrollbar_overlaps_point",
                lua.create_function(|_lua, (this, x, y): (LuaTable, f64, f64)| {
                    let v_sb: LuaTable = this.get("v_scrollbar")?;
                    let h_sb: LuaTable = this.get("h_scrollbar")?;
                    let v_result: LuaValue = v_sb.call_method("overlaps", (x, y))?;
                    let h_result: LuaValue = h_sb.call_method("overlaps", (x, y))?;
                    let v_overlaps = !matches!(v_result, LuaValue::Nil | LuaValue::Boolean(false));
                    let h_overlaps = !matches!(h_result, LuaValue::Nil | LuaValue::Boolean(false));
                    Ok(v_overlaps || h_overlaps)
                })?,
            )?;

            // View:scrollbar_dragging()
            view.set(
                "scrollbar_dragging",
                lua.create_function(|_lua, this: LuaTable| {
                    let v_sb: LuaTable = this.get("v_scrollbar")?;
                    let h_sb: LuaTable = this.get("h_scrollbar")?;
                    let v_dragging: bool = v_sb.get("dragging")?;
                    let h_dragging: bool = h_sb.get("dragging")?;
                    Ok(v_dragging || h_dragging)
                })?,
            )?;

            // View:scrollbar_hovering()
            view.set(
                "scrollbar_hovering",
                lua.create_function(|_lua, this: LuaTable| {
                    let v_sb: LuaTable = this.get("v_scrollbar")?;
                    let h_sb: LuaTable = this.get("h_scrollbar")?;
                    let v_hov: LuaTable = v_sb.get("hovering")?;
                    let h_hov: LuaTable = h_sb.get("hovering")?;
                    let v_track: bool = v_hov.get("track")?;
                    let h_track: bool = h_hov.get("track")?;
                    Ok(v_track || h_track)
                })?,
            )?;

            // View:on_mouse_pressed(button, x, y, clicks)
            view.set(
                "on_mouse_pressed",
                lua.create_function(
                    |_lua, (this, button, x, y, clicks): (LuaTable, LuaValue, f64, f64, LuaValue)| {
                        let scrollable: bool = this.get("scrollable")?;
                        if !scrollable {
                            return Ok(LuaValue::Nil);
                        }
                        let v_sb: LuaTable = this.get("v_scrollbar")?;
                        let result: LuaValue = v_sb.call_method(
                            "on_mouse_pressed",
                            (button.clone(), x, y, clicks.clone()),
                        )?;
                        if !matches!(result, LuaValue::Nil | LuaValue::Boolean(false)) {
                            if let LuaValue::Number(pct) = &result {
                                let scrollable_size: f64 =
                                    this.call_method("get_scrollable_size", ())?;
                                let size: LuaTable = this.get("size")?;
                                let size_y: f64 = size.get("y")?;
                                let scroll: LuaTable = this.get("scroll")?;
                                let scroll_to: LuaTable = scroll.get("to")?;
                                scroll_to.set("y", pct * (scrollable_size - size_y))?;
                            }
                            return Ok(LuaValue::Boolean(true));
                        }
                        let h_sb: LuaTable = this.get("h_scrollbar")?;
                        let result: LuaValue =
                            h_sb.call_method("on_mouse_pressed", (button, x, y, clicks))?;
                        if !matches!(result, LuaValue::Nil | LuaValue::Boolean(false)) {
                            if let LuaValue::Number(pct) = &result {
                                let h_scrollable_size: f64 =
                                    this.call_method("get_h_scrollable_size", ())?;
                                let size: LuaTable = this.get("size")?;
                                let size_x: f64 = size.get("x")?;
                                let scroll: LuaTable = this.get("scroll")?;
                                let scroll_to: LuaTable = scroll.get("to")?;
                                scroll_to.set("x", pct * (h_scrollable_size - size_x))?;
                            }
                            return Ok(LuaValue::Boolean(true));
                        }
                        Ok(LuaValue::Nil)
                    },
                )?,
            )?;

            // View:on_mouse_released(button, x, y)
            view.set(
                "on_mouse_released",
                lua.create_function(
                    |_lua, (this, button, x, y): (LuaTable, LuaValue, f64, f64)| {
                        let scrollable: bool = this.get("scrollable")?;
                        if !scrollable {
                            return Ok(());
                        }
                        let v_sb: LuaTable = this.get("v_scrollbar")?;
                        v_sb.call_method::<()>("on_mouse_released", (button.clone(), x, y))?;
                        let h_sb: LuaTable = this.get("h_scrollbar")?;
                        h_sb.call_method::<()>("on_mouse_released", (button, x, y))?;
                        Ok(())
                    },
                )?,
            )?;

            // View:on_mouse_moved(x, y, dx, dy)
            view.set(
                "on_mouse_moved",
                lua.create_function(
                    |lua, (this, x, y, dx, dy): (LuaTable, f64, f64, f64, f64)| {
                        let scrollable: bool = this.get("scrollable")?;
                        if !scrollable {
                            return Ok(LuaValue::Nil);
                        }
                        let h_sb: LuaTable = this.get("h_scrollbar")?;
                        let h_dragging: bool = h_sb.get("dragging")?;
                        if !h_dragging {
                            let v_sb: LuaTable = this.get("v_scrollbar")?;
                            let result: LuaValue =
                                v_sb.call_method("on_mouse_moved", (x, y, dx, dy))?;
                            if !matches!(result, LuaValue::Nil | LuaValue::Boolean(false)) {
                                if let LuaValue::Number(pct) = &result {
                                    let scrollable_size: f64 =
                                        this.call_method("get_scrollable_size", ())?;
                                    let size: LuaTable = this.get("size")?;
                                    let size_y: f64 = size.get("y")?;
                                    let scroll: LuaTable = this.get("scroll")?;
                                    let scroll_to: LuaTable = scroll.get("to")?;
                                    scroll_to.set("y", pct * (scrollable_size - size_y))?;
                                    let config: LuaTable = require_table(lua, "core.config")?;
                                    let animate: bool = config
                                        .get::<LuaValue>("animate_drag_scroll")?
                                        .as_boolean()
                                        .unwrap_or(true);
                                    if !animate {
                                        this.call_method::<()>("clamp_scroll_position", ())?;
                                        let scroll: LuaTable = this.get("scroll")?;
                                        let scroll_to: LuaTable = scroll.get("to")?;
                                        let to_y: f64 = scroll_to.get("y")?;
                                        scroll.set("y", to_y)?;
                                    }
                                }
                                h_sb.call_method::<()>("on_mouse_left", ())?;
                                return Ok(LuaValue::Boolean(true));
                            }
                        }
                        let result: LuaValue =
                            h_sb.call_method("on_mouse_moved", (x, y, dx, dy))?;
                        if !matches!(result, LuaValue::Nil | LuaValue::Boolean(false)) {
                            if let LuaValue::Number(pct) = &result {
                                let h_scrollable_size: f64 =
                                    this.call_method("get_h_scrollable_size", ())?;
                                let size: LuaTable = this.get("size")?;
                                let size_x: f64 = size.get("x")?;
                                let scroll: LuaTable = this.get("scroll")?;
                                let scroll_to: LuaTable = scroll.get("to")?;
                                scroll_to.set("x", pct * (h_scrollable_size - size_x))?;
                                let config: LuaTable = require_table(lua, "core.config")?;
                                let animate: bool = config
                                    .get::<LuaValue>("animate_drag_scroll")?
                                    .as_boolean()
                                    .unwrap_or(true);
                                if !animate {
                                    this.call_method::<()>("clamp_scroll_position", ())?;
                                    let scroll: LuaTable = this.get("scroll")?;
                                    let scroll_to: LuaTable = scroll.get("to")?;
                                    let to_x: f64 = scroll_to.get("x")?;
                                    scroll.set("x", to_x)?;
                                }
                            }
                            return Ok(LuaValue::Boolean(true));
                        }
                        Ok(LuaValue::Nil)
                    },
                )?,
            )?;

            // View:on_mouse_left()
            view.set(
                "on_mouse_left",
                lua.create_function(|_lua, this: LuaTable| {
                    let scrollable: bool = this.get("scrollable")?;
                    if !scrollable {
                        return Ok(());
                    }
                    let v_sb: LuaTable = this.get("v_scrollbar")?;
                    v_sb.call_method::<()>("on_mouse_left", ())?;
                    let h_sb: LuaTable = this.get("h_scrollbar")?;
                    h_sb.call_method::<()>("on_mouse_left", ())
                })?,
            )?;

            // View:on_file_dropped(filename, x, y)
            view.set(
                "on_file_dropped",
                lua.create_function(
                    |_lua, (_this, _filename, _x, _y): (LuaTable, LuaValue, f64, f64)| {
                        Ok(false)
                    },
                )?,
            )?;

            // View:on_text_input(text)
            view.set(
                "on_text_input",
                lua.create_function(|_lua, (_this, _text): (LuaTable, LuaValue)| Ok(()))?,
            )?;

            // View:on_ime_text_editing(text, start, length)
            view.set(
                "on_ime_text_editing",
                lua.create_function(
                    |_lua, (_this, _text, _start, _length): (LuaTable, LuaValue, LuaValue, LuaValue)| {
                        Ok(())
                    },
                )?,
            )?;

            // View:on_mouse_wheel(y, x)
            view.set(
                "on_mouse_wheel",
                lua.create_function(|_lua, (_this, _y, _x): (LuaTable, LuaValue, LuaValue)| {
                    Ok(LuaValue::Nil)
                })?,
            )?;

            // View:on_scale_change(new_scale, prev_scale)
            view.set(
                "on_scale_change",
                lua.create_function(|_lua, (_this, _new, _prev): (LuaTable, f64, f64)| Ok(()))?,
            )?;

            // View:get_content_bounds()
            view.set(
                "get_content_bounds",
                lua.create_function(|_lua, this: LuaTable| {
                    let scroll: LuaTable = this.get("scroll")?;
                    let sx: f64 = scroll.get("x")?;
                    let sy: f64 = scroll.get("y")?;
                    let size: LuaTable = this.get("size")?;
                    let size_x: f64 = size.get("x")?;
                    let size_y: f64 = size.get("y")?;
                    Ok((sx, sy, sx + size_x, sy + size_y))
                })?,
            )?;

            // View:on_touch_moved(x, y, dx, dy, i)
            view.set(
                "on_touch_moved",
                lua.create_function(
                    |_lua, (this, _x, y, dx, dy, _i): (LuaTable, f64, f64, f64, f64, LuaValue)| {
                        let scrollable: bool = this.get("scrollable")?;
                        if !scrollable {
                            return Ok(());
                        }
                        let dragging_scrollbar: bool =
                            this.get::<LuaValue>("dragging_scrollbar")?
                                .as_boolean()
                                .unwrap_or(false);
                        if dragging_scrollbar {
                            let scrollable_size: f64 =
                                this.call_method("get_scrollable_size", ())?;
                            let size: LuaTable = this.get("size")?;
                            let size_y: f64 = size.get("y")?;
                            let delta = scrollable_size / size_y * dy;
                            let scroll: LuaTable = this.get("scroll")?;
                            let scroll_to: LuaTable = scroll.get("to")?;
                            let to_y: f64 = scroll_to.get("y")?;
                            scroll_to.set("y", to_y + delta)?;
                        }
                        let overlaps: bool =
                            this.call_method("scrollbar_overlaps_point", (_x, y))?;
                        this.set("hovered_scrollbar", overlaps)?;

                        let scroll: LuaTable = this.get("scroll")?;
                        let scroll_to: LuaTable = scroll.get("to")?;
                        let to_y: f64 = scroll_to.get("y")?;
                        let to_x: f64 = scroll_to.get("x")?;
                        scroll_to.set("y", to_y - dy)?;
                        scroll_to.set("x", to_x - dx)?;
                        Ok(())
                    },
                )?,
            )?;

            // View:get_content_offset()
            view.set(
                "get_content_offset",
                lua.create_function(|lua, this: LuaTable| {
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let position: LuaTable = this.get("position")?;
                    let scroll: LuaTable = this.get("scroll")?;
                    let px: f64 = position.get("x")?;
                    let py: f64 = position.get("y")?;
                    let sx: f64 = scroll.get("x")?;
                    let sy: f64 = scroll.get("y")?;
                    let x: f64 = common.call_function("round", (px - sx,))?;
                    let y: f64 = common.call_function("round", (py - sy,))?;
                    Ok((x, y))
                })?,
            )?;

            // View:clamp_scroll_position()
            view.set(
                "clamp_scroll_position",
                lua.create_function(|lua, this: LuaTable| {
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let scroll: LuaTable = this.get("scroll")?;
                    let scroll_to: LuaTable = scroll.get("to")?;
                    let size: LuaTable = this.get("size")?;

                    let scrollable_size: f64 = this.call_method("get_scrollable_size", ())?;
                    let size_y: f64 = size.get("y")?;
                    let max_y = scrollable_size - size_y;
                    let to_y: f64 = scroll_to.get("y")?;
                    let clamped_y: f64 = common.call_function("clamp", (to_y, 0.0, max_y))?;
                    scroll_to.set("y", clamped_y)?;

                    let h_scrollable_size: f64 = this.call_method("get_h_scrollable_size", ())?;
                    let size_x: f64 = size.get("x")?;
                    let max_x = h_scrollable_size - size_x;
                    let to_x: f64 = scroll_to.get("x")?;
                    let clamped_x: f64 = common.call_function("clamp", (to_x, 0.0, max_x))?;
                    scroll_to.set("x", clamped_x)?;
                    Ok(())
                })?,
            )?;

            // View:update_scrollbar()
            view.set(
                "update_scrollbar",
                lua.create_function(|_lua, this: LuaTable| {
                    let position: LuaTable = this.get("position")?;
                    let px: f64 = position.get("x")?;
                    let py: f64 = position.get("y")?;
                    let size: LuaTable = this.get("size")?;
                    let sx: f64 = size.get("x")?;
                    let sy: f64 = size.get("y")?;
                    let scroll: LuaTable = this.get("scroll")?;

                    let v_scrollable: f64 = this.call_method("get_scrollable_size", ())?;
                    let v_sb: LuaTable = this.get("v_scrollbar")?;
                    v_sb.call_method::<()>("set_size", (px, py, sx, sy, v_scrollable))?;
                    let scroll_y: f64 = scroll.get("y")?;
                    let mut v_percent = scroll_y / (v_scrollable - sy);
                    // NaN check
                    if v_percent.is_nan() {
                        v_percent = 0.0;
                    }
                    v_sb.call_method::<()>("set_percent", (v_percent,))?;
                    v_sb.call_method::<()>("update", ())?;

                    let h_scrollable: f64 = this.call_method("get_h_scrollable_size", ())?;
                    let h_sb: LuaTable = this.get("h_scrollbar")?;
                    h_sb.call_method::<()>("set_size", (px, py, sx, sy, h_scrollable))?;
                    let scroll_x: f64 = scroll.get("x")?;
                    let mut h_percent = scroll_x / (h_scrollable - sx);
                    if h_percent.is_nan() {
                        h_percent = 0.0;
                    }
                    h_sb.call_method::<()>("set_percent", (h_percent,))?;
                    h_sb.call_method::<()>("update", ())?;
                    Ok(())
                })?,
            )?;

            // View:update()
            view.set("update", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let scale: f64 = lua.globals().get("SCALE")?;
                    let current_scale: f64 = this.get("current_scale")?;
                    if current_scale != scale {
                        this.call_method::<()>("on_scale_change", (scale, current_scale))?;
                        this.set("current_scale", scale)?;
                    }

                    this.call_method::<()>("clamp_scroll_position", ())?;
                    let scroll: LuaTable = this.get("scroll")?;
                    let scroll_to: LuaTable = scroll.get("to")?;
                    let to_x: f64 = scroll_to.get("x")?;
                    let to_y: f64 = scroll_to.get("y")?;

                    // use the class-level move_towards, not a potentially overridden one
                    let class: LuaTable = lua.registry_value(&k)?;
                    let move_towards: LuaFunction = class.get("move_towards")?;
                    move_towards.call::<()>((this.clone(), scroll.clone(), "x", to_x, 0.3, "scroll"))?;
                    move_towards.call::<()>((this.clone(), scroll, "y", to_y, 0.3, "scroll"))?;

                    let scrollable: bool = this.get("scrollable")?;
                    if !scrollable {
                        return Ok(());
                    }
                    this.call_method::<()>("update_scrollbar", ())
                })?
            })?;

            // View:draw_background(color)
            view.set(
                "draw_background",
                lua.create_function(|lua, (this, color): (LuaTable, LuaValue)| {
                    let position: LuaTable = this.get("position")?;
                    let x: f64 = position.get("x")?;
                    let y: f64 = position.get("y")?;
                    let size: LuaTable = this.get("size")?;
                    let w: f64 = size.get("x")?;
                    let h: f64 = size.get("y")?;
                    let renderer: LuaTable = lua.globals().get("renderer")?;
                    let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                    draw_rect.call::<()>((x, y, w, h, color))
                })?,
            )?;

            // View:draw_scrollbar()
            view.set(
                "draw_scrollbar",
                lua.create_function(|_lua, this: LuaTable| {
                    let v_sb: LuaTable = this.get("v_scrollbar")?;
                    v_sb.call_method::<()>("draw", ())?;
                    let h_sb: LuaTable = this.get("h_scrollbar")?;
                    h_sb.call_method::<()>("draw", ())
                })?,
            )?;

            // View:draw()
            view.set(
                "draw",
                lua.create_function(|_lua, _this: LuaTable| Ok(()))?,
            )?;

            // View:on_context_menu(x, y)
            view.set(
                "on_context_menu",
                lua.create_function(|_lua, (_this, _x, _y): (LuaTable, LuaValue, LuaValue)| {
                    Ok(LuaValue::Nil)
                })?,
            )?;

            Ok(LuaValue::Table(view))
        })?,
    )
}
