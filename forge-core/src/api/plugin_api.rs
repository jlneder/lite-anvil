use mlua::prelude::*;

/// Registers `core.plugin_api` — delegation table for plugin access to core APIs.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.plugin_api",
        lua.create_function(|lua, ()| {
            let core: LuaTable = lua
                .globals()
                .get::<LuaTable>("package")?
                .get::<LuaTable>("loaded")?
                .get("core")?;
            let api = lua.create_table()?;

            // plugin_api.session
            let session = lua.create_table()?;
            session.set(
                "on_load",
                lua.create_function(|lua, (name, hook): (LuaString, LuaFunction)| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    core.call_method::<LuaValue>("register_session_load_hook", (name, hook))
                })?,
            )?;
            session.set(
                "on_save",
                lua.create_function(|lua, (name, hook): (LuaString, LuaFunction)| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    core.call_method::<LuaValue>("register_session_save_hook", (name, hook))
                })?,
            )?;
            api.set("session", session)?;

            // plugin_api.threads
            let threads = lua.create_table()?;
            threads.set(
                "spawn",
                lua.create_function(
                    |lua, (weak_ref, func, args): (LuaValue, LuaFunction, LuaMultiValue)| {
                        let core: LuaTable = lua
                            .globals()
                            .get::<LuaTable>("package")?
                            .get::<LuaTable>("loaded")?
                            .get("core")?;
                        let add_thread: LuaFunction = core.get("add_thread")?;
                        let mut call_args = vec![LuaValue::Function(func), weak_ref];
                        call_args.extend(args);
                        add_thread.call::<LuaValue>(LuaMultiValue::from_vec(call_args))
                    },
                )?,
            )?;
            api.set("threads", threads)?;

            // plugin_api.views
            let views = lua.create_table()?;
            views.set(
                "active",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    core.get::<LuaValue>("active_view")
                })?,
            )?;
            views.set(
                "set_active",
                lua.create_function(|lua, view: LuaValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("set_active_view")?;
                    f.call::<LuaValue>(view)
                })?,
            )?;
            views.set(
                "open_doc",
                lua.create_function(|lua, path_or_doc: LuaValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_open_doc")?;
                    f.call::<LuaValue>(path_or_doc)
                })?,
            )?;
            views.set(
                "children",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_children")?;
                    f.call::<LuaValue>(())
                })?,
            )?;
            views.set(
                "get_node_for_view",
                lua.create_function(|lua, view: LuaValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_get_node_for_view")?;
                    f.call::<LuaValue>(view)
                })?,
            )?;
            views.set(
                "update_layout",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_update_layout")?;
                    f.call::<LuaValue>(())
                })?,
            )?;
            views.set(
                "root_size",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_root_size")?;
                    f.call::<LuaValue>(())
                })?,
            )?;
            views.set(
                "defer_draw",
                lua.create_function(|lua, args: LuaMultiValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let rv: LuaTable = core.get("root_view")?;
                    rv.call_method::<LuaValue>("defer_draw", args)
                })?,
            )?;
            views.set(
                "get_active_node_default",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let rv: LuaTable = core.get("root_view")?;
                    rv.call_method::<LuaValue>("get_active_node_default", ())
                })?,
            )?;
            views.set(
                "get_primary_node",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let rv: LuaTable = core.get("root_view")?;
                    rv.call_method::<LuaValue>("get_primary_node", ())
                })?,
            )?;
            views.set(
                "add_view",
                lua.create_function(|lua, (view, placement): (LuaValue, LuaValue)| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let rv: LuaTable = core.get("root_view")?;
                    rv.call_method::<LuaValue>("add_view", (view, placement))
                })?,
            )?;
            views.set(
                "close_all_docviews",
                lua.create_function(|lua, keep_active: LuaValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let rv: LuaTable = core.get("root_view")?;
                    rv.call_method::<LuaValue>("close_all_docviews", keep_active)
                })?,
            )?;
            api.set("views", views)?;

            // plugin_api.prompt
            let prompt = lua.create_table()?;
            prompt.set(
                "enter",
                lua.create_function(|lua, (label, options): (LuaValue, LuaValue)| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_enter_prompt")?;
                    f.call::<LuaValue>((label, options))
                })?,
            )?;
            prompt.set(
                "update_suggestions",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_update_prompt_suggestions")?;
                    f.call::<LuaValue>(())
                })?,
            )?;
            api.set("prompt", prompt)?;

            // plugin_api.status
            let status = lua.create_table()?;
            let constants = lua.create_table()?;
            constants.set(
                "RIGHT",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    let item: LuaTable = sv.get("Item")?;
                    item.get::<LuaValue>("RIGHT")
                })?,
            )?;
            constants.set(
                "separator2",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    sv.get::<LuaValue>("separator2")
                })?,
            )?;
            status.set("constants", constants)?;

            status.set(
                "add_item",
                lua.create_function(|lua, item: LuaValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_add_status_item")?;
                    f.call::<LuaValue>(item)
                })?,
            )?;
            status.set(
                "show_message",
                lua.create_function(|lua, (icon, color, text): (LuaValue, LuaValue, LuaValue)| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_show_status_message")?;
                    f.call::<LuaValue>((icon, color, text))
                })?,
            )?;
            status.set(
                "show_tooltip",
                lua.create_function(|lua, text: LuaValue| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_show_status_tooltip")?;
                    f.call::<LuaValue>(text)
                })?,
            )?;
            status.set(
                "remove_tooltip",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = lua
                        .globals()
                        .get::<LuaTable>("package")?
                        .get::<LuaTable>("loaded")?
                        .get("core")?;
                    let f: LuaFunction = core.get("plugin_remove_status_tooltip")?;
                    f.call::<LuaValue>(())
                })?,
            )?;
            api.set("status", status)?;

            let _ = core;
            Ok(LuaValue::Table(api))
        })?,
    )
}
