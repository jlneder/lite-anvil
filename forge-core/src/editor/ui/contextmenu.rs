use mlua::prelude::*;

use std::sync::Arc;

/// Require a module by name, returning the loaded table.
fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers `core.contextmenu` — right-click context menu with item draw and click routing.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.contextmenu",
        lua.create_function(|lua, ()| {
            let view_class: LuaTable = require_table(lua, "core.view")?;
            let context_menu = view_class.call_method::<LuaTable>("extend", ())?;

            context_menu.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("ContextMenu"))?,
            )?;

            // DIVIDER sentinel table
            let divider = lua.create_table()?;
            context_menu.set("DIVIDER", divider.clone())?;
            let divider_key = Arc::new(lua.create_registry_value(divider)?);

            let _class_key = Arc::new(lua.create_registry_value(context_menu.clone())?);

            let border_width = 1.0_f64;
            let divider_width = 1.0_f64;
            let divider_padding = 5.0_f64;

            // get_item_size helper
            let divider_k2 = Arc::clone(&divider_key);
            let get_item_size = lua.create_function(move |lua, item: LuaValue| {
                let style: LuaTable = require_table(lua, "core.style")?;
                let scale: f64 = lua.globals().get("SCALE")?;
                let divider: LuaTable = lua.registry_value(&divider_k2)?;
                let is_divider = match &item {
                    LuaValue::Table(t) => *t == divider,
                    _ => false,
                };
                if is_divider {
                    return Ok((0.0, divider_width + divider_padding * scale * 2.0));
                }
                let item_tbl = match &item {
                    LuaValue::Table(t) => t,
                    _ => return Ok((0.0, 0.0)),
                };
                let font: LuaValue = style.get("font")?;
                let text: String = item_tbl.get("text")?;
                let mut lw: f64 = match &font {
                    LuaValue::Table(t) => t.call_method("get_width", text)?,
                    LuaValue::UserData(ud) => ud.call_method("get_width", text)?,
                    _ => 40.0,
                };
                let info: LuaValue = item_tbl.get("info")?;
                if let LuaValue::String(info_str) = info {
                    let padding: LuaTable = style.get("padding")?;
                    let px: f64 = padding.get("x")?;
                    let info_w: f64 = match &font {
                        LuaValue::Table(t) => {
                            t.call_method("get_width", info_str.to_str()?.to_string())?
                        }
                        LuaValue::UserData(ud) => {
                            ud.call_method("get_width", info_str.to_str()?.to_string())?
                        }
                        _ => 20.0,
                    };
                    lw += px + info_w;
                }
                let fh: f64 = match &font {
                    LuaValue::Table(t) => t.call_method("get_height", ())?,
                    LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                    _ => 14.0,
                };
                let padding: LuaTable = style.get("padding")?;
                let py: f64 = padding.get("y")?;
                Ok((lw, fh + py))
            })?;
            let gis_key = Arc::new(lua.create_registry_value(get_item_size)?);

            // update_items_size helper
            let gis_k2 = Arc::clone(&gis_key);
            let div_k3 = Arc::clone(&divider_key);
            let update_items_size =
                lua.create_function(move |lua, (items, update_binding): (LuaTable, bool)| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let padding: LuaTable = style.get("padding")?;
                    let px: f64 = padding.get("x")?;
                    let keymap: LuaTable = require_table(lua, "core.keymap")?;
                    let gis_fn: LuaFunction = lua.registry_value(&gis_k2)?;
                    let divider: LuaTable = lua.registry_value(&div_k3)?;
                    let mut width = 0.0f64;
                    let mut height = 0.0f64;
                    let len = items.raw_len() as i64;
                    for i in 1..=len {
                        let item: LuaValue = items.raw_get(i)?;
                        let is_divider = match &item {
                            LuaValue::Table(t) => *t == divider,
                            _ => false,
                        };
                        if update_binding && !is_divider {
                            if let LuaValue::Table(ref item_tbl) = item {
                                let cmd: LuaValue = item_tbl.get("command")?;
                                if let LuaValue::String(_) = cmd {
                                    let binding: LuaValue =
                                        keymap.call_function("get_binding_display", cmd)?;
                                    item_tbl.set("info", binding)?;
                                }
                            }
                        }
                        let (lw, lh): (f64, f64) = gis_fn.call(item.clone())?;
                        if let LuaValue::Table(ref item_tbl) = item {
                            item_tbl.set("cm_width", lw)?;
                            item_tbl.set("cm_height", lh)?;
                            item_tbl.set("cm_y", height)?;
                        }
                        if lw > width {
                            width = lw;
                        }
                        height += lh;
                    }
                    width += px * 2.0;
                    items.set("width", width)?;
                    items.set("height", height)?;
                    Ok(())
                })?;
            let uis_key = Arc::new(lua.create_registry_value(update_items_size)?);

            // ContextMenu:new()
            context_menu.set(
                "new",
                lua.create_function(|lua, this: LuaTable| {
                    let scale: f64 = lua.globals().get("SCALE")?;
                    this.set("visible", false)?;
                    this.set("selected", -1)?;
                    this.set("height", 0.0)?;
                    let pos = lua.create_table()?;
                    pos.set("x", 0.0)?;
                    pos.set("y", 0.0)?;
                    this.set("position", pos)?;
                    this.set("current_scale", scale)?;
                    Ok(())
                })?,
            )?;

            // ContextMenu:show(x, y, items, ...)
            context_menu.set("show", {
                let div_k = Arc::clone(&divider_key);
                let uis_k = Arc::clone(&uis_key);
                lua.create_function(
                    move |lua,
                          (this, x, y, items, rest): (
                        LuaTable,
                        f64,
                        f64,
                        LuaTable,
                        LuaMultiValue,
                    )| {
                        let command: LuaTable = require_table(lua, "core.command")?;
                        let common: LuaTable = require_table(lua, "core.common")?;
                        let core: LuaTable = require_table(lua, "core")?;
                        let style: LuaTable = require_table(lua, "core.style")?;
                        let padding: LuaTable = style.get("padding")?;
                        let px: f64 = padding.get("x")?;
                        let divider: LuaTable = lua.registry_value(&div_k)?;

                        let items_list = lua.create_table()?;
                        items_list.set("width", 0.0)?;
                        items_list.set("height", 0.0)?;

                        let args_tbl = lua.create_table()?;
                        for (i, v) in rest.into_iter().enumerate() {
                            args_tbl.raw_set((i + 1) as i64, v)?;
                        }
                        items_list.set("arguments", args_tbl)?;

                        let len = items.raw_len() as i64;
                        let mut count = 0i64;
                        for i in 1..=len {
                            let item: LuaValue = items.raw_get(i)?;
                            if matches!(item, LuaValue::Nil | LuaValue::Boolean(false)) {
                                continue;
                            }
                            let is_divider = match &item {
                                LuaValue::Table(t) => *t == divider,
                                _ => false,
                            };
                            let should_add = if is_divider {
                                true
                            } else if let LuaValue::Table(ref item_tbl) = item {
                                let cmd: LuaValue = item_tbl.get("command")?;
                                if matches!(cmd, LuaValue::Nil) {
                                    true
                                } else if let LuaValue::String(ref s) = cmd {
                                    let args_list: LuaTable = items_list.get("arguments")?;
                                    let valid: bool = command.call_function(
                                        "is_valid",
                                        (s.to_str()?.to_string(), args_list),
                                    )?;
                                    valid
                                } else {
                                    true
                                }
                            } else {
                                false
                            };
                            if should_add {
                                count += 1;
                                items_list.raw_set(count, item)?;
                            }
                        }

                        if count > 0 {
                            this.set("items", items_list.clone())?;
                            let uis_fn: LuaFunction = lua.registry_value(&uis_k)?;
                            uis_fn.call::<()>((items_list.clone(), true))?;
                            let w: f64 = items_list.get("width")?;
                            let h: f64 = items_list.get("height")?;
                            let root_view: LuaTable = core.get("root_view")?;
                            let rv_size: LuaTable = root_view.get("size")?;
                            let rv_x: f64 = rv_size.get("x")?;
                            let rv_y: f64 = rv_size.get("y")?;
                            let cx: f64 = common.call_function("clamp", (x, 0.0, rv_x - w - px))?;
                            let cy: f64 = common.call_function("clamp", (y, 0.0, rv_y - h))?;
                            let pos: LuaTable = this.get("position")?;
                            pos.set("x", cx)?;
                            pos.set("y", cy)?;
                            this.set("visible", true)?;
                            core.call_function::<()>("set_active_view", this.clone())?;
                            core.call_function::<()>("request_cursor", "arrow")?;
                            return Ok(LuaValue::Boolean(true));
                        }
                        Ok(LuaValue::Boolean(false))
                    },
                )?
            })?;

            // ContextMenu:hide()
            context_menu.set(
                "hide",
                lua.create_function(|lua, this: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    this.set("visible", false)?;
                    this.set("items", LuaValue::Nil)?;
                    this.set("selected", -1)?;
                    this.set("height", 0.0)?;
                    let active_view: LuaValue = core.get("active_view")?;
                    if let LuaValue::Table(ref av) = active_view {
                        if *av == this {
                            let last_av: LuaValue = core.get("last_active_view")?;
                            core.call_function::<()>("set_active_view", last_av)?;
                        }
                    }
                    let active_view: LuaTable = core.get("active_view")?;
                    let cursor: LuaValue = active_view.get("cursor")?;
                    core.call_function::<()>("request_cursor", cursor)?;
                    Ok(())
                })?,
            )?;

            // ContextMenu:each_item() - returns a stateful iterator, no coroutine.yield.
            context_menu.set("each_item", {
                let gis = Arc::clone(&gis_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let gis2 = Arc::clone(&gis);
                    let items: LuaTable = this.get("items")?;
                    let x: f64 = {
                        let pos: LuaTable = this.get("position")?;
                        pos.get("x")?
                    };
                    let y: f64 = {
                        let pos: LuaTable = this.get("position")?;
                        pos.get("y")?
                    };
                    let w: f64 = items.get("width")?;
                    let height: f64 = this.get("height")?;
                    let gis_fn: LuaFunction = lua.registry_value(&gis2)?;
                    let len = items.raw_len() as i64;

                    let results = lua.create_table()?;
                    let mut count = 0i64;
                    for i in 1..=len {
                        let item: LuaValue = items.raw_get(i)?;
                        let cm_y: f64 = match &item {
                            LuaValue::Table(t) => {
                                t.get::<LuaValue>("cm_y")?.as_number().unwrap_or(0.0)
                            }
                            _ => 0.0,
                        };
                        let item_y = y + cm_y;
                        let lh: f64 = match &item {
                            LuaValue::Table(t) => {
                                let v: LuaValue = t.get("cm_height")?;
                                match v {
                                    LuaValue::Number(n) => n,
                                    LuaValue::Integer(n) => n as f64,
                                    _ => {
                                        let (_, h): (f64, f64) = gis_fn.call(item.clone())?;
                                        h
                                    }
                                }
                            }
                            _ => {
                                let (_, h): (f64, f64) = gis_fn.call(item.clone())?;
                                h
                            }
                        };
                        if item_y - y > height {
                            break;
                        }
                        count += 1;
                        let entry = lua.create_table()?;
                        entry.raw_set(1, i)?;
                        entry.raw_set(2, item)?;
                        entry.raw_set(3, x)?;
                        entry.raw_set(4, item_y)?;
                        entry.raw_set(5, w)?;
                        entry.raw_set(6, lh)?;
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
                            let item: LuaValue = entry.raw_get(2)?;
                            let ex: LuaValue = entry.raw_get(3)?;
                            let ey: LuaValue = entry.raw_get(4)?;
                            let ew: LuaValue = entry.raw_get(5)?;
                            let eh: LuaValue = entry.raw_get(6)?;
                            Ok(LuaMultiValue::from_vec(vec![i, item, ex, ey, ew, eh]))
                        })?;
                    Ok(iterator)
                })?
            })?;

            // ContextMenu:on_mouse_pressed(button, x, y)
            context_menu.set(
                "on_mouse_pressed",
                lua.create_function(
                    |lua, (this, button, x, y): (LuaTable, LuaValue, f64, f64)| {
                        let visible: bool = this.get("visible")?;
                        if !visible {
                            return Ok(LuaValue::Boolean(false));
                        }
                        let button_str = match &button {
                            LuaValue::String(s) => s.to_str()?.to_string(),
                            _ => String::new(),
                        };
                        if button_str == "left" {
                            let pos: LuaTable = this.get("position")?;
                            let px: f64 = pos.get("x")?;
                            let py: f64 = pos.get("y")?;
                            let items: LuaTable = this.get("items")?;
                            let w: f64 = items.get("width")?;
                            let h: f64 = this.get("height")?;
                            if x >= px && y >= py && x < px + w && y < py + h {
                                let item: LuaValue = this.call_method("get_item_selected", ())?;
                                if matches!(item, LuaValue::Nil | LuaValue::Boolean(false)) {
                                    this.call_method::<()>("hide", ())?;
                                    return Ok(LuaValue::Boolean(true));
                                }
                                if let LuaValue::Table(ref item_tbl) = item {
                                    let cmd: LuaValue = item_tbl.get("command")?;
                                    if matches!(cmd, LuaValue::Nil) {
                                        this.call_method::<()>("hide", ())?;
                                        return Ok(LuaValue::Boolean(true));
                                    }
                                }
                                let core: LuaTable = require_table(lua, "core")?;
                                let active_view: LuaValue = core.get("active_view")?;
                                if let LuaValue::Table(ref av) = active_view {
                                    if *av == this {
                                        let last_av: LuaValue = core.get("last_active_view")?;
                                        core.call_function::<()>("set_active_view", last_av)?;
                                    }
                                }
                                this.call_method::<()>("on_selected", item)?;
                            }
                        }
                        this.call_method::<()>("hide", ())?;
                        Ok(LuaValue::Boolean(true))
                    },
                )?,
            )?;

            // ContextMenu:on_mouse_released(button, x, y)
            context_menu.set(
                "on_mouse_released",
                lua.create_function(
                    |_lua, (this, _button, _x, _y): (LuaTable, LuaValue, f64, f64)| {
                        let visible: bool = this.get("visible")?;
                        if !visible {
                            return Ok(LuaValue::Boolean(false));
                        }
                        Ok(LuaValue::Boolean(true))
                    },
                )?,
            )?;

            // ContextMenu:on_mouse_moved(px, py)
            context_menu.set(
                "on_mouse_moved",
                lua.create_function(|lua, (this, px, py): (LuaTable, f64, f64)| {
                    let visible: bool = this.get("visible")?;
                    if !visible {
                        return Ok(LuaValue::Boolean(false));
                    }
                    this.set("selected", -1)?;
                    let iter: LuaFunction = this.call_method("each_item", ())?;
                    loop {
                        let results: LuaMultiValue = iter.call(())?;
                        let mut vals = results.into_iter();
                        let i_val = match vals.next() {
                            Some(v) if !matches!(v, LuaValue::Nil) => v,
                            _ => break,
                        };
                        let _ = vals.next(); // item
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
                        if px > x && px <= x + w && py > y && py <= y + h {
                            this.set("selected", i_val)?;
                            let core: LuaTable = require_table(lua, "core")?;
                            core.call_function::<()>("request_cursor", "arrow")?;
                            break;
                        }
                    }
                    Ok(LuaValue::Boolean(true))
                })?,
            )?;

            // ContextMenu:on_selected(item)
            context_menu.set(
                "on_selected",
                lua.create_function(|lua, (this, item): (LuaTable, LuaTable)| {
                    let cmd: LuaValue = item.get("command")?;
                    let items: LuaTable = this.get("items")?;
                    let arguments: LuaTable = items.get("arguments")?;
                    let table_unpack: LuaFunction =
                        lua.globals().get::<LuaTable>("table")?.get("unpack")?;
                    if let LuaValue::String(s) = cmd {
                        let command: LuaTable = require_table(lua, "core.command")?;
                        let args: LuaMultiValue = table_unpack.call(arguments)?;
                        let mut call_args = vec![LuaValue::String(s)];
                        for v in args {
                            call_args.push(v);
                        }
                        command
                            .call_function::<()>("perform", LuaMultiValue::from_vec(call_args))?;
                    } else if let LuaValue::Function(f) = cmd {
                        let args: LuaMultiValue = table_unpack.call(arguments)?;
                        f.call::<()>(args)?;
                    }
                    Ok(())
                })?,
            )?;

            // ContextMenu:focus_previous()
            context_menu.set("focus_previous", {
                let div_k = Arc::clone(&divider_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let items: LuaTable = this.get("items")?;
                    let n = items.raw_len() as i64;
                    let selected: i64 = this.get("selected")?;
                    let new_sel = if selected == -1 || selected == 1 {
                        n
                    } else {
                        selected - 1
                    };
                    this.set("selected", new_sel)?;
                    let divider: LuaTable = lua.registry_value(&div_k)?;
                    let sel_item: LuaValue = items.raw_get(new_sel)?;
                    let is_div = match &sel_item {
                        LuaValue::Table(t) => *t == divider,
                        _ => false,
                    };
                    if is_div {
                        let ns: i64 = this.get("selected")?;
                        this.set("selected", ns - 1)?;
                    }
                    Ok(())
                })?
            })?;

            // ContextMenu:focus_next()
            context_menu.set("focus_next", {
                let div_k = Arc::clone(&divider_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let items: LuaTable = this.get("items")?;
                    let n = items.raw_len() as i64;
                    let selected: i64 = this.get("selected")?;
                    let new_sel = if selected == -1 || selected == n {
                        1
                    } else {
                        selected + 1
                    };
                    this.set("selected", new_sel)?;
                    let divider: LuaTable = lua.registry_value(&div_k)?;
                    let sel_item: LuaValue = items.raw_get(new_sel)?;
                    let is_div = match &sel_item {
                        LuaValue::Table(t) => *t == divider,
                        _ => false,
                    };
                    if is_div {
                        let ns: i64 = this.get("selected")?;
                        this.set("selected", ns + 1)?;
                    }
                    Ok(())
                })?
            })?;

            // ContextMenu:get_item_selected()
            context_menu.set(
                "get_item_selected",
                lua.create_function(|_lua, this: LuaTable| {
                    let items: LuaValue = this.get("items")?;
                    let items = match items {
                        LuaValue::Table(t) => t,
                        _ => return Ok(LuaValue::Nil),
                    };
                    let selected: i64 = this.get("selected")?;
                    let result: LuaValue = items.raw_get(selected)?;
                    Ok(result)
                })?,
            )?;

            // ContextMenu.move_towards = View.move_towards
            let view_mt: LuaValue = view_class.get("move_towards")?;
            context_menu.set("move_towards", view_mt)?;

            // ContextMenu:update()
            context_menu.set(
                "update",
                lua.create_function(|_lua, this: LuaTable| {
                    let visible: bool = this.get("visible")?;
                    if visible {
                        let items: LuaTable = this.get("items")?;
                        let items_height: f64 = items.get("height")?;
                        this.call_method::<()>(
                            "move_towards",
                            (this.clone(), "height", items_height),
                        )?;
                    }
                    Ok(())
                })?,
            )?;

            // ContextMenu:draw()
            context_menu.set("draw", {
                let uis = Arc::clone(&uis_key);
                let div_k = Arc::clone(&divider_key);
                lua.create_function(move |lua, this: LuaTable| {
                    let visible: bool = this.get("visible")?;
                    if !visible {
                        return Ok(());
                    }
                    let scale: f64 = lua.globals().get("SCALE")?;
                    let current_scale: f64 = this.get("current_scale")?;
                    if current_scale != scale {
                        let items: LuaValue = this.get("items")?;
                        if let LuaValue::Table(items_tbl) = items {
                            let uis_fn: LuaFunction = lua.registry_value(&uis)?;
                            uis_fn.call::<()>((items_tbl, false))?;
                        }
                        this.set("current_scale", scale)?;
                    }
                    let items: LuaValue = this.get("items")?;
                    if matches!(items, LuaValue::Nil) {
                        return Ok(());
                    }
                    let items = match items {
                        LuaValue::Table(t) => t,
                        _ => return Ok(()),
                    };

                    let style: LuaTable = require_table(lua, "core.style")?;
                    let common: LuaTable = require_table(lua, "core.common")?;
                    let renderer: LuaTable = lua.globals().get("renderer")?;
                    let draw_rect: LuaFunction = renderer.get("draw_rect")?;
                    let padding: LuaTable = style.get("padding")?;
                    let px: f64 = padding.get("x")?;

                    let pos: LuaTable = this.get("position")?;
                    let bx: f64 = pos.get("x")?;
                    let by: f64 = pos.get("y")?;
                    let bw: f64 = items.get("width")?;
                    let bh: f64 = this.get("height")?;
                    let divider_tbl: LuaTable = lua.registry_value(&div_k)?;

                    let style_divider: LuaValue = style.get("divider")?;
                    let bg3: LuaValue = style.get("background3")?;
                    let selection: LuaValue = style.get("selection")?;
                    let style_text: LuaValue = style.get("text")?;
                    let style_dim: LuaValue = style.get("dim")?;

                    draw_rect.call::<()>((
                        bx - border_width,
                        by - border_width,
                        bw + border_width * 2.0,
                        bh + border_width * 2.0,
                        style_divider.clone(),
                    ))?;
                    draw_rect.call::<()>((bx, by, bw, bh, bg3))?;

                    let selected: i64 = this.get("selected")?;
                    let iter: LuaFunction = this.call_method("each_item", ())?;
                    loop {
                        let results: LuaMultiValue = iter.call(())?;
                        let mut vals = results.into_iter();
                        let i_val = match vals.next() {
                            Some(v) if !matches!(v, LuaValue::Nil) => v,
                            _ => break,
                        };
                        let item = match vals.next() {
                            Some(v) => v,
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

                        let is_divider = match &item {
                            LuaValue::Table(t) => *t == divider_tbl,
                            _ => false,
                        };
                        if is_divider {
                            draw_rect.call::<()>((
                                x,
                                y + divider_padding * scale,
                                w,
                                divider_width,
                                style_divider.clone(),
                            ))?;
                        } else {
                            let i = match i_val {
                                LuaValue::Integer(n) => n,
                                LuaValue::Number(n) => n as i64,
                                _ => -1,
                            };
                            if i == selected {
                                draw_rect.call::<()>((x, y, w, h, selection.clone()))?;
                            }
                            if let LuaValue::Table(ref item_tbl) = item {
                                let text: String = item_tbl.get("text")?;
                                let font: LuaValue = style.get("font")?;
                                common.call_function::<LuaValue>(
                                    "draw_text",
                                    (
                                        font.clone(),
                                        style_text.clone(),
                                        text,
                                        "left",
                                        x + px,
                                        y,
                                        w,
                                        h,
                                    ),
                                )?;
                                let info: LuaValue = item_tbl.get("info")?;
                                if let LuaValue::String(_) = info {
                                    common.call_function::<LuaValue>(
                                        "draw_text",
                                        (font, style_dim.clone(), info, "right", x, y, w - px, h),
                                    )?;
                                }
                            }
                        }
                    }
                    Ok(())
                })?
            })?;

            Ok(LuaValue::Table(context_menu))
        })?,
    )
}
