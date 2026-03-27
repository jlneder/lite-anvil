use mlua::prelude::*;

fn require_table(lua: &Lua, name: &str) -> LuaResult<LuaTable> {
    let require: LuaFunction = lua.globals().get("require")?;
    require.call(name)
}

fn minimap_config(lua: &Lua) -> LuaResult<Option<LuaTable>> {
    let config = require_table(lua, "core.config")?;
    let plugins: LuaTable = config.get("plugins")?;
    plugins.get("minimap")
}

fn minimap_enabled(lua: &Lua) -> LuaResult<bool> {
    match minimap_config(lua)? {
        Some(conf) => Ok(conf.get("enabled").unwrap_or(false)),
        None => Ok(false),
    }
}

fn minimap_width(lua: &Lua) -> LuaResult<f64> {
    match minimap_config(lua)? {
        Some(conf) => Ok(conf.get("width").unwrap_or(120.0)),
        None => Ok(120.0),
    }
}

fn minimap_line_height(lua: &Lua) -> LuaResult<f64> {
    match minimap_config(lua)? {
        Some(conf) => Ok(conf.get("line_height").unwrap_or(4.0)),
        None => Ok(2.0),
    }
}

fn is_docview(lua: &Lua, this: &LuaTable) -> LuaResult<bool> {
    let dv_class = require_table(lua, "core.docview")?;
    this.call_method("is", dv_class)
}

/// Returns (minimap_x, minimap_y, minimap_w, minimap_h) for the given DocView.
fn minimap_rect(lua: &Lua, this: &LuaTable) -> LuaResult<(f64, f64, f64, f64)> {
    let position: LuaTable = this.get("position")?;
    let size: LuaTable = this.get("size")?;
    let px: f64 = position.get("x")?;
    let py: f64 = position.get("y")?;
    let sw: f64 = size.get("x")?;
    let sh: f64 = size.get("y")?;
    let mw = minimap_width(lua)?;
    Ok((px + sw - mw, py, mw, sh))
}

/// Creates a color table with the given RGBA components.
fn make_color(lua: &Lua, r: f64, g: f64, b: f64, a: f64) -> LuaResult<LuaTable> {
    let c = lua.create_table()?;
    c.set(1, r)?;
    c.set(2, g)?;
    c.set(3, b)?;
    c.set(4, a)?;
    Ok(c)
}

/// Clones a color table, overriding the alpha channel.
fn color_with_alpha(lua: &Lua, color: &LuaValue, alpha: f64) -> LuaResult<LuaTable> {
    match color {
        LuaValue::Table(t) => {
            let r: f64 = t.get(1).unwrap_or(255.0);
            let g: f64 = t.get(2).unwrap_or(255.0);
            let b: f64 = t.get(3).unwrap_or(255.0);
            make_color(lua, r, g, b, alpha)
        }
        _ => make_color(lua, 200.0, 200.0, 200.0, alpha),
    }
}

fn set_config_defaults(lua: &Lua) -> LuaResult<()> {
    let config = require_table(lua, "core.config")?;
    let plugins: LuaTable = config.get("plugins")?;
    let common = require_table(lua, "core.common")?;

    let defaults = lua.create_table()?;
    defaults.set("enabled", false)?;
    defaults.set("width", 120)?;
    defaults.set("line_height", 4)?;

    let spec = lua.create_table()?;
    spec.set("name", "Minimap")?;

    let enabled_entry = lua.create_table()?;
    enabled_entry.set("label", "Enabled")?;
    enabled_entry.set("description", "Enable the code overview minimap sidebar.")?;
    enabled_entry.set("path", "enabled")?;
    enabled_entry.set("type", "toggle")?;
    enabled_entry.set("default", false)?;
    spec.push(enabled_entry)?;

    let width_entry = lua.create_table()?;
    width_entry.set("label", "Width")?;
    width_entry.set("description", "Width of the minimap in pixels.")?;
    width_entry.set("path", "width")?;
    width_entry.set("type", "number")?;
    width_entry.set("default", 120)?;
    width_entry.set("min", 40)?;
    width_entry.set("max", 200)?;
    spec.push(width_entry)?;

    let lh_entry = lua.create_table()?;
    lh_entry.set("label", "Line Height")?;
    lh_entry.set("description", "Height of each minimap line in pixels.")?;
    lh_entry.set("path", "line_height")?;
    lh_entry.set("type", "number")?;
    lh_entry.set("default", 2)?;
    lh_entry.set("min", 1)?;
    lh_entry.set("max", 4)?;
    spec.push(lh_entry)?;

    defaults.set("config_spec", spec)?;

    let merged: LuaTable =
        common.call_function("merge", (defaults, plugins.get::<LuaValue>("minimap")?))?;
    plugins.set("minimap", merged)?;
    Ok(())
}

