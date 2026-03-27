use mlua::prelude::*;

use super::project_fs::{invalidate_shared_file_list, shared_file_list};

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "get_files",
        lua.create_function(|lua, (root, opts): (String, Option<LuaTable>)| {
            let max_size_bytes = if let Some(opts) = opts {
                opts.get::<Option<u64>>("max_size_bytes")?
            } else {
                None
            };
            let files_arc = shared_file_list(&root, max_size_bytes);
            let out = lua.create_table()?;
            let files = files_arc.lock();
            for (idx, file) in files.iter().enumerate() {
                out.raw_set((idx + 1) as i64, file.as_str())?;
            }
            Ok(out)
        })?,
    )?;

    module.set(
        "invalidate",
        lua.create_function(|_, root: String| Ok(invalidate_shared_file_list(&root)))?,
    )?;

    module.set(
        "clear_all",
        lua.create_function(|_, ()| {
            // The shared cache is root-scoped; callers invalidate per-root on close.
            Ok(true)
        })?,
    )?;

    Ok(module)
}
