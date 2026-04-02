use mlua::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Per-document bookmarked lines, keyed by absolute filename.
static BOOKMARKS: Lazy<Mutex<HashMap<String, BTreeSet<i64>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Returns the absolute filename for the active document, if any.
fn doc_abs_filename(doc: &LuaTable) -> LuaResult<Option<String>> {
    match doc.get::<LuaValue>("abs_filename")? {
        LuaValue::String(s) => Ok(Some(s.to_str()?.to_owned())),
        _ => Ok(None),
    }
}

fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command = require_table(lua, "core.command")?;
    let cmds = lua.create_table()?;

    cmds.set(
        "bookmarks:toggle",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            let view: LuaTable = core.get("active_view")?;
            let doc: LuaTable = view.get("doc")?;
            let Some(filename) = doc_abs_filename(&doc)? else {
                return Ok(());
            };
            let sel: LuaMultiValue = doc.call_method("get_selection", ())?;
            let line = match sel.front() {
                Some(LuaValue::Integer(n)) => *n,
                Some(LuaValue::Number(n)) => *n as i64,
                _ => return Ok(()),
            };
            let mut map = BOOKMARKS.lock();
            let set = map.entry(filename).or_default();
            if !set.remove(&line) {
                set.insert(line);
            }
            Ok(())
        })?,
    )?;

    cmds.set(
        "bookmarks:next",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            let view: LuaTable = core.get("active_view")?;
            let doc: LuaTable = view.get("doc")?;
            let Some(filename) = doc_abs_filename(&doc)? else {
                return Ok(());
            };
            let sel: LuaMultiValue = doc.call_method("get_selection", ())?;
            let current = match sel.front() {
                Some(LuaValue::Integer(n)) => *n,
                Some(LuaValue::Number(n)) => *n as i64,
                _ => return Ok(()),
            };
            let map = BOOKMARKS.lock();
            if let Some(set) = map.get(&filename) {
                let target = set
                    .range((current + 1)..)
                    .next()
                    .or_else(|| set.iter().next());
                if let Some(&line) = target {
                    doc.call_method::<()>("set_selection", (line, 1))?;
                    view.call_method::<()>("scroll_to_line", (line, true, true))?;
                }
            }
            Ok(())
        })?,
    )?;

    cmds.set(
        "bookmarks:previous",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            let view: LuaTable = core.get("active_view")?;
            let doc: LuaTable = view.get("doc")?;
            let Some(filename) = doc_abs_filename(&doc)? else {
                return Ok(());
            };
            let sel: LuaMultiValue = doc.call_method("get_selection", ())?;
            let current = match sel.front() {
                Some(LuaValue::Integer(n)) => *n,
                Some(LuaValue::Number(n)) => *n as i64,
                _ => return Ok(()),
            };
            let map = BOOKMARKS.lock();
            if let Some(set) = map.get(&filename) {
                let target = set
                    .range(..current)
                    .next_back()
                    .or_else(|| set.iter().next_back());
                if let Some(&line) = target {
                    doc.call_method::<()>("set_selection", (line, 1))?;
                    view.call_method::<()>("scroll_to_line", (line, true, true))?;
                }
            }
            Ok(())
        })?,
    )?;

    cmds.set(
        "bookmarks:clear",
        lua.create_function(|lua, ()| {
            let core = require_table(lua, "core")?;
            let view: LuaTable = core.get("active_view")?;
            let doc: LuaTable = view.get("doc")?;
            if let Some(filename) = doc_abs_filename(&doc)? {
                BOOKMARKS.lock().remove(&filename);
            }
            Ok(())
        })?,
    )?;

    command.call_function::<()>("add", (LuaValue::Nil, cmds))?;
    Ok(())
}

fn patch_draw_line_gutter(lua: &Lua) -> LuaResult<()> {
    let docview = require_table(lua, "core.docview")?;
    let old: LuaFunction = docview.get("draw_line_gutter")?;
    let old_key = Arc::new(lua.create_registry_value(old)?);

    docview.set(
        "draw_line_gutter",
        lua.create_function(move |lua, (this, line, x, y, w): (LuaTable, i64, f64, f64, f64)| {
            let old: LuaFunction = lua.registry_value(&old_key)?;
            let result: LuaMultiValue = old.call((this.clone(), line, x, y, w))?;

            let doc: LuaTable = this.get("doc")?;
            if let Some(filename) = doc_abs_filename(&doc)? {
                let map = BOOKMARKS.lock();
                if let Some(set) = map.get(&filename) {
                    if set.contains(&line) {
                        let style = require_table(lua, "core.style")?;
                        let accent: LuaValue = style.get("accent")?;
                        let renderer: LuaTable = lua.globals().get("renderer")?;
                        // Draw a small diamond/marker in the gutter.
                        let font: LuaValue = this.call_method("get_font", ())?;
                        let fh: f64 = match &font {
                            LuaValue::Table(t) => t.call_method("get_height", ())?,
                            LuaValue::UserData(ud) => ud.call_method("get_height", ())?,
                            _ => 14.0,
                        };
                        let size = 6.0_f64;
                        let mx = x + 2.0;
                        let my = y + (fh - size) / 2.0;
                        renderer.call_function::<()>(
                            "draw_rect",
                            (mx, my, size, size, accent),
                        )?;
                    }
                }
            }
            Ok(result)
        })?,
    )?;
    Ok(())
}

fn register_keymap(lua: &Lua) -> LuaResult<()> {
    let keymap = require_table(lua, "core.keymap")?;
    let bindings = lua.create_table()?;
    bindings.set("ctrl+f2", "bookmarks:toggle")?;
    bindings.set("f2", "bookmarks:next")?;
    bindings.set("shift+f2", "bookmarks:previous")?;
    keymap.call_function::<()>("add", bindings)?;
    Ok(())
}

/// Registers `plugins.bookmarks`: toggle/navigate/clear commands with gutter markers.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.bookmarks",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            patch_draw_line_gutter(lua)?;
            register_keymap(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