/// Draws the minimap for a DocView. Called after the original draw completes.
fn draw_minimap(lua: &Lua, this: &LuaTable) -> LuaResult<()> {
    let doc: LuaTable = this.get("doc")?;
    let lines: LuaTable = doc.get("lines")?;
    let total_lines = lines.raw_len();
    if total_lines == 0 {
        return Ok(());
    }

    let (mx, my, mw, mh) = minimap_rect(lua, this)?;
    let mlh = minimap_line_height(lua)?;

    let renderer: LuaTable = lua.globals().get("renderer")?;
    let style = require_table(lua, "core.style")?;

    // Draw minimap background
    let bg: LuaValue = style.get("background")?;
    let bg_color = color_with_alpha(lua, &bg, 230.0)?;
    renderer.call_function::<()>("draw_rect", (mx, my, mw, mh, bg_color))?;

    // Draw a subtle left border
    let border_color = make_color(lua, 80.0, 80.0, 80.0, 60.0)?;
    renderer.call_function::<()>("draw_rect", (mx, my, 1.0, mh, border_color))?;

    // Determine the range of lines the minimap can display.
    // Center the minimap view around the currently visible region.
    let (vis_min, vis_max): (usize, usize) = this.call_method("get_visible_line_range", ())?;
    let vis_center = (vis_min + vis_max) / 2;
    let lines_that_fit = (mh / mlh).floor() as usize;

    // Compute minimap_start: the first document line shown in the minimap
    let minimap_start = if total_lines <= lines_that_fit {
        1
    } else {
        let half = lines_that_fit / 2;
        let start = vis_center.saturating_sub(half).max(1);
        start.min(total_lines.saturating_sub(lines_that_fit) + 1)
    };
    let minimap_end = (minimap_start + lines_that_fit).min(total_lines + 1);

    let highlighter: LuaTable = doc.get("highlighter")?;
    let syntax_colors: LuaTable = style.get("syntax")?;
    let text_padding = 4.0;
    let usable_width = mw - text_padding * 2.0;
    // Fixed char width based on a reference column count for consistent scaling.
    let config: LuaTable = require_table(lua, "core.config")?;
    let ref_cols: f64 = config.get::<Option<f64>>("line_limit")?.unwrap_or(80.0);
    let fixed_char_w = usable_width / ref_cols;

    let core = require_table(lua, "core")?;
    core.call_function::<()>("push_clip_rect", (mx, my, mw, mh))?;

    // Draw colored blocks for each line.
    // Blocks are shorter than the line height to leave a gap (like Sublime's minimap).
    let block_height = (mlh * 0.6).max(1.0);
    let block_y_pad = (mlh - block_height) / 2.0;

    for line_idx in minimap_start..minimap_end {
        let y_pos = my + (line_idx - minimap_start) as f64 * mlh + block_y_pad;

        let line_info: LuaResult<LuaTable> = highlighter.call_method("get_line", line_idx);
        let tokens_table = match line_info {
            Ok(info) => info.get::<Option<LuaTable>>("tokens")?,
            Err(_) => None,
        };

        if let Some(tokens) = tokens_table {
            let token_count = tokens.raw_len();
            if token_count == 0 {
                continue;
            }

            let scale = fixed_char_w;
            let mut x_off = 0.0;
            let mut idx = 1usize;
            while idx < token_count {
                let token_type: String = tokens.get(idx)?;
                let text: String = tokens.get(idx + 1)?;
                let text_len = text.len();
                if text_len > 0 {
                    let draw_len = if text.ends_with('\n') {
                        text_len - 1
                    } else {
                        text_len
                    };
                    if draw_len > 0 {
                        // Skip leading whitespace — render as empty space.
                        let trimmed = text.trim_start_matches([' ', '\t']);
                        let leading = text_len - trimmed.len();
                        let trimmed_draw = draw_len.saturating_sub(leading);
                        if trimmed_draw > 0 {
                            let seg_x = (x_off + leading as f64 * scale).min(usable_width);
                            let seg_w = (trimmed_draw as f64 * scale)
                                .min(usable_width - seg_x + text_padding);
                            if seg_w > 0.2 {
                                let color: LuaValue = syntax_colors
                                    .get::<Option<LuaValue>>(token_type.as_str())?
                                    .or_else(|| {
                                        syntax_colors
                                            .get::<Option<LuaValue>>("normal")
                                            .ok()
                                            .flatten()
                                    })
                                    .unwrap_or(LuaValue::Nil);
                                let draw_color = color_with_alpha(lua, &color, 130.0)?;
                                renderer.call_function::<()>(
                                    "draw_rect",
                                    (
                                        mx + text_padding + seg_x,
                                        y_pos,
                                        seg_w,
                                        block_height,
                                        draw_color,
                                    ),
                                )?;
                            }
                        }
                    }
                    x_off += text_len as f64 * scale;
                }
                idx += 2;
            }
        }
    }

    // Draw viewport indicator: highlight the currently visible lines
    let sel_color: LuaValue = style.get("selection")?;
    let indicator_color = color_with_alpha(lua, &sel_color, 76.0)?;
    if vis_min >= minimap_start && vis_min < minimap_end {
        let ind_y = my + (vis_min - minimap_start) as f64 * mlh;
        let ind_h = (vis_max - vis_min + 1) as f64 * mlh;
        let clamped_h = ind_h.min(mh - (ind_y - my));
        renderer.call_function::<()>("draw_rect", (mx, ind_y, mw, clamped_h, indicator_color))?;
    }

    core.call_function::<()>("pop_clip_rect", ())?;
    Ok(())
}

