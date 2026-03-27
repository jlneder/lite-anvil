use std::sync::Arc;

use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

/// Collects all selections from doc:get_selections(), properly driving the Lua iterator protocol.
///
/// Each returned Vec contains [idx, line1, col1, line2, col2].
fn collect_selections(
    doc: &LuaTable,
    sort_intra: bool,
    idx_reverse: LuaValue,
) -> LuaResult<Vec<Vec<LuaValue>>> {
    let ret: LuaMultiValue = doc.call_method("get_selections", (sort_intra, idx_reverse))?;
    let mut ret_vals = ret.into_iter();
    let iter_fn = match ret_vals.next() {
        Some(LuaValue::Function(f)) => f,
        _ => {
            return Err(mlua::Error::runtime(
                "get_selections: expected iterator function",
            ));
        }
    };
    let invariant = ret_vals.next().unwrap_or(LuaValue::Nil);
    let mut control = ret_vals.next().unwrap_or(LuaValue::Nil);

    let mut results = Vec::new();
    loop {
        let result: LuaMultiValue = iter_fn.call((invariant.clone(), control))?;
        let vals: Vec<LuaValue> = result.into_iter().collect();
        if vals.first().is_none_or(|v| v.is_nil()) {
            break;
        }
        control = vals[0].clone();
        results.push(vals);
    }
    Ok(results)
}

/// Gets the current doc: active DocView's doc, or last_view's doc as fallback.
fn get_doc(lua: &Lua, state: &LuaTable) -> LuaResult<Option<LuaTable>> {
    let core: LuaTable = require_table(lua, "core")?;
    let doc_view_class: LuaTable = require_table(lua, "core.docview")?;
    let command_view_class: LuaTable = require_table(lua, "core.commandview")?;
    let active_view: LuaTable = core.get("active_view")?;

    let is_dv: bool = active_view.call_method("is", doc_view_class)?;
    let is_cv: bool = active_view.call_method("is", command_view_class)?;

    if is_dv && !is_cv {
        return active_view.get("doc");
    }
    let last_view: LuaValue = state.get("last_view")?;
    if let LuaValue::Table(lv) = last_view {
        let doc: LuaValue = lv.get("doc")?;
        if let LuaValue::Table(d) = doc {
            return Ok(Some(d));
        }
    }
    Ok(None)
}

/// Builds the find tooltip string from keymap bindings and state flags.
fn get_find_tooltip(lua: &Lua, state: &LuaTable) -> LuaResult<String> {
    let keymap: LuaTable = require_table(lua, "core.keymap")?;
    let find_regex: bool = state.get("find_regex")?;
    let case_sensitive: bool = state.get("case_sensitive")?;
    let whole_word: bool = state.get("whole_word")?;
    let in_selection: bool = state.get("in_selection")?;

    let rf: LuaValue = keymap.call_function("get_binding", "find-replace:repeat-find")?;
    let sa: LuaValue = keymap.call_function("get_binding", "find-replace:select-all-found")?;
    let ti: LuaValue = keymap.call_function("get_binding", "find-replace:toggle-sensitivity")?;
    let tr: LuaValue = keymap.call_function("get_binding", "find-replace:toggle-regex")?;
    let tw: LuaValue = keymap.call_function("get_binding", "find-replace:toggle-whole-word")?;
    let ts: LuaValue = keymap.call_function("get_binding", "find-replace:toggle-in-selection")?;

    let mut result = String::new();
    if find_regex {
        result.push_str("[Regex] ");
    }
    if case_sensitive {
        result.push_str("[Sensitive] ");
    }
    if whole_word {
        result.push_str("[Whole Word] ");
    }
    if in_selection {
        result.push_str("[In Selection] ");
    }
    if let LuaValue::String(s) = &rf {
        result.push_str(&format!("Press {} to select the next match.", s.to_str()?));
    }
    if let LuaValue::String(s) = &sa {
        result.push_str(&format!(
            " {} selects all matches as multi-cursors.",
            s.to_str()?
        ));
    }
    if let LuaValue::String(s) = &ti {
        result.push_str(&format!(" {} toggles case sensitivity.", s.to_str()?));
    }
    if let LuaValue::String(s) = &tr {
        result.push_str(&format!(" {} toggles regex find.", s.to_str()?));
    }
    if let LuaValue::String(s) = &tw {
        result.push_str(&format!(" {} toggles whole word.", s.to_str()?));
    }
    if let LuaValue::String(s) = &ts {
        result.push_str(&format!(" {} toggles in-selection.", s.to_str()?));
    }
    Ok(result)
}

/// Calls pcall on the search function and updates the preview in the doc view.
fn update_preview(
    lua: &Lua,
    state: &LuaTable,
    sel: &LuaTable,
    search_fn: &LuaFunction,
    text: &str,
) -> LuaResult<()> {
    let last_view: LuaTable = state.get("last_view")?;
    let doc: LuaTable = last_view.get("doc")?;
    let case_sensitive: bool = state.get("case_sensitive")?;
    let find_regex: bool = state.get("find_regex")?;
    let whole_word: bool = state.get("whole_word")?;

    let sel1: LuaValue = sel.get(1)?;
    let sel2: LuaValue = sel.get(2)?;

    let pcall: LuaFunction = lua.globals().get("pcall")?;
    let result: LuaMultiValue = pcall.call((
        search_fn.clone(),
        doc.clone(),
        sel1,
        sel2,
        text,
        case_sensitive,
        find_regex,
        false,
        whole_word,
    ))?;
    let vals: Vec<LuaValue> = result.into_iter().collect();
    let ok = matches!(vals.first(), Some(LuaValue::Boolean(true)));
    let has_line1 = vals.get(1).is_some_and(|v| !v.is_nil());

    if ok && has_line1 && !text.is_empty() {
        let line2 = &vals[3];
        let col2 = &vals[4];
        let line1 = &vals[1];
        let col1 = &vals[2];
        doc.call_method::<()>(
            "set_selection",
            (line2.clone(), col2.clone(), line1.clone(), col1.clone()),
        )?;
        last_view.call_method::<()>("scroll_to_line", (line2.clone(), true))?;
        state.set("found_expression", true)?;
    } else {
        let table_mod: LuaTable = lua.globals().get("table")?;
        let unpack: LuaFunction = table_mod.get("unpack")?;
        let unpacked: LuaMultiValue = unpack.call(sel.clone())?;
        doc.call_method::<()>("set_selection", unpacked)?;
        state.set("found_expression", false)?;
    }
    Ok(())
}

