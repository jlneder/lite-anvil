use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Recursively collects non-EmptyView views from unlocked leaf nodes.
fn collect_views(node: &LuaTable, empty_view: &LuaTable, out: &LuaTable) -> LuaResult<()> {
    let node_type: String = node.get("type")?;
    if node_type == "leaf" {
        let locked: bool = node.get::<bool>("locked").unwrap_or(false);
        if !locked {
            let views: LuaTable = node.get("views")?;
            for i in 1..=views.raw_len() {
                let view: LuaTable = views.get(i)?;
                let is_empty: bool = view.call_method("is", empty_view.clone())?;
                if !is_empty {
                    out.push(view)?;
                }
            }
        }
    } else {
        let a: LuaTable = node.get("a")?;
        let b: LuaTable = node.get("b")?;
        collect_views(&a, empty_view, out)?;
        collect_views(&b, empty_view, out)?;
    }
    Ok(())
}

/// Registers root-node commands (close, split, tab switching, scroll, layout).
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    // Build the main command table with node-predicate commands
    let t = lua.create_table()?;

    t.set(
        "root:close",
        lua.create_function(|lua, node: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            let root_node: LuaTable = root_view.get("root_node")?;
            node.call_method::<()>("close_active_view", root_node)
        })?,
    )?;

    t.set(
        "root:close-or-quit",
        lua.create_function(|lua, node: LuaTable| {
            let is_empty: bool = node.call_method("is_empty", ())?;
            let is_primary: bool = node.get("is_primary_node").unwrap_or(false);
            if !is_empty || !is_primary {
                let core: LuaTable = require_table(lua, "core")?;
                let root_view: LuaTable = core.get("root_view")?;
                let root_node: LuaTable = root_view.get("root_node")?;
                node.call_method::<()>("close_active_view", root_node)
            } else {
                let core: LuaTable = require_table(lua, "core")?;
                core.call_function::<()>("quit", ())
            }
        })?,
    )?;

    t.set(
        "root:close-all",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            let close_fn: LuaFunction = root_view.get("close_all_docviews")?;
            let docs: LuaTable = core.get("docs")?;
            core.call_function::<()>("confirm_close_docs", (docs, close_fn, root_view))
        })?,
    )?;

    t.set(
        "root:close-all-others",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let active_view: LuaValue = core.get("active_view")?;
            let active_doc: LuaValue = if let LuaValue::Table(ref v) = active_view {
                v.get("doc").unwrap_or(LuaValue::Nil)
            } else {
                LuaValue::Nil
            };
            let all_docs: LuaTable = core.get("docs")?;
            let docs = lua.create_table()?;
            for i in 1..=all_docs.raw_len() {
                let doc: LuaValue = all_docs.get(i)?;
                if doc != active_doc {
                    docs.push(doc)?;
                }
            }
            let root_view: LuaTable = core.get("root_view")?;
            let close_fn: LuaFunction = root_view.get("close_all_docviews")?;
            core.call_function::<()>("confirm_close_docs", (docs, close_fn, root_view, true))
        })?,
    )?;

    t.set(
        "root:move-tab-left",
        lua.create_function(|lua, node: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let active_view: LuaTable = core.get("active_view")?;
            let idx: i64 = node.call_method("get_view_idx", active_view.clone())?;
            if idx > 1 {
                let views: LuaTable = node.get("views")?;
                let table_mod: LuaTable = lua.globals().get("table")?;
                let remove: LuaFunction = table_mod.get("remove")?;
                remove.call::<()>((views.clone(), idx))?;
                let insert: LuaFunction = table_mod.get("insert")?;
                insert.call::<()>((views, idx - 1, active_view))?;
            }
            Ok(())
        })?,
    )?;

    t.set(
        "root:move-tab-right",
        lua.create_function(|lua, node: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let active_view: LuaTable = core.get("active_view")?;
            let views: LuaTable = node.get("views")?;
            let idx: i64 = node.call_method("get_view_idx", active_view.clone())?;
            let len = views.raw_len() as i64;
            if idx < len {
                let table_mod: LuaTable = lua.globals().get("table")?;
                let remove: LuaFunction = table_mod.get("remove")?;
                remove.call::<()>((views.clone(), idx))?;
                let insert: LuaFunction = table_mod.get("insert")?;
                insert.call::<()>((views, idx + 1, active_view))?;
            }
            Ok(())
        })?,
    )?;

    t.set(
        "root:shrink",
        lua.create_function(|lua, node: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let common: LuaTable = require_table(lua, "core.common")?;
            let root_view: LuaTable = core.get("root_view")?;
            let root_node: LuaTable = root_view.get("root_node")?;
            let parent: LuaTable = node.call_method("get_parent_node", root_node)?;
            let a: LuaTable = parent.get("a")?;
            let n: f64 = if a.equals(&node)? { -0.1 } else { 0.1 };
            let divider: f64 = parent.get("divider")?;
            let clamped: f64 = common.call_function("clamp", (divider + n, 0.1, 0.9))?;
            parent.set("divider", clamped)?;
            Ok(())
        })?,
    )?;

    t.set(
        "root:grow",
        lua.create_function(|lua, node: LuaTable| {
            let core: LuaTable = require_table(lua, "core")?;
            let common: LuaTable = require_table(lua, "core.common")?;
            let root_view: LuaTable = core.get("root_view")?;
            let root_node: LuaTable = root_view.get("root_node")?;
            let parent: LuaTable = node.call_method("get_parent_node", root_node)?;
            let a: LuaTable = parent.get("a")?;
            let n: f64 = if a.equals(&node)? { 0.1 } else { -0.1 };
            let divider: f64 = parent.get("divider")?;
            let clamped: f64 = common.call_function("clamp", (divider + n, 0.1, 0.9))?;
            parent.set("divider", clamped)?;
            Ok(())
        })?,
    )?;

    // root:switch-to-tab-1..9
    for i in 1..=9 {
        t.set(
            format!("root:switch-to-tab-{i}"),
            lua.create_function(move |_lua, node: LuaTable| {
                let views: LuaTable = node.get("views")?;
                let view: LuaValue = views.get(i)?;
                if let LuaValue::Table(v) = view {
                    node.call_method::<()>("set_active_view", v)?;
                }
                Ok(())
            })?,
        )?;
    }

    // root:split-{dir} and root:switch-to-{dir}
    for dir in &["left", "right", "up", "down"] {
        t.set(
            format!("root:split-{dir}"),
            lua.create_function({
                let dir = dir.to_string();
                move |lua, node: LuaTable| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let doc_view: LuaTable = require_table(lua, "core.docview")?;
                    let root_view: LuaTable = core.get("root_view")?;

                    let is_focus: bool = root_view.call_method("is_focus_mode_active", ())?;
                    let node = if is_focus {
                        root_view.call_method::<()>("exit_focus_mode", ())?;
                        root_view.call_method("get_active_node", ())?
                    } else {
                        node
                    };

                    let av: LuaTable = node.get("active_view")?;
                    node.call_method::<()>("split", dir.as_str())?;
                    let is_dv: bool = av.call_method("is", doc_view)?;
                    if is_dv {
                        let doc: LuaTable = av.get("doc")?;
                        root_view.call_method::<()>("open_doc", doc)?;
                    }
                    Ok(())
                }
            })?,
        )?;

        t.set(
            format!("root:switch-to-{dir}"),
            lua.create_function({
                let dir = dir.to_string();
                move |lua, node: LuaTable| {
                    let style: LuaTable = require_table(lua, "core.style")?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let root_view: LuaTable = core.get("root_view")?;
                    let root_node: LuaTable = root_view.get("root_node")?;

                    let position: LuaTable = node.get("position")?;
                    let size: LuaTable = node.get("size")?;
                    let pos_x: f64 = position.get("x")?;
                    let pos_y: f64 = position.get("y")?;
                    let sz_x: f64 = size.get("x")?;
                    let sz_y: f64 = size.get("y")?;
                    let divider_size: f64 = style.get("divider_size")?;

                    let (x, y) = match dir.as_str() {
                        "left" => (pos_x - 1.0, pos_y + sz_y / 2.0),
                        "right" => (pos_x + sz_x + divider_size, pos_y + sz_y / 2.0),
                        "up" => (pos_x + sz_x / 2.0, pos_y - 1.0),
                        "down" => (pos_x + sz_x / 2.0, pos_y + sz_y + divider_size),
                        _ => return Ok(()),
                    };

                    let target: LuaTable =
                        root_node.call_method("get_child_overlapping_point", (x, y))?;
                    let locked: LuaMultiValue = target.call_method("get_locked_size", ())?;
                    let vals: Vec<LuaValue> = locked.into_iter().collect();
                    let sx_nil = vals
                        .first()
                        .map(|v| matches!(v, LuaValue::Nil))
                        .unwrap_or(true);
                    let sy_nil = vals
                        .get(1)
                        .map(|v| matches!(v, LuaValue::Nil))
                        .unwrap_or(true);
                    if sx_nil && sy_nil {
                        let av: LuaTable = target.get("active_view")?;
                        core.call_function::<()>("set_active_view", av)?;
                    }
                    Ok(())
                }
            })?,
        )?;
    }

    // Main predicate: node with no locked size
    let main_predicate = lua.create_function(|lua, ()| {
        let core: LuaTable = require_table(lua, "core")?;
        let root_view: LuaTable = core.get("root_view")?;
        let node: LuaTable = root_view.call_method("get_active_node", ())?;
        let locked: LuaMultiValue = node.call_method("get_locked_size", ())?;
        let vals: Vec<LuaValue> = locked.into_iter().collect();
        let sx_nil = vals
            .first()
            .map(|v| matches!(v, LuaValue::Nil))
            .unwrap_or(true);
        let sy_nil = vals
            .get(1)
            .map(|v| matches!(v, LuaValue::Nil))
            .unwrap_or(true);
        if sx_nil && sy_nil {
            Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Boolean(true),
                LuaValue::Table(node),
            ]))
        } else {
            Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]))
        }
    })?;
    add_fn.call::<()>((main_predicate, t))?;

    // Global commands (nil predicate)
    let global_cmds = lua.create_table()?;

    global_cmds.set(
        "root:scroll",
        lua.create_function(|lua, delta: f64| {
            let core: LuaTable = require_table(lua, "core")?;
            let config: LuaTable = require_table(lua, "core.config")?;
            let root_view: LuaTable = core.get("root_view")?;
            let view: LuaValue = root_view.get("overlapping_view")?;
            let view = match view {
                LuaValue::Table(v) => v,
                _ => core.get("active_view")?,
            };
            let scrollable: bool = view.get("scrollable").unwrap_or(false);
            if scrollable {
                let scroll: LuaTable = view.get("scroll")?;
                let to: LuaTable = scroll.get("to")?;
                let y: f64 = to.get("y")?;
                let wheel_scroll: f64 = config.get("mouse_wheel_scroll")?;
                to.set("y", y + delta * -wheel_scroll)?;
                Ok(LuaValue::Boolean(true))
            } else {
                Ok(LuaValue::Boolean(false))
            }
        })?,
    )?;

    global_cmds.set(
        "root:horizontal-scroll",
        lua.create_function(|lua, delta: f64| {
            let core: LuaTable = require_table(lua, "core")?;
            let config: LuaTable = require_table(lua, "core.config")?;
            let root_view: LuaTable = core.get("root_view")?;
            let view: LuaValue = root_view.get("overlapping_view")?;
            let view = match view {
                LuaValue::Table(v) => v,
                _ => core.get("active_view")?,
            };
            let scrollable: bool = view.get("scrollable").unwrap_or(false);
            if scrollable {
                let scroll: LuaTable = view.get("scroll")?;
                let to: LuaTable = scroll.get("to")?;
                let x: f64 = to.get("x")?;
                let wheel_scroll: f64 = config.get("mouse_wheel_scroll")?;
                to.set("x", x + delta * -wheel_scroll)?;
                Ok(LuaValue::Boolean(true))
            } else {
                Ok(LuaValue::Boolean(false))
            }
        })?,
    )?;

    global_cmds.set(
        "root:reset-layout",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let empty_view: LuaTable = require_table(lua, "core.emptyview")?;
            let root_view: LuaTable = core.get("root_view")?;

            let is_focus: bool = root_view.call_method("is_focus_mode_active", ())?;
            if is_focus {
                root_view.call_method::<()>("exit_focus_mode", ())?;
            }

            let prev_active: LuaValue = core.get("active_view")?;
            let root_node: LuaTable = root_view.get("root_node")?;

            // Collect all session views from non-locked leaf nodes
            let collected = lua.create_table()?;
            collect_views(&root_node, &empty_view, &collected)?;

            root_view.call_method::<()>("close_all_docviews", ())?;

            let primary: LuaTable = root_view.call_method("get_active_node_default", ())?;
            for i in 1..=collected.raw_len() {
                let view: LuaValue = collected.get(i)?;
                primary.call_method::<()>("add_view", view)?;
            }

            if let LuaValue::Table(ref pv) = prev_active {
                let ctx: Option<String> = pv.get("context")?;
                if ctx.as_deref() == Some("session") {
                    core.call_function::<()>("set_active_view", prev_active)?;
                }
            }
            Ok(())
        })?,
    )?;

    global_cmds.set(
        "root:enter-focus-mode",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            root_view.call_method::<LuaValue>("enter_focus_mode", ())
        })?,
    )?;

    global_cmds.set(
        "root:toggle-focus-mode",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            root_view.call_method::<LuaValue>("toggle_focus_mode", ())
        })?,
    )?;

    add_fn.call::<()>((LuaValue::Nil, global_cmds))?;

    // Focus mode exit predicate
    let focus_predicate = lua.create_function(|lua, ()| {
        let core: LuaTable = require_table(lua, "core")?;
        let root_view: LuaTable = core.get("root_view")?;
        let active: bool = root_view.call_method("is_focus_mode_active", ())?;
        Ok(active)
    })?;

    let focus_cmds = lua.create_table()?;
    focus_cmds.set(
        "root:exit-focus-mode",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            root_view.call_method::<LuaValue>("exit_focus_mode", ())
        })?,
    )?;
    add_fn.call::<()>((focus_predicate, focus_cmds))?;

    // Node-or-active predicate (for tab switching)
    let node_predicate = lua.create_function(|lua, node: LuaValue| {
        let node_class: LuaTable = require_table(lua, "core.node")?;
        let core: LuaTable = require_table(lua, "core")?;

        let actual_node = if let LuaValue::Table(ref n) = node {
            let is_node: bool = node_class.call_method("is_extended_by", n.clone())?;
            if is_node {
                n.clone()
            } else {
                let root_view: LuaTable = core.get("root_view")?;
                root_view.call_method("get_active_node", ())?
            }
        } else {
            let root_view: LuaTable = core.get("root_view")?;
            root_view.call_method("get_active_node", ())?
        };

        Ok(LuaMultiValue::from_vec(vec![
            LuaValue::Boolean(true),
            LuaValue::Table(actual_node),
        ]))
    })?;

    let tab_cmds = lua.create_table()?;
    tab_cmds.set(
        "root:switch-to-previous-tab",
        lua.create_function(|_lua, node: LuaTable| {
            let views: LuaTable = node.get("views")?;
            let active_view: LuaTable = node.get("active_view")?;
            let mut idx: i64 = node.call_method("get_view_idx", active_view)?;
            idx -= 1;
            if idx < 1 {
                idx = views.raw_len() as i64;
            }
            let view: LuaTable = views.get(idx)?;
            node.call_method::<()>("set_active_view", view)
        })?,
    )?;

    tab_cmds.set(
        "root:switch-to-next-tab",
        lua.create_function(|_lua, node: LuaTable| {
            let views: LuaTable = node.get("views")?;
            let active_view: LuaTable = node.get("active_view")?;
            let mut idx: i64 = node.call_method("get_view_idx", active_view)?;
            idx += 1;
            if idx > views.raw_len() as i64 {
                idx = 1;
            }
            let view: LuaTable = views.get(idx)?;
            node.call_method::<()>("set_active_view", view)
        })?,
    )?;

    tab_cmds.set(
        "root:scroll-tabs-backward",
        lua.create_function(|_lua, node: LuaTable| node.call_method::<()>("scroll_tabs", 1))?,
    )?;

    tab_cmds.set(
        "root:scroll-tabs-forward",
        lua.create_function(|_lua, node: LuaTable| node.call_method::<()>("scroll_tabs", 2))?,
    )?;

    add_fn.call::<()>((node_predicate, tab_cmds))?;

    // Hovered tab predicate
    let hovered_predicate = lua.create_function(|lua, ()| {
        let core: LuaTable = require_table(lua, "core")?;
        let root_view: LuaTable = core.get("root_view")?;
        let root_node: LuaTable = root_view.get("root_node")?;
        let mouse: LuaTable = root_view.get("mouse")?;
        let mx: f64 = mouse.get("x")?;
        let my: f64 = mouse.get("y")?;
        let node: LuaTable = root_node.call_method("get_child_overlapping_point", (mx, my))?;

        let hovered_tab: LuaValue = node.get("hovered_tab")?;
        let hovered_scroll: i64 = node.get::<i64>("hovered_scroll_button").unwrap_or(0);
        let has_hover =
            !matches!(hovered_tab, LuaValue::Nil | LuaValue::Boolean(false)) || hovered_scroll > 0;

        if has_hover {
            Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Boolean(true),
                LuaValue::Table(node),
            ]))
        } else {
            Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]))
        }
    })?;

    let hovered_cmds = lua.create_table()?;
    hovered_cmds.set(
        "root:switch-to-hovered-previous-tab",
        lua.create_function(|lua, node: LuaTable| {
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", ("root:switch-to-previous-tab", node))
        })?,
    )?;
    hovered_cmds.set(
        "root:switch-to-hovered-next-tab",
        lua.create_function(|lua, node: LuaTable| {
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", ("root:switch-to-next-tab", node))
        })?,
    )?;
    hovered_cmds.set(
        "root:scroll-hovered-tabs-backward",
        lua.create_function(|lua, node: LuaTable| {
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", ("root:scroll-tabs-backward", node))
        })?,
    )?;
    hovered_cmds.set(
        "root:scroll-hovered-tabs-forward",
        lua.create_function(|lua, node: LuaTable| {
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", ("root:scroll-tabs-forward", node))
        })?,
    )?;
    add_fn.call::<()>((hovered_predicate, hovered_cmds))?;

    // Double-click tab bar predicate
    let tabbar_predicate = lua.create_function(|lua, (x, y): (LuaValue, LuaValue)| {
        let core: LuaTable = require_table(lua, "core")?;
        let root_view: LuaTable = core.get("root_view")?;
        let root_node: LuaTable = root_view.get("root_node")?;
        let (xf, yf) = match (&x, &y) {
            (LuaValue::Number(xn), LuaValue::Number(yn)) => (*xn, *yn),
            (LuaValue::Integer(xi), LuaValue::Integer(yi)) => (*xi as f64, *yi as f64),
            _ => return Ok(LuaValue::Nil),
        };
        let node: LuaTable = root_node.call_method("get_child_overlapping_point", (xf, yf))?;
        let in_tab: bool = node.call_method("is_in_tab_area", (xf, yf))?;
        Ok(LuaValue::Boolean(in_tab))
    })?;

    let tabbar_cmds = lua.create_table()?;
    tabbar_cmds.set(
        "tabbar:new-doc",
        lua.create_function(|lua, ()| {
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", "core:new-doc")
        })?,
    )?;
    add_fn.call::<()>((tabbar_predicate, tabbar_cmds))?;

    // EmptyView predicate
    let empty_cmds = lua.create_table()?;
    empty_cmds.set(
        "emptyview:new-doc",
        lua.create_function(|lua, ()| {
            let command: LuaTable = require_table(lua, "core.command")?;
            command.call_function::<()>("perform", "core:new-doc")
        })?,
    )?;
    add_fn.call::<()>(("core.emptyview", empty_cmds))?;

    Ok(())
}

/// Registers the `core.commands.root` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.root",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