fn patch_draw(lua: &Lua) -> LuaResult<()> {
    let doc_view = require_table(lua, "core.docview")?;
    let old: LuaFunction = doc_view.get("draw")?;
    let old_key = lua.create_registry_value(old)?;

    doc_view.set(
        "draw",
        lua.create_function(move |lua, (this, rest): (LuaTable, LuaMultiValue)| {
            // Call original draw
            let old: LuaFunction = lua.registry_value(&old_key)?;
            let mut args = LuaMultiValue::new();
            args.push_back(LuaValue::Table(this.clone()));
            args.extend(rest);
            old.call::<LuaMultiValue>(args)?;

            if !minimap_enabled(lua)? {
                return Ok(());
            }
            if !is_docview(lua, &this)? {
                return Ok(());
            }

            draw_minimap(lua, &this)?;
            Ok(())
        })?,
    )?;
    Ok(())
}

/// Returns true if (x,y) falls within the minimap area.
fn point_in_minimap(lua: &Lua, this: &LuaTable, x: f64, y: f64) -> LuaResult<bool> {
    let (mx, my, mw, mh) = minimap_rect(lua, this)?;
    Ok(x >= mx && x <= mx + mw && y >= my && y <= my + mh)
}

/// Scrolls the DocView so the clicked minimap position becomes the center of the viewport.
fn scroll_to_minimap_position(lua: &Lua, this: &LuaTable, y: f64) -> LuaResult<()> {
    let (_, my, _, mh) = minimap_rect(lua, this)?;
    let mlh = minimap_line_height(lua)?;
    let doc: LuaTable = this.get("doc")?;
    let lines: LuaTable = doc.get("lines")?;
    let total_lines = lines.raw_len();
    if total_lines == 0 {
        return Ok(());
    }

    // Recompute minimap_start the same way draw_minimap does
    let (vis_min, vis_max): (usize, usize) = this.call_method("get_visible_line_range", ())?;
    let vis_center = (vis_min + vis_max) / 2;
    let lines_that_fit = (mh / mlh).floor() as usize;
    let minimap_start = if total_lines <= lines_that_fit {
        1
    } else {
        let half = lines_that_fit / 2;
        let start = vis_center.saturating_sub(half).max(1);
        start.min(total_lines.saturating_sub(lines_that_fit) + 1)
    };

    let relative_y = y - my;
    let clicked_line_offset = (relative_y / mlh).floor() as usize;
    let target_line = minimap_start + clicked_line_offset;
    let target_line = target_line.clamp(1, total_lines);

    this.call_method::<()>("scroll_to_line", (target_line, true, LuaValue::Nil))?;
    Ok(())
}