/// Moves `v` to the front of array table `t`, removing any existing occurrence.
fn insert_unique(lua: &Lua, t: &LuaTable, v: &str) -> LuaResult<()> {
    let n = t.raw_len();
    for i in 1..=n {
        let existing: LuaValue = t.get(i)?;
        if let LuaValue::String(s) = &existing {
            if s.to_str()? == v {
                let table_mod: LuaTable = lua.globals().get("table")?;
                let remove: LuaFunction = table_mod.get("remove")?;
                remove.call::<()>((t.clone(), i))?;
                break;
            }
        }
    }
    let table_mod: LuaTable = lua.globals().get("table")?;
    let insert: LuaFunction = table_mod.get("insert")?;
    insert.call::<()>((t.clone(), 1, v))
}

/// Registers find-replace commands and the status bar indicator.
fn register_commands(lua: &Lua) -> LuaResult<()> {
    let state = lua.create_table()?;
    let config: LuaTable = require_table(lua, "core.config")?;
    state.set(
        "case_sensitive",
        config.get::<bool>("find_case_sensitive").unwrap_or(false),
    )?;
    state.set(
        "find_regex",
        config.get::<bool>("find_regex").unwrap_or(false),
    )?;
    state.set(
        "whole_word",
        config.get::<bool>("find_whole_word").unwrap_or(false),
    )?;
    state.set("in_selection", false)?;
    state.set("found_expression", false)?;
    state.set("find_ui_active", false)?;
    let state_key = Arc::new(lua.create_registry_value(state)?);

    let core: LuaTable = require_table(lua, "core")?;
    let command: LuaTable = require_table(lua, "core.command")?;
    let add_fn: LuaFunction = command.get("add")?;

    // --- has_unique_selection predicate commands ---
    let sk = state_key.clone();
    let has_unique_selection = lua.create_function(move |lua, ()| {
        let state: LuaTable = lua.registry_value(&sk)?;
        match get_doc(lua, &state)? {
            None => Ok(false),
            Some(doc) => {
                let mut text: Option<String> = None;
                let sels = collect_selections(&doc, true, LuaValue::Boolean(true))?;
                for vals in &sels {
                    let line1 = &vals[1];
                    let col1 = &vals[2];
                    let line2 = &vals[3];
                    let col2 = &vals[4];
                    if line1 == line2 && col1 == col2 {
                        return Ok(false);
                    }
                    let sel: String = doc.call_method(
                        "get_text",
                        (line1.clone(), col1.clone(), line2.clone(), col2.clone()),
                    )?;
                    if let Some(ref prev) = text {
                        if *prev != sel {
                            return Ok(false);
                        }
                    }
                    text = Some(sel);
                }
                Ok(text.is_some())
            }
        }
    })?;

    let unique_cmds = lua.create_table()?;

    // select_next helper
    let sk = state_key.clone();
    let select_next_fn = lua.create_function(move |lua, reverse: LuaValue| {
        let state: LuaTable = lua.registry_value(&sk)?;
        let doc = get_doc(lua, &state)?.ok_or_else(|| LuaError::runtime("no doc"))?;
        let search: LuaTable = require_table(lua, "core.doc.search")?;
        let whole_word: bool = state.get("whole_word")?;
        let sel: LuaMultiValue = doc.call_method("get_selection", true)?;
        let vals: Vec<LuaValue> = sel.into_iter().collect();
        let (l1, c1, l2, c2) = (
            vals[0].clone(),
            vals[1].clone(),
            vals[2].clone(),
            vals[3].clone(),
        );
        let text: String =
            doc.call_method("get_text", (l1.clone(), c1.clone(), l2.clone(), c2.clone()))?;

        let is_reverse = matches!(reverse, LuaValue::Boolean(true));
        let opt = lua.create_table()?;
        opt.set("wrap", true)?;
        opt.set("whole_word", whole_word)?;
        if is_reverse {
            opt.set("reverse", true)?;
        }

        let (start_line, start_col) = if is_reverse { (l1, c1) } else { (l2, c2) };

        let result: LuaMultiValue =
            search.call_function("find", (doc.clone(), start_line, start_col, text, opt))?;
        let r: Vec<LuaValue> = result.into_iter().collect();
        if r.len() >= 4 && !r[1].is_nil() {
            doc.call_method::<()>(
                "set_selection",
                (r[1].clone(), r[3].clone(), r[0].clone(), r[2].clone()),
            )?;
        }
        Ok(())
    })?;
    let sn_key = Arc::new(lua.create_registry_value(select_next_fn)?);

    let sn_key_c = sn_key.clone();
    unique_cmds.set(
        "find-replace:select-next",
        lua.create_function(move |lua, ()| {
            let f: LuaFunction = lua.registry_value(&sn_key_c)?;
            f.call::<()>(false)
        })?,
    )?;

    let sn_key_c = sn_key.clone();
    unique_cmds.set(
        "find-replace:select-previous",
        lua.create_function(move |lua, ()| {
            let f: LuaFunction = lua.registry_value(&sn_key_c)?;
            f.call::<()>(true)
        })?,
    )?;

    // select_add_next helper
    let sk = state_key.clone();
    let select_add_next_fn = lua.create_function(move |lua, all: LuaValue| {
        let state: LuaTable = lua.registry_value(&sk)?;
        let doc = get_doc(lua, &state)?.ok_or_else(|| LuaError::runtime("no doc"))?;
        let search: LuaTable = require_table(lua, "core.doc.search")?;
        let core: LuaTable = require_table(lua, "core")?;
        let all = matches!(all, LuaValue::Boolean(true));

        let mut il1: Option<LuaValue> = None;
        let mut ic1: Option<LuaValue> = None;

        // Collect selections to avoid iterator interference
        let sel_vals = collect_selections(&doc, true, LuaValue::Boolean(true))?;
        let mut sels = Vec::new();
        for vals in sel_vals {
            sels.push((
                vals[1].clone(),
                vals[2].clone(),
                vals[3].clone(),
                vals[4].clone(),
            ));
        }

        for (l1, c1, l2, c2) in sels {
            if il1.is_none() {
                il1 = Some(l1.clone());
                ic1 = Some(c1.clone());
            }
            let text: String = doc.call_method("get_text", (l1, c1, l2.clone(), c2.clone()))?;
            let mut search_l = l2;
            let mut search_c = c2;
            loop {
                let opt = lua.create_table()?;
                opt.set("wrap", true)?;
                let result: LuaMultiValue = search
                    .call_function("find", (doc.clone(), search_l, search_c, text.clone(), opt))?;
                let r: Vec<LuaValue> = result.into_iter().collect();
                if r[0].is_nil() {
                    break;
                }
                let (rl1, rc1, rl2, rc2) = (r[0].clone(), r[1].clone(), r[2].clone(), r[3].clone());

                if rl1 == *il1.as_ref().unwrap() && rc1 == *ic1.as_ref().unwrap() {
                    break;
                }

                // Check if rl2,rc2 is already in an existing selection
                let mut in_sel = false;
                let check_sels = collect_selections(&doc, true, LuaValue::Boolean(false))?;
                for sv in &check_sels {
                    if is_in_selection(&rl2, &rc2, &sv[1], &sv[2], &sv[3], &sv[4]) {
                        in_sel = true;
                        break;
                    }
                }

                if !in_sel {
                    doc.call_method::<()>(
                        "add_selection",
                        (rl2.clone(), rc2.clone(), rl1.clone(), rc1.clone()),
                    )?;
                    if !all {
                        let active_view: LuaTable = core.get("active_view")?;
                        active_view.call_method::<()>("scroll_to_make_visible", (rl2, rc2))?;
                        return Ok(());
                    }
                }

                if !all {
                    break;
                }
                search_l = r[2].clone();
                search_c = r[3].clone();
            }
            if all {
                break;
            }
        }
        Ok(())
    })?;
    let san_key = Arc::new(lua.create_registry_value(select_add_next_fn)?);

    let san_key_c = san_key.clone();
    unique_cmds.set(
        "find-replace:select-add-next",
        lua.create_function(move |lua, ()| {
            let f: LuaFunction = lua.registry_value(&san_key_c)?;
            f.call::<()>(false)
        })?,
    )?;

    let san_key_c = san_key.clone();
    unique_cmds.set(
        "find-replace:select-add-all",
        lua.create_function(move |lua, ()| {
            let f: LuaFunction = lua.registry_value(&san_key_c)?;
            f.call::<()>(true)
        })?,
    )?;

    add_fn.call::<()>((has_unique_selection, unique_cmds))?;

    // --- DocView predicate commands ---
    let dv_cmds = lua.create_table()?;

    // find-replace:find
    let sk = state_key.clone();
    dv_cmds.set(
        "find-replace:find",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;

            let active_view: LuaTable = core.get("active_view")?;
            state.set("last_view", active_view.clone())?;

            let doc: LuaTable = active_view.get("doc")?;
            let sel: LuaMultiValue = doc.call_method("get_selection", true)?;
            let sel_vals: Vec<LuaValue> = sel.into_iter().collect();
            let sel_tbl = lua.create_table()?;
            for (i, v) in sel_vals.iter().enumerate() {
                sel_tbl.set(i + 1, v.clone())?;
            }
            state.set("last_sel", sel_tbl.clone())?;

            // Detect multi-line selection for in_selection mode
            let sel_l1 = lua_to_i64(&sel_vals[0]);
            let sel_l2 = lua_to_i64(&sel_vals[2]);
            if sel_l1 != sel_l2 {
                // Multi-line selection: store boundaries for in_selection constraint
                state.set("selection_bounds", sel_tbl.clone())?;
            } else {
                state.set("selection_bounds", LuaValue::Nil)?;
            }

            let table_mod: LuaTable = lua.globals().get("table")?;
            let unpack: LuaFunction = table_mod.get("unpack")?;
            let unpacked: LuaMultiValue = unpack.call(sel_tbl)?;
            let text: String = doc.call_method("get_text", unpacked)?;
            state.set("found_expression", false)?;
            state.set("find_ui_active", true)?;

            let tooltip = get_find_tooltip(lua, &state)?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show_tooltip", tooltip)?;

            // Create the search_fn that wraps search.find with options,
            // constraining results to selection bounds when in_selection is active.
            let sk2 = sk.clone();
            let search_fn = lua.create_function(
                move |lua, args: LuaMultiValue| -> LuaResult<LuaMultiValue> {
                    let state: LuaTable = lua.registry_value(&sk2)?;
                    let search: LuaTable = require_table(lua, "core.doc.search")?;
                    let whole_word: bool = state.get("whole_word")?;
                    let in_selection: bool = state.get("in_selection")?;
                    let a: Vec<LuaValue> = args.into_iter().collect();
                    // args: doc, line, col, text, case_sensitive, find_regex, find_reverse, _whole_word
                    let case_sensitive = matches!(a.get(4), Some(LuaValue::Boolean(true)));
                    let find_regex = matches!(a.get(5), Some(LuaValue::Boolean(true)));
                    let find_reverse = matches!(a.get(6), Some(LuaValue::Boolean(true)));
                    let opt = lua.create_table()?;
                    opt.set("no_case", !case_sensitive)?;
                    opt.set("regex", find_regex)?;
                    opt.set("reverse", find_reverse)?;
                    opt.set("whole_word", whole_word)?;

                    // When constraining to selection, disable wrap to avoid infinite loops
                    let bounds = if in_selection {
                        match state.get::<LuaValue>("selection_bounds")? {
                            LuaValue::Table(b) => {
                                opt.set("wrap", false)?;
                                let bl1 = lua_to_i64(&b.get::<LuaValue>(1)?);
                                let bc1 = lua_to_i64(&b.get::<LuaValue>(2)?);
                                let bl2 = lua_to_i64(&b.get::<LuaValue>(3)?);
                                let bc2 = lua_to_i64(&b.get::<LuaValue>(4)?);
                                Some((bl1, bc1, bl2, bc2))
                            }
                            _ => {
                                opt.set("wrap", true)?;
                                None
                            }
                        }
                    } else {
                        opt.set("wrap", true)?;
                        None
                    };

                    let result: LuaMultiValue = search.call_function(
                        "find",
                        (a[0].clone(), a[1].clone(), a[2].clone(), a[3].clone(), opt),
                    )?;

                    if let Some((bl1, bc1, bl2, bc2)) = bounds {
                        let r: Vec<LuaValue> = result.into_iter().collect();
                        if r.len() >= 4 && !r[0].is_nil() {
                            let rl1 = lua_to_i64(&r[0]);
                            let rc1 = lua_to_i64(&r[1]);
                            let rl2 = lua_to_i64(&r[2]);
                            let rc2 = lua_to_i64(&r[3]);
                            // Match start must be at or after selection start
                            let start_ok = rl1 > bl1 || (rl1 == bl1 && rc1 >= bc1);
                            // Match end must be at or before selection end
                            let end_ok = rl2 < bl2 || (rl2 == bl2 && rc2 <= bc2);
                            if start_ok && end_ok {
                                return Ok(LuaMultiValue::from_vec(r));
                            }
                            return Ok(LuaMultiValue::new());
                        }
                        return Ok(LuaMultiValue::from_vec(r));
                    }

                    Ok(result)
                },
            )?;

            let sk3 = sk.clone();
            let sfn_key = Arc::new(lua.create_registry_value(search_fn)?);

            // Build command_view options
            let opts = lua.create_table()?;
            opts.set("text", text)?;
            opts.set("select_text", true)?;
            opts.set("show_suggestions", false)?;

            // submit
            let sk_sub = sk3.clone();
            let sfn_sub = sfn_key.clone();
            opts.set(
                "submit",
                lua.create_function(move |lua, (text, _item): (String, LuaValue)| {
                    let state: LuaTable = lua.registry_value(&sk_sub)?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let prev_find: LuaTable = core.get("previous_find")?;
                    insert_unique(lua, &prev_find, &text)?;
                    let sv: LuaTable = core.get("status_view")?;
                    sv.call_method::<()>("remove_tooltip", ())?;
                    state.set("find_ui_active", false)?;

                    let found: bool = state.get("found_expression")?;
                    if found {
                        let sfn: LuaFunction = lua.registry_value(&sfn_sub)?;
                        state.set("last_fn", sfn)?;
                        state.set("last_text", text)?;
                    } else {
                        let string_mod: LuaTable = lua.globals().get("string")?;
                        let msg: String =
                            string_mod.call_function("format", ("Couldn't find %q", text))?;
                        core.call_function::<()>("error", msg)?;
                        let last_view: LuaTable = state.get("last_view")?;
                        let doc: LuaTable = last_view.get("doc")?;
                        let sel: LuaTable = state.get("last_sel")?;
                        let table_mod: LuaTable = lua.globals().get("table")?;
                        let unpack: LuaFunction = table_mod.get("unpack")?;
                        let unpacked: LuaMultiValue = unpack.call(sel.clone())?;
                        doc.call_method::<()>("set_selection", unpacked)?;
                        let unpacked2: LuaMultiValue = unpack.call(sel)?;
                        last_view.call_method::<()>("scroll_to_make_visible", unpacked2)?;
                    }
                    Ok(())
                })?,
            )?;

            // suggest
            let sk_sug = sk3.clone();
            let sfn_sug = sfn_key.clone();
            opts.set(
                "suggest",
                lua.create_function(move |lua, text: String| {
                    let state: LuaTable = lua.registry_value(&sk_sug)?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let sel: LuaTable = state.get("last_sel")?;
                    let sfn: LuaFunction = lua.registry_value(&sfn_sug)?;
                    update_preview(lua, &state, &sel, &sfn, &text)?;
                    state.set("last_fn", sfn)?;
                    state.set("last_text", text)?;
                    core.get::<LuaValue>("previous_find")
                })?,
            )?;

            // cancel
            let sk_can = sk3;
            opts.set(
                "cancel",
                lua.create_function(move |lua, explicit: LuaValue| {
                    let state: LuaTable = lua.registry_value(&sk_can)?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    sv.call_method::<()>("remove_tooltip", ())?;
                    state.set("find_ui_active", false)?;
                    if matches!(explicit, LuaValue::Boolean(true)) {
                        let last_view: LuaTable = state.get("last_view")?;
                        let doc: LuaTable = last_view.get("doc")?;
                        let sel: LuaTable = state.get("last_sel")?;
                        let table_mod: LuaTable = lua.globals().get("table")?;
                        let unpack: LuaFunction = table_mod.get("unpack")?;
                        let unpacked: LuaMultiValue = unpack.call(sel.clone())?;
                        doc.call_method::<()>("set_selection", unpacked)?;
                        let unpacked2: LuaMultiValue = unpack.call(sel)?;
                        last_view.call_method::<()>("scroll_to_make_visible", unpacked2)?;
                    }
                    Ok(())
                })?,
            )?;

            let cv: LuaTable = core.get("command_view")?;
            cv.call_method::<()>("enter", ("Find Text", opts))
        })?,
    )?;

    // find-replace:replace
    let sk = state_key.clone();
    dv_cmds.set(
        "find-replace:replace",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            do_find_replace(lua, &state, false)
        })?,
    )?;

    // find-replace:replace-in-selection
    let sk = state_key.clone();
    dv_cmds.set(
        "find-replace:replace-in-selection",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            do_find_replace(lua, &state, true)
        })?,
    )?;

    // find-replace:replace-symbol
    let sk = state_key.clone();
    dv_cmds.set(
        "find-replace:replace-symbol",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let config: LuaTable = require_table(lua, "core.config")?;
            let doc = get_doc(lua, &state)?.ok_or_else(|| LuaError::runtime("no doc"))?;

            let mut first = String::new();
            let has_sel: bool = doc.call_method("has_selection", ())?;
            if has_sel {
                let sel: LuaMultiValue = doc.call_method("get_selection", ())?;
                let text: String = doc.call_method("get_text", sel)?;
                let string_mod: LuaTable = lua.globals().get("string")?;
                let symbol_pattern: String = config.get("symbol_pattern")?;
                let matched: LuaValue =
                    string_mod.call_function("match", (text, symbol_pattern))?;
                if let LuaValue::String(s) = matched {
                    first = s.to_str()?.to_string();
                }
            }

            let fn_replace = lua.create_function(
                |lua, (text, old, new): (String, String, String)| -> LuaResult<LuaMultiValue> {
                    let config: LuaTable = require_table(lua, "core.config")?;
                    let symbol_pattern: String = config.get("symbol_pattern")?;
                    let string_mod: LuaTable = lua.globals().get("string")?;
                    let n_tbl = lua.create_table()?;
                    n_tbl.set("n", 0)?;
                    let n_tbl_key = lua.create_registry_value(n_tbl)?;
                    let replacer = lua.create_function(move |lua, sym: String| {
                        if old == sym {
                            let n_tbl: LuaTable = lua.registry_value(&n_tbl_key)?;
                            let n: i64 = n_tbl.get("n")?;
                            n_tbl.set("n", n + 1)?;
                            Ok(LuaValue::String(lua.create_string(&new)?))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    })?;
                    string_mod.call_function("gsub", (text, symbol_pattern, replacer))
                },
            )?;

            do_replace(lua, &state, "Symbol", &first, fn_replace)
        })?,
    )?;

    add_fn.call::<()>(("core.docview!", dv_cmds))?;

    // --- valid_for_finding predicate commands ---
    let sk = state_key.clone();
    let valid_for_finding = lua.create_function(move |lua, ()| {
        let state: LuaTable = lua.registry_value(&sk)?;
        let core: LuaTable = require_table(lua, "core")?;
        let command_view_class: LuaTable = require_table(lua, "core.commandview")?;
        let doc_view_class: LuaTable = require_table(lua, "core.docview")?;
        let active_view: LuaTable = core.get("active_view")?;
        let is_cv: bool = active_view.call_method("is", command_view_class)?;
        if is_cv {
            let last_view: LuaValue = state.get("last_view")?;
            if !last_view.is_nil() {
                return Ok(LuaMultiValue::from_vec(vec![
                    LuaValue::Boolean(true),
                    last_view,
                ]));
            }
        }
        let is_dv: bool = active_view.call_method("is", doc_view_class)?;
        Ok(LuaMultiValue::from_vec(vec![
            LuaValue::Boolean(is_dv),
            LuaValue::Table(active_view),
        ]))
    })?;

    let finding_cmds = lua.create_table()?;

    // find-replace:repeat-find
    let sk = state_key.clone();
    finding_cmds.set(
        "find-replace:repeat-find",
        lua.create_function(move |lua, dv: LuaTable| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let last_fn: LuaValue = state.get("last_fn")?;
            if last_fn.is_nil() {
                core.call_function::<()>("error", "No find to continue from")
            } else {
                let last_fn = match last_fn {
                    LuaValue::Function(f) => f,
                    _ => return core.call_function::<()>("error", "No find to continue from"),
                };
                let doc: LuaTable = dv.get("doc")?;
                let sel: LuaMultiValue = doc.call_method("get_selection", true)?;
                let vals: Vec<LuaValue> = sel.into_iter().collect();
                let last_text: String = state.get("last_text")?;
                let case_sensitive: bool = state.get("case_sensitive")?;
                let find_regex: bool = state.get("find_regex")?;
                let whole_word: bool = state.get("whole_word")?;
                let result: LuaMultiValue = last_fn.call((
                    doc.clone(),
                    vals[2].clone(),
                    vals[3].clone(),
                    last_text.clone(),
                    case_sensitive,
                    find_regex,
                    false,
                    whole_word,
                ))?;
                let r: Vec<LuaValue> = result.into_iter().collect();
                if !r[0].is_nil() {
                    doc.call_method::<()>(
                        "set_selection",
                        (r[2].clone(), r[3].clone(), r[0].clone(), r[1].clone()),
                    )?;
                    dv.call_method::<()>("scroll_to_line", (r[2].clone(), true))?;
                } else {
                    let string_mod: LuaTable = lua.globals().get("string")?;
                    let msg: String =
                        string_mod.call_function("format", ("Couldn't find %q", last_text))?;
                    core.call_function::<()>("error", msg)?;
                }
                Ok(())
            }
        })?,
    )?;

    // find-replace:previous-find
    let sk = state_key.clone();
    finding_cmds.set(
        "find-replace:previous-find",
        lua.create_function(move |lua, dv: LuaTable| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let last_fn: LuaValue = state.get("last_fn")?;
            if last_fn.is_nil() {
                core.call_function::<()>("error", "No find to continue from")
            } else {
                let last_fn = match last_fn {
                    LuaValue::Function(f) => f,
                    _ => return core.call_function::<()>("error", "No find to continue from"),
                };
                let doc: LuaTable = dv.get("doc")?;
                let sel: LuaMultiValue = doc.call_method("get_selection", true)?;
                let vals: Vec<LuaValue> = sel.into_iter().collect();
                let last_text: String = state.get("last_text")?;
                let case_sensitive: bool = state.get("case_sensitive")?;
                let find_regex: bool = state.get("find_regex")?;
                let whole_word: bool = state.get("whole_word")?;
                // Use sl1, sc1 for reverse search
                let result: LuaMultiValue = last_fn.call((
                    doc.clone(),
                    vals[0].clone(),
                    vals[1].clone(),
                    last_text.clone(),
                    case_sensitive,
                    find_regex,
                    true,
                    whole_word,
                ))?;
                let r: Vec<LuaValue> = result.into_iter().collect();
                if !r[0].is_nil() {
                    doc.call_method::<()>(
                        "set_selection",
                        (r[2].clone(), r[3].clone(), r[0].clone(), r[1].clone()),
                    )?;
                    dv.call_method::<()>("scroll_to_line", (r[2].clone(), true))?;
                } else {
                    let string_mod: LuaTable = lua.globals().get("string")?;
                    let msg: String =
                        string_mod.call_function("format", ("Couldn't find %q", last_text))?;
                    core.call_function::<()>("error", msg)?;
                }
                Ok(())
            }
        })?,
    )?;

    // find-replace:select-all-found
    let sk = state_key.clone();
    finding_cmds.set(
        "find-replace:select-all-found",
        lua.create_function(move |lua, dv: LuaTable| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let style: LuaTable = require_table(lua, "core.style")?;
            let search: LuaTable = require_table(lua, "core.doc.search")?;

            let last_text: LuaValue = state.get("last_text")?;
            let lt_str = match &last_text {
                LuaValue::String(s) if !s.to_str()?.is_empty() => s.to_str()?.to_string(),
                _ => {
                    core.call_function::<()>(
                        "error",
                        "No find text to convert into multi-cursors",
                    )?;
                    return Ok(());
                }
            };

            let case_sensitive: bool = state.get("case_sensitive")?;
            let find_regex: bool = state.get("find_regex")?;
            let whole_word: bool = state.get("whole_word")?;

            let opt = lua.create_table()?;
            opt.set("no_case", !case_sensitive)?;
            opt.set("regex", find_regex)?;
            opt.set("whole_word", whole_word)?;

            let doc: LuaTable = dv.get("doc")?;
            let mut matches: Vec<(LuaValue, LuaValue, LuaValue, LuaValue)> = Vec::new();
            let mut line = LuaValue::Integer(1);
            let mut col = LuaValue::Integer(1);

            loop {
                let result: LuaMultiValue = search.call_function(
                    "find",
                    (
                        doc.clone(),
                        line.clone(),
                        col.clone(),
                        lt_str.clone(),
                        opt.clone(),
                    ),
                )?;
                let r: Vec<LuaValue> = result.into_iter().collect();
                if r[0].is_nil() {
                    break;
                }
                let (l1, c1, l2, c2) = (r[0].clone(), r[1].clone(), r[2].clone(), r[3].clone());
                matches.push((l2.clone(), c2.clone(), l1.clone(), c1.clone()));

                // Advance past match
                let (mut next_line, mut next_col) = (l2.clone(), c2.clone());
                if l1 == l2 && c1 == c2 {
                    let pos: LuaMultiValue =
                        doc.call_method("position_offset", (l2.clone(), c2.clone(), 1))?;
                    let pv: Vec<LuaValue> = pos.into_iter().collect();
                    if pv[0] == l2 && pv[1] == c2 {
                        break;
                    }
                    next_line = pv[0].clone();
                    next_col = pv[1].clone();
                }
                line = next_line;
                col = next_col;
            }

            if matches.is_empty() {
                let string_mod: LuaTable = lua.globals().get("string")?;
                let msg: String =
                    string_mod.call_function("format", ("Couldn't find %q", lt_str))?;
                core.call_function::<()>("error", msg)?;
                return Ok(());
            }

            let first = &matches[0];
            doc.call_method::<()>(
                "set_selection",
                (
                    first.0.clone(),
                    first.1.clone(),
                    first.2.clone(),
                    first.3.clone(),
                ),
            )?;
            for m in &matches[1..] {
                doc.call_method::<()>(
                    "add_selection",
                    (m.0.clone(), m.1.clone(), m.2.clone(), m.3.clone()),
                )?;
            }
            dv.call_method::<()>("scroll_to_line", (first.2.clone(), true))?;
            let sv: LuaTable = core.get("status_view")?;
            let text_color: LuaValue = style.get("text")?;
            sv.call_method::<()>(
                "show_message",
                (
                    "i",
                    text_color,
                    format!("{} selection(s) active", matches.len()),
                ),
            )?;
            Ok(())
        })?,
    )?;

    add_fn.call::<()>((valid_for_finding, finding_cmds))?;

    // --- CommandView predicate commands (toggles) ---
    let toggle_cmds = lua.create_table()?;

    let sk = state_key.clone();
    toggle_cmds.set(
        "find-replace:toggle-sensitivity",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let cs: bool = state.get("case_sensitive")?;
            state.set("case_sensitive", !cs)?;
            let tooltip = get_find_tooltip(lua, &state)?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show_tooltip", tooltip)?;
            let last_sel: LuaValue = state.get("last_sel")?;
            if let LuaValue::Table(sel) = last_sel {
                let last_fn: LuaValue = state.get("last_fn")?;
                let last_text: LuaValue = state.get("last_text")?;
                if let (LuaValue::Function(f), LuaValue::String(t)) = (last_fn, last_text) {
                    let text = t.to_str()?.to_string();
                    update_preview(lua, &state, &sel, &f, &text)?;
                }
            }
            Ok(())
        })?,
    )?;

    let sk = state_key.clone();
    toggle_cmds.set(
        "find-replace:toggle-regex",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let fr: bool = state.get("find_regex")?;
            state.set("find_regex", !fr)?;
            let tooltip = get_find_tooltip(lua, &state)?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show_tooltip", tooltip)?;
            let last_sel: LuaValue = state.get("last_sel")?;
            if let LuaValue::Table(sel) = last_sel {
                let last_fn: LuaValue = state.get("last_fn")?;
                let last_text: LuaValue = state.get("last_text")?;
                if let (LuaValue::Function(f), LuaValue::String(t)) = (last_fn, last_text) {
                    let text = t.to_str()?.to_string();
                    update_preview(lua, &state, &sel, &f, &text)?;
                }
            }
            Ok(())
        })?,
    )?;

    let sk = state_key.clone();
    toggle_cmds.set(
        "find-replace:toggle-whole-word",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let ww: bool = state.get("whole_word")?;
            state.set("whole_word", !ww)?;
            let tooltip = get_find_tooltip(lua, &state)?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show_tooltip", tooltip)?;
            let last_sel: LuaValue = state.get("last_sel")?;
            if let LuaValue::Table(sel) = last_sel {
                let last_fn: LuaValue = state.get("last_fn")?;
                let last_text: LuaValue = state.get("last_text")?;
                if let (LuaValue::Function(f), LuaValue::String(t)) = (last_fn, last_text) {
                    let text = t.to_str()?.to_string();
                    update_preview(lua, &state, &sel, &f, &text)?;
                }
            }
            Ok(())
        })?,
    )?;

    let sk = state_key.clone();
    toggle_cmds.set(
        "find-replace:toggle-in-selection",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let is: bool = state.get("in_selection")?;
            state.set("in_selection", !is)?;
            let tooltip = get_find_tooltip(lua, &state)?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("show_tooltip", tooltip)?;
            let last_sel: LuaValue = state.get("last_sel")?;
            if let LuaValue::Table(sel) = last_sel {
                let last_fn: LuaValue = state.get("last_fn")?;
                let last_text: LuaValue = state.get("last_text")?;
                if let (LuaValue::Function(f), LuaValue::String(t)) = (last_fn, last_text) {
                    let text = t.to_str()?.to_string();
                    update_preview(lua, &state, &sel, &f, &text)?;
                }
            }
            Ok(())
        })?,
    )?;

    add_fn.call::<()>(("core.commandview", toggle_cmds))?;

    // --- Status bar item ---
    let sk = state_key;
    let sv: LuaTable = core.get("status_view")?;
    let status_view_class: LuaTable = require_table(lua, "core.statusview")?;
    let item_tbl = lua.create_table()?;

    let sk_pred = sk.clone();
    item_tbl.set(
        "predicate",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk_pred)?;
            let active: bool = state.get("find_ui_active")?;
            if !active {
                return Ok(false);
            }
            let core: LuaTable = require_table(lua, "core")?;
            let command_view_class: LuaTable = require_table(lua, "core.commandview")?;
            let av: LuaValue = core.get("active_view")?;
            if let LuaValue::Table(v) = av {
                let is_cv: bool = v.call_method("is", command_view_class)?;
                return Ok(is_cv);
            }
            Ok(false)
        })?,
    )?;

    item_tbl.set("name", "find:state")?;
    let item_class: LuaTable = status_view_class.get("Item")?;
    let right_val: LuaValue = item_class.get("RIGHT")?;
    item_tbl.set("alignment", right_val)?;

    item_tbl.set(
        "get_item",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let style: LuaTable = require_table(lua, "core.style")?;
            let accent: LuaValue = style.get("accent")?;
            let dim: LuaValue = style.get("dim")?;

            let cs: bool = state.get("case_sensitive")?;
            let fr: bool = state.get("find_regex")?;
            let ww: bool = state.get("whole_word")?;
            let is: bool = state.get("in_selection")?;

            let result = lua.create_table()?;
            result.push(if cs { accent.clone() } else { dim.clone() })?;
            result.push("Aa")?;
            result.push(dim.clone())?;
            result.push(" ")?;
            result.push(if fr { accent.clone() } else { dim.clone() })?;
            result.push(".*")?;
            result.push(dim.clone())?;
            result.push(" ")?;
            result.push(if ww { accent.clone() } else { dim.clone() })?;
            result.push("W")?;
            result.push(dim.clone())?;
            result.push(" ")?;
            result.push(if is { accent } else { dim })?;
            result.push("S")?;
            Ok(result)
        })?,
    )?;

    item_tbl.set(
        "tooltip",
        "Search toggles: case, regex, whole word, in selection",
    )?;
    sv.call_method::<()>("add_item", item_tbl)?;

    Ok(())
}

