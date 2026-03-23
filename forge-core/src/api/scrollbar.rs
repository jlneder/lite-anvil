use mlua::prelude::*;

use std::sync::Arc;

/// Require a module by name, returning the loaded table.
fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers `core.scrollbar` — scrollbar geometry, thumb tracking, and drag state.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.scrollbar",
        lua.create_function(|lua, ()| {
            let object: LuaTable = require_table(lua, "core.object")?;
            let scrollbar = object.call_method::<LuaTable>("extend", ())?;

            scrollbar.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("Scrollbar"))?,
            )?;

            let class_key = Arc::new(lua.create_registry_value(scrollbar.clone())?);

            // Scrollbar:new(options)
            scrollbar.set("new", {
                let k = Arc::clone(&class_key);
                lua.create_function(move |lua, (this, options): (LuaTable, LuaTable)| {
                    let class: LuaTable = lua.registry_value(&k)?;
                    let super_tbl: LuaTable = class.get("super")?;
                    let super_new: LuaFunction = super_tbl.get("new")?;
                    super_new.call::<()>(this.clone())?;

                    let rect = lua.create_table()?;
                    rect.set("x", 0.0)?;
                    rect.set("y", 0.0)?;
                    rect.set("w", 0.0)?;
                    rect.set("h", 0.0)?;
                    rect.set("scrollable", 0.0)?;
                    this.set("rect", rect)?;

                    let nr = lua.create_table()?;
                    nr.set("across", 0.0)?;
                    nr.set("along", 0.0)?;
                    nr.set("across_size", 0.0)?;
                    nr.set("along_size", 0.0)?;
                    nr.set("scrollable", 0.0)?;
                    this.set("normal_rect", nr)?;

                    this.set("percent", 0.0)?;
                    this.set("dragging", false)?;
                    this.set("drag_start_offset", 0.0)?;

                    let hovering = lua.create_table()?;
                    hovering.set("track", false)?;
                    hovering.set("thumb", false)?;
                    this.set("hovering", hovering)?;

                    let direction: String = options
                        .get::<LuaValue>("direction")?
                        .as_string()
                        .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                        .unwrap_or_else(|| "v".to_string());
                    this.set("direction", direction)?;

                    let alignment: String = options
                        .get::<LuaValue>("alignment")?
                        .as_string()
                        .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                        .unwrap_or_else(|| "e".to_string());
                    this.set("alignment", alignment)?;

                    this.set("expand_percent", 0.0)?;

                    let force_status: LuaValue = options.get("force_status")?;
                    this.set("force_status", force_status.clone())?;
                    // set_forced_status inline
                    if let LuaValue::String(s) = &force_status {
                        if s.to_str()? == "expanded" {
                            this.set("expand_percent", 1.0)?;
                        }
                    }

                    let cs: LuaValue = options.get("contracted_size")?;
                    this.set("contracted_size", cs)?;
                    let es: LuaValue = options.get("expanded_size")?;
                    this.set("expanded_size", es)?;
                    let mts: LuaValue = options.get("minimum_thumb_size")?;
                    this.set("minimum_thumb_size", mts)?;
                    let cm: LuaValue = options.get("contracted_margin")?;
                    this.set("contracted_margin", cm)?;
                    let em: LuaValue = options.get("expanded_margin")?;
                    this.set("expanded_margin", em)?;

                    Ok(())
                })?
            })?;

            // Scrollbar:set_forced_status(status)
            scrollbar.set(
                "set_forced_status",
                lua.create_function(|_lua, (this, status): (LuaTable, LuaValue)| {
                    this.set("force_status", status.clone())?;
                    if let LuaValue::String(s) = &status {
                        if s.to_str()? == "expanded" {
                            this.set("expand_percent", 1.0)?;
                        }
                    }
                    Ok(())
                })?,
            )?;

            // Scrollbar:real_to_normal(x, y, w, h)
            scrollbar.set(
                "real_to_normal",
                lua.create_function(
                    |_lua,
                     (this, x, y, w, h): (
                        LuaTable,
                        Option<f64>,
                        Option<f64>,
                        Option<f64>,
                        Option<f64>,
                    )| {
                        let x = x.unwrap_or(0.0);
                        let y = y.unwrap_or(0.0);
                        let w = w.unwrap_or(0.0);
                        let h = h.unwrap_or(0.0);
                        let direction: String = this.get("direction")?;
                        let alignment: String = this.get("alignment")?;
                        if direction == "v" {
                            if alignment == "s" {
                                let rect: LuaTable = this.get("rect")?;
                                let rx: f64 = rect.get("x")?;
                                let rw: f64 = rect.get("w")?;
                                let x = (rx + rw) - x - w;
                                return Ok((x, y, w, h));
                            }
                            Ok((x, y, w, h))
                        } else {
                            if alignment == "s" {
                                let rect: LuaTable = this.get("rect")?;
                                let ry: f64 = rect.get("y")?;
                                let rh: f64 = rect.get("h")?;
                                let y = (ry + rh) - y - h;
                                return Ok((y, x, h, w));
                            }
                            Ok((y, x, h, w))
                        }
                    },
                )?,
            )?;

            // Scrollbar:normal_to_real(x, y, w, h)
            scrollbar.set(
                "normal_to_real",
                lua.create_function(
                    |_lua,
                     (this, x, y, w, h): (
                        LuaTable,
                        Option<f64>,
                        Option<f64>,
                        Option<f64>,
                        Option<f64>,
                    )| {
                        let x = x.unwrap_or(0.0);
                        let y = y.unwrap_or(0.0);
                        let w = w.unwrap_or(0.0);
                        let h = h.unwrap_or(0.0);
                        let direction: String = this.get("direction")?;
                        let alignment: String = this.get("alignment")?;
                        if direction == "v" {
                            if alignment == "s" {
                                let rect: LuaTable = this.get("rect")?;
                                let rx: f64 = rect.get("x")?;
                                let rw: f64 = rect.get("w")?;
                                let x = (rx + rw) - x - w;
                                return Ok((x, y, w, h));
                            }
                            Ok((x, y, w, h))
                        } else {
                            if alignment == "s" {
                                let rect: LuaTable = this.get("rect")?;
                                let ry: f64 = rect.get("y")?;
                                let rh: f64 = rect.get("h")?;
                                let x = (ry + rh) - x - w;
                                return Ok((y, x, h, w));
                            }
                            Ok((y, x, h, w))
                        }
                    },
                )?,
            )?;

            // Scrollbar:_get_thumb_rect_normal()
            scrollbar.set(
                "_get_thumb_rect_normal",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let nr: LuaTable = this.get("normal_rect")?;
                    let sz: f64 = nr.get("scrollable")?;
                    let along_size: f64 = nr.get("along_size")?;
                    if sz == f64::INFINITY || sz <= along_size {
                        return Ok((0.0, 0.0, 0.0, 0.0));
                    }
                    let contracted: f64 = match this.get::<LuaValue>("contracted_size")? {
                        LuaValue::Number(n) => n,
                        LuaValue::Integer(n) => n as f64,
                        _ => style.get("scrollbar_size")?,
                    };
                    let expanded: f64 = match this.get::<LuaValue>("expanded_size")? {
                        LuaValue::Number(n) => n,
                        LuaValue::Integer(n) => n as f64,
                        _ => style.get("expanded_scrollbar_size")?,
                    };
                    let min_thumb: f64 = match this.get::<LuaValue>("minimum_thumb_size")? {
                        LuaValue::Number(n) => n,
                        LuaValue::Integer(n) => n as f64,
                        _ => style.get("minimum_thumb_size")?,
                    };
                    let expand_pct: f64 = this.get("expand_percent")?;
                    let result_along_size = (along_size * along_size / sz).max(min_thumb);
                    let across_size = contracted + (expanded - contracted) * expand_pct;
                    let nr_across: f64 = nr.get("across")?;
                    let nr_across_size: f64 = nr.get("across_size")?;
                    let nr_along: f64 = nr.get("along")?;
                    let percent: f64 = this.get("percent")?;
                    Ok((
                        nr_across + nr_across_size - across_size,
                        nr_along + percent * (along_size - result_along_size),
                        across_size,
                        result_along_size,
                    ))
                })?,
            )?;

            // Scrollbar:get_thumb_rect()
            scrollbar.set(
                "get_thumb_rect",
                lua.create_function(|_lua, this: LuaTable| {
                    let (x, y, w, h): (f64, f64, f64, f64) =
                        this.call_method("_get_thumb_rect_normal", ())?;
                    let result: (f64, f64, f64, f64) =
                        this.call_method("normal_to_real", (x, y, w, h))?;
                    Ok(result)
                })?,
            )?;

            // Scrollbar:_get_track_rect_normal()
            scrollbar.set(
                "_get_track_rect_normal",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let nr: LuaTable = this.get("normal_rect")?;
                    let sz: f64 = nr.get("scrollable")?;
                    let along_size: f64 = nr.get("along_size")?;
                    if sz <= along_size || sz == f64::INFINITY {
                        return Ok((0.0, 0.0, 0.0, 0.0));
                    }
                    let contracted: f64 = match this.get::<LuaValue>("contracted_size")? {
                        LuaValue::Number(n) => n,
                        LuaValue::Integer(n) => n as f64,
                        _ => style.get("scrollbar_size")?,
                    };
                    let expanded: f64 = match this.get::<LuaValue>("expanded_size")? {
                        LuaValue::Number(n) => n,
                        LuaValue::Integer(n) => n as f64,
                        _ => style.get("expanded_scrollbar_size")?,
                    };
                    let expand_pct: f64 = this.get("expand_percent")?;
                    let across_size = contracted + (expanded - contracted) * expand_pct;
                    let nr_across: f64 = nr.get("across")?;
                    let nr_across_size: f64 = nr.get("across_size")?;
                    let nr_along: f64 = nr.get("along")?;
                    Ok((
                        nr_across + nr_across_size - across_size,
                        nr_along,
                        across_size,
                        along_size,
                    ))
                })?,
            )?;

            // Scrollbar:get_track_rect()
            scrollbar.set(
                "get_track_rect",
                lua.create_function(|_lua, this: LuaTable| {
                    let (x, y, w, h): (f64, f64, f64, f64) =
                        this.call_method("_get_track_rect_normal", ())?;
                    let result: (f64, f64, f64, f64) =
                        this.call_method("normal_to_real", (x, y, w, h))?;
                    Ok(result)
                })?,
            )?;

            // Scrollbar:_overlaps_normal(x, y)
            scrollbar.set(
                "_overlaps_normal",
                lua.create_function(|lua, (this, x, y): (LuaTable, f64, f64)| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let (sx, sy, sw, sh): (f64, f64, f64, f64) =
                        this.call_method("_get_thumb_rect_normal", ())?;
                    let expand_pct: f64 = this.get("expand_percent")?;
                    let expanded_margin: f64 =
                        match this.get::<LuaValue>("expanded_margin")? {
                            LuaValue::Number(n) => n,
                            LuaValue::Integer(n) => n as f64,
                            _ => style.get("expanded_scrollbar_margin")?,
                        };
                    let contracted_margin: f64 =
                        match this.get::<LuaValue>("contracted_margin")? {
                            LuaValue::Number(n) => n,
                            LuaValue::Integer(n) => n as f64,
                            _ => style.get("contracted_scrollbar_margin")?,
                        };
                    let scrollbar_margin =
                        expand_pct * expanded_margin + (1.0 - expand_pct) * contracted_margin;
                    if x >= sx - scrollbar_margin && x <= sx + sw && y >= sy && y <= sy + sh {
                        return Ok(LuaValue::String(lua.create_string("thumb")?));
                    }
                    let (sx, sy, sw, sh): (f64, f64, f64, f64) =
                        this.call_method("_get_track_rect_normal", ())?;
                    if x >= sx - scrollbar_margin && x <= sx + sw && y >= sy && y <= sy + sh {
                        return Ok(LuaValue::String(lua.create_string("track")?));
                    }
                    Ok(LuaValue::Nil)
                })?,
            )?;

            // Scrollbar:overlaps(x, y)
            scrollbar.set(
                "overlaps",
                lua.create_function(|_lua, (this, x, y): (LuaTable, f64, f64)| {
                    let (nx, ny, _, _): (f64, f64, f64, f64) =
                        this.call_method("real_to_normal", (x, y))?;
                    let result: LuaValue = this.call_method("_overlaps_normal", (nx, ny))?;
                    Ok(result)
                })?,
            )?;

            // Scrollbar:_on_mouse_pressed_normal(button, x, y, clicks)
            scrollbar.set(
                "_on_mouse_pressed_normal",
                lua.create_function(
                    |lua, (this, _button, _x, y, _clicks): (LuaTable, LuaValue, f64, f64, LuaValue)| {
                        let common: LuaTable = require_table(lua, "core.common")?;
                        let overlaps: LuaValue =
                            this.call_method("_overlaps_normal", (_x, y))?;
                        if matches!(overlaps, LuaValue::Nil) {
                            return Ok(LuaValue::Nil);
                        }
                        let (_, along, _, along_size): (f64, f64, f64, f64) =
                            this.call_method("_get_thumb_rect_normal", ())?;
                        this.set("dragging", true)?;
                        let overlaps_str = match &overlaps {
                            LuaValue::String(s) => s.to_str()?.to_string(),
                            _ => String::new(),
                        };
                        if overlaps_str == "thumb" {
                            this.set("drag_start_offset", along - y)?;
                            return Ok(LuaValue::Boolean(true));
                        } else if overlaps_str == "track" {
                            let nr: LuaTable = this.get("normal_rect")?;
                            let nr_along: f64 = nr.get("along")?;
                            let nr_along_size: f64 = nr.get("along_size")?;
                            this.set("drag_start_offset", -along_size / 2.0)?;
                            let clamped: f64 = common.call_function(
                                "clamp",
                                (
                                    (y - nr_along - along_size / 2.0)
                                        / (nr_along_size - along_size),
                                    0.0,
                                    1.0,
                                ),
                            )?;
                            return Ok(LuaValue::Number(clamped));
                        }
                        Ok(LuaValue::Nil)
                    },
                )?,
            )?;

            // Scrollbar:on_mouse_pressed(button, x, y, clicks)
            scrollbar.set(
                "on_mouse_pressed",
                lua.create_function(
                    |_lua, (this, button, x, y, clicks): (LuaTable, LuaValue, f64, f64, LuaValue)| {
                        if let LuaValue::String(s) = &button {
                            if s.to_str()? != "left" {
                                return Ok(LuaValue::Nil);
                            }
                        }
                        let (nx, ny, _, _): (f64, f64, f64, f64) =
                            this.call_method("real_to_normal", (x, y))?;
                        let result: LuaValue = this.call_method(
                            "_on_mouse_pressed_normal",
                            (button, nx, ny, clicks),
                        )?;
                        Ok(result)
                    },
                )?,
            )?;

            // Scrollbar:_update_hover_status_normal(x, y)
            scrollbar.set(
                "_update_hover_status_normal",
                lua.create_function(|_lua, (this, x, y): (LuaTable, f64, f64)| {
                    let overlaps: LuaValue = this.call_method("_overlaps_normal", (x, y))?;
                    let overlaps_str = match &overlaps {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => String::new(),
                    };
                    let is_thumb = overlaps_str == "thumb";
                    let is_track = is_thumb || overlaps_str == "track";
                    let hovering: LuaTable = this.get("hovering")?;
                    hovering.set("thumb", is_thumb)?;
                    hovering.set("track", is_track)?;
                    Ok(is_track || is_thumb)
                })?,
            )?;

            // Scrollbar:_on_mouse_released_normal(button, x, y)
            scrollbar.set(
                "_on_mouse_released_normal",
                lua.create_function(
                    |_lua, (this, _button, x, y): (LuaTable, LuaValue, f64, f64)| {
                        this.set("dragging", false)?;
                        let result: LuaValue =
                            this.call_method("_update_hover_status_normal", (x, y))?;
                        Ok(result)
                    },
                )?,
            )?;

            // Scrollbar:on_mouse_released(button, x, y)
            scrollbar.set(
                "on_mouse_released",
                lua.create_function(
                    |_lua, (this, button, x, y): (LuaTable, LuaValue, f64, f64)| {
                        if let LuaValue::String(s) = &button {
                            if s.to_str()? != "left" {
                                return Ok(LuaValue::Nil);
                            }
                        }
                        let (nx, ny, _, _): (f64, f64, f64, f64) =
                            this.call_method("real_to_normal", (x, y))?;
                        let result: LuaValue =
                            this.call_method("_on_mouse_released_normal", (button, nx, ny))?;
                        Ok(result)
                    },
                )?,
            )?;

            // Scrollbar:_on_mouse_moved_normal(x, y, dx, dy)
            scrollbar.set(
                "_on_mouse_moved_normal",
                lua.create_function(
                    |lua, (this, x, y, _dx, _dy): (LuaTable, f64, f64, f64, f64)| {
                        let dragging: bool = this.get("dragging")?;
                        if dragging {
                            let common: LuaTable = require_table(lua, "core.common")?;
                            let nr: LuaTable = this.get("normal_rect")?;
                            let (_, _, _, along_size): (f64, f64, f64, f64) =
                                this.call_method("_get_thumb_rect_normal", ())?;
                            let nr_along: f64 = nr.get("along")?;
                            let nr_along_size: f64 = nr.get("along_size")?;
                            let drag_start_offset: f64 = this.get("drag_start_offset")?;
                            let clamped: f64 = common.call_function(
                                "clamp",
                                (
                                    (y - nr_along + drag_start_offset)
                                        / (nr_along_size - along_size),
                                    0.0,
                                    1.0,
                                ),
                            )?;
                            return Ok(LuaValue::Number(clamped));
                        }
                        let result: LuaValue =
                            this.call_method("_update_hover_status_normal", (x, y))?;
                        Ok(result)
                    },
                )?,
            )?;

            // Scrollbar:on_mouse_moved(x, y, dx, dy)
            scrollbar.set(
                "on_mouse_moved",
                lua.create_function(
                    |_lua, (this, x, y, dx, dy): (LuaTable, f64, f64, f64, f64)| {
                        let (nx, ny, _, _): (f64, f64, f64, f64) =
                            this.call_method("real_to_normal", (x, y))?;
                        let (ndx, ndy, _, _): (f64, f64, f64, f64) =
                            this.call_method("real_to_normal", (dx, dy))?;
                        let result: LuaValue =
                            this.call_method("_on_mouse_moved_normal", (nx, ny, ndx, ndy))?;
                        Ok(result)
                    },
                )?,
            )?;

            // Scrollbar:on_mouse_left()
            scrollbar.set(
                "on_mouse_left",
                lua.create_function(|_lua, this: LuaTable| {
                    let hovering: LuaTable = this.get("hovering")?;
                    hovering.set("track", false)?;
                    hovering.set("thumb", false)?;
                    Ok(())
                })?,
            )?;

            // Scrollbar:set_size(x, y, w, h, scrollable)
            scrollbar.set(
                "set_size",
                lua.create_function(
                    |_lua, (this, x, y, w, h, scrollable): (LuaTable, f64, f64, f64, f64, f64)| {
                        let rect: LuaTable = this.get("rect")?;
                        rect.set("x", x)?;
                        rect.set("y", y)?;
                        rect.set("w", w)?;
                        rect.set("h", h)?;
                        rect.set("scrollable", scrollable)?;

                        let nr: LuaTable = this.get("normal_rect")?;
                        let (across, along, across_size, along_size): (f64, f64, f64, f64) =
                            this.call_method("real_to_normal", (x, y, w, h))?;
                        nr.set("across", across)?;
                        nr.set("along", along)?;
                        nr.set("across_size", across_size)?;
                        nr.set("along_size", along_size)?;
                        nr.set("scrollable", scrollable)?;
                        Ok(())
                    },
                )?,
            )?;

            // Scrollbar:set_percent(percent)
            scrollbar.set(
                "set_percent",
                lua.create_function(|_lua, (this, percent): (LuaTable, f64)| {
                    this.set("percent", percent)
                })?,
            )?;

            // Scrollbar:update()
            scrollbar.set(
                "update",
                lua.create_function(|lua, this: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let config: LuaTable = require_table(lua, "core.config")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let force_status: LuaValue = this.get("force_status")?;

                    if matches!(force_status, LuaValue::Nil | LuaValue::Boolean(false)) {
                        let hovering: LuaTable = this.get("hovering")?;
                        let track_hover: bool = hovering.get("track")?;
                        let dragging: bool = this.get("dragging")?;
                        let dest: f64 = if track_hover || dragging { 1.0 } else { 0.0 };
                        let expand_pct: f64 = this.get("expand_percent")?;
                        let diff = (expand_pct - dest).abs();

                        let transitions: bool =
                            config.get::<LuaValue>("transitions")?.as_boolean().unwrap_or(true);
                        let disabled: LuaTable = config.get("disabled_transitions")?;
                        let scroll_disabled: bool = disabled
                            .get::<LuaValue>("scroll")?
                            .as_boolean()
                            .unwrap_or(false);

                        if !transitions || diff < 0.05 || scroll_disabled {
                            this.set("expand_percent", dest)?;
                        } else {
                            let mut rate = 0.3_f64;
                            let fps: f64 = config.get("fps")?;
                            let anim_rate: f64 = config.get("animation_rate")?;
                            if fps != 60.0 || anim_rate != 1.0 {
                                let dt = 60.0 / fps;
                                rate = 1.0
                                    - (1.0 - rate).clamp(1e-8, 1.0 - 1e-8).powf(anim_rate * dt);
                            }
                            let lerped: f64 =
                                common.call_function("lerp", (expand_pct, dest, rate))?;
                            this.set("expand_percent", lerped)?;
                        }
                        if diff > 1e-8 {
                            core.set("redraw", true)?;
                        }
                    } else if let LuaValue::String(s) = &force_status {
                        let s = s.to_str()?.to_string();
                        if s == "expanded" {
                            this.set("expand_percent", 1.0)?;
                        } else if s == "contracted" {
                            this.set("expand_percent", 0.0)?;
                        }
                    }
                    Ok(())
                })?,
            )?;

            // Scrollbar:draw_track()
            scrollbar.set(
                "draw_track",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let hovering: LuaTable = this.get("hovering")?;
                    let track_hover: bool = hovering.get("track")?;
                    let dragging: bool = this.get("dragging")?;
                    let expand_pct: f64 = this.get("expand_percent")?;
                    if !(track_hover || dragging) && expand_pct == 0.0 {
                        return Ok(());
                    }
                    let track_color: LuaTable = style.get("scrollbar_track")?;
                    let color = lua.create_table()?;
                    for i in 1..=track_color.raw_len() {
                        let v: LuaValue = track_color.raw_get(i as i64)?;
                        color.raw_set(i as i64, v)?;
                    }
                    let a4: f64 = color.raw_get(4)?;
                    color.raw_set(4, a4 * expand_pct)?;
                    let (x, y, w, h): (f64, f64, f64, f64) =
                        this.call_method("get_track_rect", ())?;
                    if !x.is_nan()
                        && !y.is_nan()
                        && !w.is_nan()
                        && !h.is_nan()
                        && x.abs() < 2_147_480_000.0
                        && y.abs() < 2_147_480_000.0
                        && w.abs() < 2_147_480_000.0
                        && h.abs() < 2_147_480_000.0
                        && w > 0.0
                        && h > 0.0
                    {
                        let renderer: LuaTable =
                            lua.globals().get::<LuaTable>("renderer")?;
                        let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                        draw_rect.call::<()>((x, y, w, h, color))?;
                    }
                    Ok(())
                })?,
            )?;

            // Scrollbar:draw_thumb()
            scrollbar.set(
                "draw_thumb",
                lua.create_function(|lua, this: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let hovering: LuaTable = this.get("hovering")?;
                    let thumb_hover: bool = hovering.get("thumb")?;
                    let dragging: bool = this.get("dragging")?;
                    let highlight = thumb_hover || dragging;
                    let color: LuaValue = if highlight {
                        style.get("scrollbar2")?
                    } else {
                        style.get("scrollbar")?
                    };
                    let (x, y, w, h): (f64, f64, f64, f64) =
                        this.call_method("get_thumb_rect", ())?;
                    if !x.is_nan()
                        && !y.is_nan()
                        && !w.is_nan()
                        && !h.is_nan()
                        && x.abs() < 2_147_480_000.0
                        && y.abs() < 2_147_480_000.0
                        && w.abs() < 2_147_480_000.0
                        && h.abs() < 2_147_480_000.0
                        && w > 0.0
                        && h > 0.0
                    {
                        let renderer: LuaTable =
                            lua.globals().get::<LuaTable>("renderer")?;
                        let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                        draw_rect.call::<()>((x, y, w, h, color))?;
                    }
                    Ok(())
                })?,
            )?;

            // Scrollbar:draw()
            scrollbar.set(
                "draw",
                lua.create_function(|_lua, this: LuaTable| {
                    this.call_method::<()>("draw_track", ())?;
                    this.call_method::<()>("draw_thumb", ())
                })?,
            )?;

            Ok(LuaValue::Table(scrollbar))
        })?,
    )
}
