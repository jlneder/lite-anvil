use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Replaces the selected lines in the document with transformed lines.
fn transform_selected_lines(
    lua: &Lua,
    transform: impl FnOnce(&mut Vec<String>),
) -> LuaResult<()> {
    let core = require_table(lua, "core")?;
    let view: LuaTable = core.get("active_view")?;
    let doc: LuaTable = view.get("doc")?;
    let sel: LuaMultiValue = doc.call_method("get_selection", ())?;
    let vals: Vec<LuaValue> = sel.into_vec();
    let line1 = match vals.first() {
        Some(LuaValue::Integer(n)) => *n as i64,
        Some(LuaValue::Number(n)) => *n as i64,
        _ => return Ok(()),
    };
    let line2 = match vals.get(2) {
        Some(LuaValue::Integer(n)) => *n as i64,
        Some(LuaValue::Number(n)) => *n as i64,
        _ => line1,
    };
    let col2 = match vals.get(3) {
        Some(LuaValue::Integer(n)) => *n as i64,
        Some(LuaValue::Number(n)) => *n as i64,
        _ => 1,
    };
    // If cursor is at col 1 of the last line, don't include that line.
    let end = if line2 > line1 && col2 <= 1 {
        line2 - 1
    } else {
        line2
    };
    let (start, end) = if line1 <= end {
        (line1, end)
    } else {
        (end, line1)
    };

    let lines: LuaTable = doc.get("lines")?;
    let mut text_lines: Vec<String> = Vec::new();
    for i in start..=end {
        let line: String = lines.get(i)?;
        text_lines.push(line.trim_end_matches('\n').to_owned());
    }

    transform(&mut text_lines);

    let replacement = text_lines.join("\n") + "\n";
    doc.call_method::<()>("remove", (start, 1, end + 1, 1))?;
    doc.call_method::<()>("insert", (start, 1, replacement))?;
    doc.call_method::<()>(
        "set_selection",
        (start, 1, start + text_lines.len() as i64, 1),
    )?;
    let _ = lua;
    Ok(())
}

fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command = require_table(lua, "core.command")?;
    let cmds = lua.create_table()?;

    cmds.set(
        "lines:sort",
        lua.create_function(|lua, ()| {
            transform_selected_lines(lua, |lines| lines.sort())
        })?,
    )?;

    cmds.set(
        "lines:sort-reverse",
        lua.create_function(|lua, ()| {
            transform_selected_lines(lua, |lines| {
                lines.sort();
                lines.reverse();
            })
        })?,
    )?;

    cmds.set(
        "lines:reverse",
        lua.create_function(|lua, ()| transform_selected_lines(lua, |lines| lines.reverse()))?,
    )?;

    cmds.set(
        "lines:unique",
        lua.create_function(|lua, ()| {
            transform_selected_lines(lua, |lines| {
                let mut seen = std::collections::HashSet::new();
                lines.retain(|line| seen.insert(line.clone()));
            })
        })?,
    )?;

    cmds.set(
        "lines:sort-case-insensitive",
        lua.create_function(|lua, ()| {
            transform_selected_lines(lua, |lines| {
                lines.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
            })
        })?,
    )?;

    command.call_function::<()>("add", (LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers `plugins.sortlines`: sort, reverse, unique, and case-insensitive sort commands.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.sortlines",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
