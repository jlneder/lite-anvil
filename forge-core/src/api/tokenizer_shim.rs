use mlua::prelude::*;

/// Minimal shim for `core.tokenizer`.
///
/// The full 415-line tokenizer.lua is never called at runtime — the native
/// tokenizer handles all tokenization. Only `each_token` is needed by
/// `core.doc.highlighter`. Replaces `data/core/tokenizer.lua`.
const BOOTSTRAP: &str = r#"
local tokenizer = {}

local function iter(t, i)
  i = i + 2
  local token_type, text = t[i], t[i + 1]
  if token_type then return i, token_type, text end
end

function tokenizer.each_token(t)
  return iter, t, -1
end

return tokenizer
"#;

/// Register `core.tokenizer` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.tokenizer",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.tokenizer").eval::<LuaValue>()
        })?,
    )
}
