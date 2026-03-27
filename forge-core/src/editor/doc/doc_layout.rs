use mlua::prelude::*;

fn char_width_at(column: usize, ch: char, tab_size: usize, cell_width: f64) -> f64 {
    if ch == '\t' {
        let tab_cols = tab_size.max(1);
        let offset = (column.saturating_sub(1)) % tab_cols;
        ((tab_cols - offset) as f64) * cell_width
    } else {
        cell_width
    }
}

fn col_x_offset(text: &str, col: usize, tab_size: usize, cell_width: f64) -> f64 {
    let mut x = 0.0;
    let mut column = 1usize;
    for ch in text.chars() {
        if column >= col {
            break;
        }
        x += char_width_at(column, ch, tab_size, cell_width);
        column += ch.len_utf8();
    }
    x
}

fn x_offset_col(text: &str, x: f64, tab_size: usize, cell_width: f64) -> usize {
    let mut xoffset = 0.0;
    let mut column = 1usize;
    for ch in text.chars() {
        let width = char_width_at(column, ch, tab_size, cell_width);
        if xoffset + width >= x {
            return if x <= xoffset + (width / 2.0) {
                column
            } else {
                column + ch.len_utf8()
            };
        }
        xoffset += width;
        column += ch.len_utf8();
    }
    text.len()
}

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "col_x_offset",
        lua.create_function(
            |_, (text, col, tab_size, cell_width): (String, usize, usize, f64)| {
                Ok(col_x_offset(&text, col, tab_size, cell_width))
            },
        )?,
    )?;
    module.set(
        "x_offset_col",
        lua.create_function(
            |_, (text, x, tab_size, cell_width): (String, f64, usize, f64)| {
                Ok(x_offset_col(&text, x, tab_size, cell_width) as i64)
            },
        )?,
    )?;
    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::{col_x_offset, x_offset_col};

    #[test]
    fn handles_tabs() {
        assert_eq!(col_x_offset("\tab", 2, 4, 8.0), 32.0);
        assert_eq!(x_offset_col("\tab", 16.0, 4, 8.0), 1);
        assert_eq!(x_offset_col("\tab", 24.0, 4, 8.0), 2);
    }
}