fn patch_on_mouse_pressed(lua: &Lua) -> LuaResult<()> {
    let doc_view = require_table(lua, "core.docview")?;
    let old: LuaFunction = doc_view.get("on_mouse_pressed")?;
    let old_key = lua.create_registry_value(old)?;

    doc_view.set(
        "on_mouse_pressed",
        lua.create_function(
            move |lua, (this, button, x, y, clicks): (LuaTable, LuaValue, f64, f64, LuaValue)| {
                if minimap_enabled(lua)?
                    && is_docview(lua, &this)?
                    && point_in_minimap(lua, &this, x, y)?
                {
                    this.set("minimap_dragging", true)?;
                    scroll_to_minimap_position(lua, &this, y)?;
                    return Ok(LuaValue::Boolean(true));
                }
                let old: LuaFunction = lua.registry_value(&old_key)?;
                old.call((this, button, x, y, clicks))
            },
        )?,
    )?;
    Ok(())
}

fn patch_on_mouse_moved(lua: &Lua) -> LuaResult<()> {
    let doc_view = require_table(lua, "core.docview")?;
    let old: LuaFunction = doc_view.get("on_mouse_moved")?;
    let old_key = lua.create_registry_value(old)?;

    doc_view.set(
        "on_mouse_moved",
        lua.create_function(
            move |lua, (this, x, y, dx, dy): (LuaTable, f64, f64, f64, f64)| {
                let dragging: bool = this.get("minimap_dragging").unwrap_or(false);
                if dragging && minimap_enabled(lua)? && is_docview(lua, &this)? {
                    scroll_to_minimap_position(lua, &this, y)?;
                    return Ok(());
                }
                let old: LuaFunction = lua.registry_value(&old_key)?;
                old.call((this, x, y, dx, dy))
            },
        )?,
    )?;
    Ok(())
}

fn patch_on_mouse_released(lua: &Lua) -> LuaResult<()> {
    let doc_view = require_table(lua, "core.docview")?;
    let old: LuaFunction = doc_view.get("on_mouse_released")?;
    let old_key = lua.create_registry_value(old)?;

    doc_view.set(
        "on_mouse_released",
        lua.create_function(
            move |lua, (this, button, x, y): (LuaTable, LuaValue, f64, f64)| -> LuaResult<()> {
                this.set("minimap_dragging", false)?;
                let old: LuaFunction = lua.registry_value(&old_key)?;
                old.call::<()>((this, button, x, y))
            },
        )?,
    )?;
    Ok(())
}

fn register_commands(lua: &Lua) -> LuaResult<()> {
    let command = require_table(lua, "core.command")?;
    let cmds = lua.create_table()?;
    cmds.set(
        "minimap:toggle",
        lua.create_function(|lua, ()| {
            let config = require_table(lua, "core.config")?;
            let plugins: LuaTable = config.get("plugins")?;
            let mm: LuaTable = plugins.get("minimap")?;
            let enabled: bool = mm.get("enabled").unwrap_or(false);
            mm.set("enabled", !enabled)?;
            let core = require_table(lua, "core")?;
            let log_fn: LuaFunction = core.get("log")?;
            if enabled {
                log_fn.call::<()>("Minimap disabled")?;
            } else {
                log_fn.call::<()>("Minimap enabled")?;
            }
            Ok(())
        })?,
    )?;
    command.call_function::<()>("add", (LuaValue::Nil, cmds))?;
    Ok(())
}

/// Registers `plugins.minimap`: config defaults, DocView draw/mouse hooks,
/// and `minimap:toggle` command.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.minimap",
        lua.create_function(|lua, ()| {
            set_config_defaults(lua)?;
            patch_draw(lua)?;
            patch_on_mouse_pressed(lua)?;
            patch_on_mouse_moved(lua)?;
            patch_on_mouse_released(lua)?;
            register_commands(lua)?;
            Ok(LuaValue::Boolean(true))
        })?,
    )
}