/// Extracts an i64 from a LuaValue (Integer or Number).
fn lua_to_i64(v: &LuaValue) -> i64 {
    match v {
        LuaValue::Integer(i) => *i,
        LuaValue::Number(n) => *n as i64,
        _ => 0,
    }
}

/// Checks if (line, col) is inside the selection (l1,c1)-(l2,c2).
fn is_in_selection(
    line: &LuaValue,
    col: &LuaValue,
    l1: &LuaValue,
    c1: &LuaValue,
    l2: &LuaValue,
    c2: &LuaValue,
) -> bool {
    let line = lua_to_i64(line);
    let col = lua_to_i64(col);
    let l1 = lua_to_i64(l1);
    let c1 = lua_to_i64(c1);
    let l2 = lua_to_i64(l2);
    let c2 = lua_to_i64(c2);

    if line < l1 || line > l2 {
        return false;
    }
    if line == l1 && col <= c1 {
        return false;
    }
    if line == l2 && col > c2 {
        return false;
    }
    true
}

/// Executes the find_replace flow (for "replace" and "replace-in-selection" commands).
fn do_find_replace(lua: &Lua, state: &LuaTable, in_selection: bool) -> LuaResult<()> {
    let doc = get_doc(lua, state)?.ok_or_else(|| LuaError::runtime("no doc"))?;
    let sel: LuaMultiValue = doc.call_method("get_selection", ())?;
    let sv: Vec<LuaValue> = sel.into_iter().collect();
    let (l1, c1, l2, c2) = (sv[0].clone(), sv[1].clone(), sv[2].clone(), sv[3].clone());

    let mut default = String::new();
    if !in_selection {
        let text: String = doc.call_method("get_text", (l1.clone(), c1, l2.clone(), c2.clone()))?;
        doc.call_method::<()>("set_selection", (l2.clone(), c2.clone(), l2.clone(), c2))?;
        if l1 == l2 {
            default = text;
        }
    }

    let state_key = Arc::new(lua.create_registry_value(state.clone())?);
    let fn_replace =
        lua.create_function(move |lua, (text, old, new): (String, String, String)| {
            let state: LuaTable = lua.registry_value(&state_key)?;
            let find_regex: bool = state.get("find_regex")?;
            let doc_native: LuaTable = require_table(lua, "doc_native")?;

            if !find_regex {
                let opts = lua.create_table()?;
                opts.set("regex", false)?;
                let result: LuaTable = doc_native
                    .call_function("replace", (text.clone(), old.clone(), new.clone(), opts))?;
                let native_text: LuaValue = result.get("text")?;
                let native_count: LuaValue = result.get("count")?;
                if !native_text.is_nil() {
                    return Ok(LuaMultiValue::from_vec(vec![native_text, native_count]));
                }
                // Fallback to Lua gsub
                let string_mod: LuaTable = lua.globals().get("string")?;
                let escaped_old: String = string_mod.call_function("gsub", (old, "%W", "%%%1"))?;
                let escaped_new: String = string_mod.call_function("gsub", (new, "%%", "%%%%"))?;
                return string_mod.call_function("gsub", (text, escaped_old, escaped_new));
            }

            let opts = lua.create_table()?;
            opts.set("regex", true)?;
            let result: LuaTable = doc_native
                .call_function("replace", (text.clone(), old.clone(), new.clone(), opts))?;
            let native_text: LuaValue = result.get("text")?;
            let native_count: LuaValue = result.get("count")?;
            if !native_text.is_nil() {
                return Ok(LuaMultiValue::from_vec(vec![native_text, native_count]));
            }
            // Fallback to regex module
            let regex_mod: LuaTable = lua.globals().get("regex")?;
            let compiled: LuaValue = regex_mod.call_function("compile", (old, "m"))?;
            regex_mod.call_function("gsub", (compiled, text, new))
        })?;

    do_replace(lua, state, "Text", &default, fn_replace)
}

