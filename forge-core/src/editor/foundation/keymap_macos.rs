use mlua::prelude::*;

/// Registers `core.keymap-macos` — macOS-specific keybinding overrides.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.keymap-macos",
        lua.create_function(|lua, ()| {
            let f = lua.create_function(|lua, keymap: LuaTable| {
                let add_direct: LuaFunction = keymap.get("add_direct")?;
                let defaults = build_macos_bindings(lua)?;
                add_direct.call::<()>(defaults)?;
                Ok(())
            })?;
            Ok(LuaValue::Function(f))
        })?,
    )
}

/// Builds the macOS-specific keybinding table.
fn build_macos_bindings(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;

    // Dialog keys with inline functions that require core.command lazily
    // y -> { "dialog:select-yes", function }
    {
        let y_cmds = lua.create_table()?;
        y_cmds.set(1, "dialog:select-yes")?;
        y_cmds.set(
            2,
            lua.create_function(|lua, ()| -> LuaResult<LuaValue> {
                let require: LuaFunction = lua.globals().get("require")?;
                let cmd: LuaTable = require.call("core.command")?;
                let perform: LuaFunction = cmd.get("perform")?;
                perform.call(("dialog:select-initial", "y"))
            })?,
        )?;
        t.set("y", y_cmds)?;
    }

    // n -> { "dialog:select-no", function }
    {
        let n_cmds = lua.create_table()?;
        n_cmds.set(1, "dialog:select-no")?;
        n_cmds.set(
            2,
            lua.create_function(|lua, ()| -> LuaResult<LuaValue> {
                let require: LuaFunction = lua.globals().get("require")?;
                let cmd: LuaTable = require.call("core.command")?;
                let perform: LuaFunction = cmd.get("perform")?;
                perform.call(("dialog:select-initial", "n"))
            })?,
        )?;
        t.set("n", n_cmds)?;
    }

    // t -> function
    t.set(
        "t",
        lua.create_function(|lua, ()| -> LuaResult<LuaValue> {
            let require: LuaFunction = lua.globals().get("require")?;
            let cmd: LuaTable = require.call("core.command")?;
            let perform: LuaFunction = cmd.get("perform")?;
            perform.call(("dialog:select-initial", "t"))
        })?,
    )?;

    // c -> function
    t.set(
        "c",
        lua.create_function(|lua, ()| -> LuaResult<LuaValue> {
            let require: LuaFunction = lua.globals().get("require")?;
            let cmd: LuaTable = require.call("core.command")?;
            let perform: LuaFunction = cmd.get("perform")?;
            perform.call(("dialog:select-initial", "c"))
        })?,
    )?;

    let bindings: &[(&str, &[&str])] = &[
        ("cmd+p", &["core:find-command"]),
        ("cmd+o", &["core:open-file"]),
        ("cmd+shift+r", &["core:open-recent-file"]),
        ("cmd+option+shift+r", &["core:open-recent-folder"]),
        ("cmd+n", &["core:new-doc"]),
        ("cmd+shift+n", &["core:new-window"]),
        ("cmd+shift+c", &["core:change-project-folder"]),
        ("cmd+option+o", &["core:open-project-folder"]),
        ("cmd+option+w", &["core:close-project-folder"]),
        ("cmd+option+r", &["core:restart"]),
        ("cmd+ctrl+return", &["core:toggle-fullscreen"]),
        ("cmd+ctrl+shift+j", &["root:split-left"]),
        ("cmd+ctrl+shift+l", &["root:split-right"]),
        ("cmd+ctrl+shift+i", &["root:split-up"]),
        ("cmd+ctrl+shift+k", &["root:split-down"]),
        ("cmd+ctrl+j", &["root:switch-to-left"]),
        ("cmd+ctrl+l", &["root:switch-to-right"]),
        ("cmd+ctrl+i", &["root:switch-to-up"]),
        ("cmd+ctrl+k", &["root:switch-to-down"]),
        ("cmd+w", &["root:close-or-quit"]),
        ("ctrl+tab", &["root:switch-to-next-tab"]),
        ("ctrl+shift+tab", &["root:switch-to-previous-tab"]),
        ("option+tab", &["root:switch-to-next-tab"]),
        ("option+shift+tab", &["root:switch-to-previous-tab"]),
        ("cmd+shift+f", &["root:toggle-focus-mode"]),
        ("cmd+pageup", &["root:move-tab-left"]),
        ("cmd+pagedown", &["root:move-tab-right"]),
        ("cmd+1", &["root:switch-to-tab-1"]),
        ("cmd+2", &["root:switch-to-tab-2"]),
        ("cmd+3", &["root:switch-to-tab-3"]),
        ("cmd+4", &["root:switch-to-tab-4"]),
        ("cmd+5", &["root:switch-to-tab-5"]),
        ("cmd+6", &["root:switch-to-tab-6"]),
        ("cmd+7", &["root:switch-to-tab-7"]),
        ("cmd+8", &["root:switch-to-tab-8"]),
        ("cmd+9", &["root:switch-to-tab-9"]),
        ("wheel", &["root:scroll"]),
        ("hwheel", &["root:horizontal-scroll"]),
        ("shift+hwheel", &["root:horizontal-scroll"]),
        ("wheelup", &["root:scroll-hovered-tabs-backward"]),
        ("wheeldown", &["root:scroll-hovered-tabs-forward"]),
        ("cmd+f", &["find-replace:find"]),
        ("cmd+h", &["find-replace:replace"]),
        ("option+w", &["find-replace:toggle-whole-word"]),
        ("cmd+r", &["find-replace:replace"]),
        ("f3", &["find-replace:repeat-find"]),
        ("shift+f3", &["find-replace:previous-find"]),
        ("cmd+g", &["doc:go-to-line"]),
        ("cmd+s", &["doc:save"]),
        ("cmd+shift+s", &["doc:save-as"]),
        ("cmd+z", &["doc:undo"]),
        ("cmd+shift+z", &["doc:redo"]),
        ("cmd+y", &["doc:redo"]),
        ("cmd+x", &["doc:cut"]),
        ("cmd+c", &["doc:copy"]),
        ("cmd+v", &["doc:paste"]),
        ("ctrl+insert", &["doc:copy"]),
        ("shift+insert", &["doc:paste"]),
        (
            "escape",
            &[
                "root:exit-focus-mode",
                "command:escape",
                "doc:select-none",
                "dialog:select-no",
            ],
        ),
        ("tab", &["command:complete", "doc:indent"]),
        ("shift+tab", &["doc:unindent"]),
        ("backspace", &["doc:backspace"]),
        ("shift+backspace", &["doc:backspace"]),
        ("option+backspace", &["doc:delete-to-previous-word-start"]),
        (
            "cmd+shift+backspace",
            &["doc:delete-to-previous-word-start"],
        ),
        ("cmd+backspace", &["doc:delete-to-start-of-indentation"]),
        ("delete", &["doc:delete"]),
        ("shift+delete", &["doc:delete"]),
        ("option+delete", &["doc:delete-to-next-word-end"]),
        ("cmd+shift+delete", &["doc:delete-to-next-word-end"]),
        ("cmd+delete", &["doc:delete-to-end-of-line"]),
        (
            "return",
            &["command:submit", "doc:newline", "dialog:select"],
        ),
        (
            "keypad enter",
            &["command:submit", "doc:newline", "dialog:select"],
        ),
        ("cmd+return", &["doc:newline-below"]),
        ("cmd+shift+return", &["doc:newline-above"]),
        ("cmd+j", &["doc:join-lines"]),
        ("cmd+a", &["doc:select-all"]),
        (
            "cmd+d",
            &["find-replace:select-add-next", "doc:select-word"],
        ),
        ("cmd+option+l", &["find-replace:select-all-found"]),
        ("cmd+f3", &["find-replace:select-next"]),
        ("cmd+l", &["doc:select-lines"]),
        (
            "cmd+shift+l",
            &["find-replace:select-add-all", "doc:select-word"],
        ),
        ("cmd+/", &["doc:toggle-line-comments"]),
        ("option+up", &["doc:move-lines-up"]),
        ("option+down", &["doc:move-lines-down"]),
        ("cmd+shift+d", &["doc:duplicate-lines"]),
        ("cmd+shift+k", &["doc:delete-lines"]),
        ("cmd+option+[", &["fold:toggle"]),
        (
            "left",
            &["doc:move-to-previous-char", "dialog:previous-entry"],
        ),
        ("right", &["doc:move-to-next-char", "dialog:next-entry"]),
        (
            "up",
            &["command:select-previous", "doc:move-to-previous-line"],
        ),
        ("down", &["command:select-next", "doc:move-to-next-line"]),
        ("option+left", &["doc:move-to-previous-word-start"]),
        ("option+right", &["doc:move-to-next-word-end"]),
        ("cmd+left", &["doc:move-to-start-of-indentation"]),
        ("cmd+right", &["doc:move-to-end-of-line"]),
        ("cmd+[", &["doc:move-to-previous-block-start"]),
        ("cmd+]", &["doc:move-to-next-block-end"]),
        ("home", &["doc:move-to-start-of-indentation"]),
        ("end", &["doc:move-to-end-of-line"]),
        ("cmd+up", &["doc:move-to-start-of-doc"]),
        ("cmd+down", &["doc:move-to-end-of-doc"]),
        ("pageup", &["doc:move-to-previous-page"]),
        ("pagedown", &["doc:move-to-next-page"]),
        ("shift+1lclick", &["doc:select-to-cursor"]),
        ("ctrl+1lclick", &["doc:split-cursor"]),
        ("1lclick", &["doc:set-cursor"]),
        ("2lclick", &["doc:set-cursor-word"]),
        ("3lclick", &["doc:set-cursor-line"]),
        ("shift+left", &["doc:select-to-previous-char"]),
        ("shift+right", &["doc:select-to-next-char"]),
        ("shift+up", &["doc:select-to-previous-line"]),
        ("shift+down", &["doc:select-to-next-line"]),
        ("option+shift+left", &["doc:select-to-previous-word-start"]),
        ("option+shift+right", &["doc:select-to-next-word-end"]),
        ("cmd+shift+left", &["doc:select-to-start-of-indentation"]),
        ("cmd+shift+right", &["doc:select-to-end-of-line"]),
        ("cmd+shift+[", &["doc:select-to-previous-block-start"]),
        ("cmd+shift+]", &["doc:select-to-next-block-end"]),
        ("shift+home", &["doc:select-to-start-of-indentation"]),
        ("shift+end", &["doc:select-to-end-of-line"]),
        ("cmd+shift+up", &["doc:select-to-start-of-doc"]),
        ("cmd+shift+down", &["doc:select-to-end-of-doc"]),
        ("shift+pageup", &["doc:select-to-previous-page"]),
        ("shift+pagedown", &["doc:select-to-next-page"]),
        ("cmd+option+up", &["doc:create-cursor-previous-line"]),
        ("cmd+option+down", &["doc:create-cursor-next-line"]),
    ];

    for (key, cmds) in bindings {
        if cmds.len() == 1 {
            t.set(*key, cmds[0])?;
        } else {
            let arr = lua.create_table()?;
            for (i, cmd) in cmds.iter().enumerate() {
                arr.set(i + 1, *cmd)?;
            }
            t.set(*key, arr)?;
        }
    }

    Ok(t)
}
