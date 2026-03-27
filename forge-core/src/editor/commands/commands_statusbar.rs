use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Builds the suggest function for status-bar item picker commands.
fn status_view_get_items(lua: &Lua) -> LuaResult<LuaFunction> {
    lua.create_function(|lua, text: String| {
        let core: LuaTable = require_table(lua, "core")?;
        let command: LuaTable = require_table(lua, "core.command")?;
        let status_view_mod: LuaTable = require_table(lua, "core.statusview")?;
        let native_picker: LuaTable = require_table(lua, "picker")?;
        let status_view: LuaTable = core.get("status_view")?;

        let left_val: LuaValue = status_view_mod.get::<LuaTable>("Item")?.get("LEFT")?;

        // Get item names
        let items: LuaTable = status_view.call_method("get_items_list", ())?;
        let names = lua.create_table()?;
        for i in 1..=items.raw_len() {
            let item: LuaTable = items.get(i)?;
            let name: String = item.get("name")?;
            names.push(name)?;
        }

        // Rank names by text
        let ranked: LuaTable = native_picker.call_function("rank_strings", (names, text))?;

        // Build result with text/info/name
        let data = lua.create_table()?;
        for i in 1..=ranked.raw_len() {
            let name: String = ranked.get(i)?;
            let item: LuaTable = status_view.call_method("get_item", name.clone())?;
            let alignment: LuaValue = item.get("alignment")?;
            let info = if alignment == left_val {
                "Left"
            } else {
                "Right"
            };
            let prettified: String = command.call_function("prettify_name", name.clone())?;

            let entry = lua.create_table()?;
            entry.set("text", prettified)?;
            entry.set("info", info)?;
            entry.set("name", name)?;
            data.push(entry)?;
        }

        Ok(data)
    })
}

/// Registers status-bar commands.
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    let cmds = lua.create_table()?;

    cmds.set(
        "status-bar:toggle",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("toggle", ())
        })?,
    )?;

    cmds.set(
        "status-bar:show",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show", ())
        })?,
    )?;

    cmds.set(
        "status-bar:hide",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("hide", ())
        })?,
    )?;

    cmds.set(
        "status-bar:disable-messages",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("display_messages", false)
        })?,
    )?;

    cmds.set(
        "status-bar:enable-messages",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("display_messages", true)
        })?,
    )?;

    cmds.set(
        "status-bar:hide-item",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function(|lua, (_text, item): (String, LuaTable)| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    let name: String = item.get("name")?;
                    sv.call_method::<()>("hide_items", name)
                })?,
            )?;
            opts.set("suggest", status_view_get_items(lua)?)?;
            command_view.call_method::<()>("enter", ("Status bar item to hide", opts))
        })?,
    )?;

    cmds.set(
        "status-bar:show-item",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let command_view: LuaTable = core.get("command_view")?;
            let opts = lua.create_table()?;
            opts.set(
                "submit",
                lua.create_function(|lua, (_text, item): (String, LuaTable)| {
                    let core: LuaTable = require_table(lua, "core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    let name: String = item.get("name")?;
                    sv.call_method::<()>("show_items", name)
                })?,
            )?;
            opts.set("suggest", status_view_get_items(lua)?)?;
            command_view.call_method::<()>("enter", ("Status bar item to show", opts))
        })?,
    )?;

    cmds.set(
        "status-bar:reset-items",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show_items", ())
        })?,
    )?;

    add_fn.call::<()>((LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers the `core.commands.statusbar` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.statusbar",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
