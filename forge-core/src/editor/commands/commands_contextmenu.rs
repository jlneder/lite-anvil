use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Registers context-menu commands (show, focus, hide, submit).
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;
    let context_menu_class: LuaTable = require_table(lua, "core.contextmenu")?;

    // First predicate: function(x, y) that checks for context menu items
    let show_predicate = lua.create_function(|lua, (x, y): (LuaValue, LuaValue)| {
        let core: LuaTable = require_table(lua, "core")?;
        let root_view: LuaTable = core.get("root_view")?;

        let (mx, my) = match (&x, &y) {
            (LuaValue::Number(xn), LuaValue::Number(yn)) => (*xn, *yn),
            (LuaValue::Integer(xi), LuaValue::Integer(yi)) => (*xi as f64, *yi as f64),
            _ => {
                let mouse: LuaTable = root_view.get("mouse")?;
                let mx: f64 = mouse.get("x")?;
                let my: f64 = mouse.get("y")?;
                (mx, my)
            }
        };

        let root_node: LuaTable = root_view.get("root_node")?;
        let child: LuaTable = root_node.call_method("get_child_overlapping_point", (mx, my))?;
        let view: LuaValue = child.get("active_view")?;
        let view = match view {
            LuaValue::Table(v) => v,
            _ => return Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil])),
        };

        let results: LuaMultiValue = view.call_method("on_context_menu", (mx, my))?;
        let results_vec: Vec<LuaValue> = results.into_iter().collect();
        if results_vec.is_empty() {
            return Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]));
        }

        // Check first result has .items with length > 0
        let first = &results_vec[0];
        let has_items = if let LuaValue::Table(t) = first {
            let items: LuaValue = t.get("items")?;
            if let LuaValue::Table(items_tbl) = &items {
                items_tbl.raw_len() > 0
            } else {
                false
            }
        } else {
            false
        };

        if !has_items {
            return Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]));
        }

        let mut ret = vec![LuaValue::Boolean(true), lua.pack(mx)?, lua.pack(my)?];
        for v in results_vec {
            ret.push(v);
        }
        Ok(LuaMultiValue::from_vec(ret))
    })?;

    let show_cmds = lua.create_table()?;
    show_cmds.set(
        "context-menu:show",
        lua.create_function(|lua, args: LuaMultiValue| {
            let mut iter = args.into_iter();
            let x_val = iter.next().unwrap_or(LuaValue::Nil);
            let y_val = iter.next().unwrap_or(LuaValue::Nil);
            let results_val = iter.next().unwrap_or(LuaValue::Nil);
            let rest: Vec<LuaValue> = iter.collect();

            let x: f64 = lua.unpack(x_val.clone())?;
            let y: f64 = lua.unpack(y_val.clone())?;

            let results = match results_val {
                LuaValue::Table(ref t) => t,
                _ => return Ok(()),
            };

            let show_x: f64 = results
                .get::<LuaValue>("x")
                .and_then(|v| lua.unpack(v))
                .unwrap_or(x);
            let show_y: f64 = results
                .get::<LuaValue>("y")
                .and_then(|v| lua.unpack(v))
                .unwrap_or(y);
            let items: LuaValue = results.get("items")?;

            let core: LuaTable = require_table(lua, "core")?;
            let root_view: LuaTable = core.get("root_view")?;
            let context_menu: LuaTable = root_view.get("context_menu")?;

            let mut call_args = vec![lua.pack(show_x)?, lua.pack(show_y)?, items];
            call_args.extend(rest);
            context_menu.call_method::<()>("show", LuaMultiValue::from_vec(call_args))
        })?,
    )?;
    add_fn.call::<()>((show_predicate, show_cmds))?;

    // Second group: ContextMenu class predicate (focus-previous, focus-next, hide)
    let ctx_cmds = lua.create_table()?;
    ctx_cmds.set(
        "context-menu:focus-previous",
        lua.create_function(|_lua, context_menu: LuaTable| {
            context_menu.call_method::<()>("focus_previous", ())
        })?,
    )?;
    ctx_cmds.set(
        "context-menu:focus-next",
        lua.create_function(|_lua, context_menu: LuaTable| {
            context_menu.call_method::<()>("focus_next", ())
        })?,
    )?;
    ctx_cmds.set(
        "context-menu:hide",
        lua.create_function(|_lua, context_menu: LuaTable| {
            context_menu.call_method::<()>("hide", ())
        })?,
    )?;
    add_fn.call::<()>((context_menu_class.clone(), ctx_cmds))?;

    // Third group: predicate that checks active_view:is(ContextMenu) and get_item_selected
    let submit_predicate = lua.create_function(move |lua, ()| {
        let core: LuaTable = require_table(lua, "core")?;
        let active_view: LuaTable = core.get("active_view")?;
        let context_menu_cls: LuaTable = require_table(lua, "core.contextmenu")?;
        let is_ctx: bool = active_view.call_method("is", context_menu_cls)?;
        if !is_ctx {
            return Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]));
        }
        let item: LuaValue = active_view.call_method("get_item_selected", ())?;
        match item {
            LuaValue::Nil | LuaValue::Boolean(false) => {
                Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil]))
            }
            _ => {
                let root_view: LuaTable = core.get("root_view")?;
                let ctx: LuaTable = root_view.get("context_menu")?;
                Ok(LuaMultiValue::from_vec(vec![
                    item.clone(),
                    LuaValue::Table(ctx),
                    item,
                ]))
            }
        }
    })?;

    let submit_cmds = lua.create_table()?;
    submit_cmds.set(
        "context-menu:submit",
        lua.create_function(|_lua, (context_menu, item): (LuaTable, LuaTable)| {
            let cmd: LuaValue = item.get("command")?;
            if !matches!(cmd, LuaValue::Nil) {
                context_menu.call_method::<()>("on_selected", item)?;
                context_menu.call_method::<()>("hide", ())?;
            }
            Ok(())
        })?,
    )?;
    add_fn.call::<()>((submit_predicate, submit_cmds))?;

    Ok(())
}

/// Registers the `core.commands.contextmenu` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.contextmenu",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