/// Implements the replace flow: prompts for find text, then replace text, performs replacement.
fn do_replace(
    lua: &Lua,
    state: &LuaTable,
    kind: &str,
    default: &str,
    fn_replace: LuaFunction,
) -> LuaResult<()> {
    let core: LuaTable = require_table(lua, "core")?;
    let tooltip = get_find_tooltip(lua, state)?;
    let sv: LuaTable = core.get("status_view")?;
    sv.call_method::<()>("show_tooltip", tooltip)?;
    state.set("find_ui_active", true)?;

    let state_key = Arc::new(lua.create_registry_value(state.clone())?);
    let fn_replace_key = Arc::new(lua.create_registry_value(fn_replace)?);
    let kind_owned = kind.to_string();

    let opts = lua.create_table()?;
    opts.set("text", default)?;
    opts.set("select_text", true)?;
    opts.set("show_suggestions", false)?;

    // submit: prompt for replacement text
    let sk = state_key.clone();
    let frk = fn_replace_key.clone();
    let kind_s = kind_owned.clone();
    opts.set(
        "submit",
        lua.create_function(move |lua, old: String| {
            let core: LuaTable = require_table(lua, "core")?;
            let prev_find: LuaTable = core.get("previous_find")?;
            insert_unique(lua, &prev_find, &old)?;

            let label = format!("Replace {} {:?} With", kind_s, old);
            let inner_opts = lua.create_table()?;
            inner_opts.set("text", old.clone())?;
            inner_opts.set("select_text", true)?;
            inner_opts.set("show_suggestions", false)?;

            let sk2 = sk.clone();
            let frk2 = frk.clone();
            let kind_s2 = kind_s.clone();
            let old2 = old;
            inner_opts.set(
                "submit",
                lua.create_function(move |lua, new: String| {
                    let state: LuaTable = lua.registry_value(&sk2)?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    sv.call_method::<()>("remove_tooltip", ())?;
                    state.set("find_ui_active", false)?;
                    let prev_replace: LuaTable = core.get("previous_replace")?;
                    insert_unique(lua, &prev_replace, &new)?;

                    let doc = get_doc(lua, &state)?.ok_or_else(|| LuaError::runtime("no doc"))?;
                    let fn_replace: LuaFunction = lua.registry_value(&frk2)?;

                    let replacer = lua.create_function({
                        let old_c = old2.clone();
                        let new_c = new.clone();
                        let frk3 = frk2.clone();
                        move |lua, text: String| {
                            let fr: LuaFunction = lua.registry_value(&frk3)?;
                            fr.call::<LuaMultiValue>((text, old_c.clone(), new_c.clone()))
                        }
                    })?;
                    let _ = fn_replace;
                    let results: LuaTable = doc.call_method("replace", replacer)?;

                    let mut n: i64 = 0;
                    for pair in results.pairs::<LuaValue, i64>() {
                        let (_, v) = pair?;
                        n += v;
                    }

                    core.call_function::<()>(
                        "log",
                        format!(
                            "Replaced {} instance(s) of {} {:?} with {:?}",
                            n, kind_s2, old2, new
                        ),
                    )
                })?,
            )?;

            inner_opts.set(
                "suggest",
                lua.create_function(|lua, ()| {
                    let core: LuaTable = require_table(lua, "core")?;
                    core.get::<LuaValue>("previous_replace")
                })?,
            )?;

            let sk4 = sk.clone();
            inner_opts.set(
                "cancel",
                lua.create_function(move |lua, ()| {
                    let state: LuaTable = lua.registry_value(&sk4)?;
                    let core: LuaTable = require_table(lua, "core")?;
                    let sv: LuaTable = core.get("status_view")?;
                    sv.call_method::<()>("remove_tooltip", ())?;
                    state.set("find_ui_active", false)
                })?,
            )?;

            let cv: LuaTable = core.get("command_view")?;
            cv.call_method::<()>("enter", (label, inner_opts))
        })?,
    )?;

    // suggest for find text
    opts.set(
        "suggest",
        lua.create_function(|lua, ()| {
            let core: LuaTable = require_table(lua, "core")?;
            core.get::<LuaValue>("previous_find")
        })?,
    )?;

    // cancel
    let sk = state_key;
    opts.set(
        "cancel",
        lua.create_function(move |lua, ()| {
            let state: LuaTable = lua.registry_value(&sk)?;
            let core: LuaTable = require_table(lua, "core")?;
            let sv: LuaTable = core.get("status_view")?;
            sv.call_method::<()>("remove_tooltip", ())?;
            state.set("find_ui_active", false)
        })?,
    )?;

    let cv: LuaTable = core.get("command_view")?;
    let label = format!("Find To Replace {kind_owned}");
    cv.call_method::<()>("enter", (label, opts))
}

/// Registers the `core.commands.findreplace` preload entry.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.commands.findreplace",
        lua.create_function(|lua, ()| {
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
