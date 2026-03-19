use mlua::prelude::*;

/// Embedded Lua bootstrap for `core.syntax`.
///
/// JSON asset loading is delegated to `native_tokenizer.load_assets()`.
/// Replaces `data/core/syntax.lua` which is no longer read from disk.
const BOOTSTRAP: &str = r##"
local common = require "core.common"
local core   = require "core"
local native_tokenizer = require "native_tokenizer"

local syntax = {}
syntax.items = {}
syntax.lazy_items = {}
syntax.lazy_loaded = {}
syntax.loaded_assets = {}

syntax.plain_text_syntax = { name = "Plain Text", patterns = {}, symbols = {} }

if native_tokenizer.register_syntax then
  pcall(native_tokenizer.register_syntax, "Plain Text", syntax.plain_text_syntax)
end

local function check_pattern(pattern_type, pattern)
  local ok, err, mstart, mend
  if pattern_type == "regex" then
    ok, err = regex.compile(pattern)
    if ok then
      mstart, mend = regex.find_offsets(ok, "")
      if mstart and mstart > mend then
        ok, err = false, "Regex matches an empty string"
      end
    end
  else
    ok, mstart, mend = pcall(string.ufind, "", pattern)
    if ok and mstart and mstart > mend then
      ok, err = false, "Pattern matches an empty string"
    elseif not ok then
      err = mstart
    end
  end
  return ok, err
end

function syntax.add(t)
  if type(t.space_handling) ~= "boolean" then t.space_handling = true end

  if t.patterns then
    for i, pattern in ipairs(t.patterns) do
      local p, ok, err, name = pattern.pattern or pattern.regex, nil, nil, nil
      if type(p) == "table" then
        for j = 1, 2 do
          ok, err = check_pattern(pattern.pattern and "pattern" or "regex", p[j])
          if not ok then name = string.format("#%d:%d <%s>", i, j, p[j]) end
        end
      elseif type(p) == "string" then
        ok, err = check_pattern(pattern.pattern and "pattern" or "regex", p)
        if not ok then name = string.format("#%d <%s>", i, p) end
      else
        ok, err, name = false, "Missing pattern or regex", "#"..i
      end
      if not ok then
        pattern.disabled = true
        core.warn("Malformed pattern %s in %s language plugin: %s", name, t.name, err)
      end
    end

    if t.space_handling then
      table.insert(t.patterns, { pattern = "%s+", type = "normal" })
    end
    table.insert(t.patterns, { pattern = "%w+%f[%s]", type = "normal" })
  end

  table.insert(syntax.items, t)

  if native_tokenizer.available and t.name then
    local ok, err = pcall(native_tokenizer.register_syntax, t.name, t)
    if not ok then
      core.warn("Failed to register %s with native tokenizer: %s", t.name, err)
    end
  end
end

local function find(str, field)
  local best_match = 0
  local best_syntax
  for i = #syntax.items, 1, -1 do
    local t = syntax.items[i]
    local s, e = common.match_pattern(str, t[field] or {})
    if s and e - s > best_match then
      best_match = e - s
      best_syntax = t
    end
  end
  return best_syntax
end

local function extract_match_list(source, field)
  local list = {}
  local block = source:match(field .. "%s*=%s*%b{}")
  if not block then return list end
  for _, text in block:gmatch("(['\"])(.-)%1") do
    list[#list + 1] = text
  end
  return list
end

local function should_load_lazy_plugin(entry, filename, header)
  return (filename and common.match_pattern(filename, entry.files))
      or (header and common.match_pattern(header, entry.headers))
end

local function load_lazy_plugin(entry)
  if syntax.lazy_loaded[entry.name] then return true end
  syntax.lazy_loaded[entry.name] = true
  local ok, res = core.try(entry.load, entry.plugin)
  if ok then return res end
  return nil
end

function syntax.register_lazy_plugin(plugin)
  local json = package.loaded["plugins.lsp.json"]
  local files = {}
  local headers = {}
  local metadata_path = plugin.file:gsub("%.lua$", ".lazy.json")
  local mfile = io.open(metadata_path, "r")
  local metadata = mfile and mfile:read("*a")
  if mfile then mfile:close() end
  if metadata and json and json.decode_safe then
    local ok, decoded = json.decode_safe(metadata)
    if ok and type(decoded) == "table" then
      files = decoded.files or {}
      headers = decoded.headers or {}
    end
  end

  if #files == 0 and #headers == 0 then
    local src_file = io.open(plugin.file, "r")
    if not src_file then return end
    local source = src_file:read("*a")
    src_file:close()
    files = extract_match_list(source, "files")
    headers = extract_match_list(source, "headers")
  end

  syntax.lazy_items[#syntax.lazy_items + 1] = {
    name = plugin.name,
    plugin = plugin,
    load = plugin.load,
    files = files,
    headers = headers,
  }
end

function syntax.get(filename, header)
  local loaded = (filename and find(filename, "files"))
      or (header and find(header, "headers"))
  if loaded then return loaded end

  for i = #syntax.lazy_items, 1, -1 do
    local entry = syntax.lazy_items[i]
    if should_load_lazy_plugin(entry, filename, header) then
      table.remove(syntax.lazy_items, i)
      load_lazy_plugin(entry)
      local lazy_loaded = (filename and find(filename, "files"))
          or (header and find(header, "headers"))
      if lazy_loaded then return lazy_loaded end
    end
  end

  return syntax.plain_text_syntax
end

-- Eagerly load all builtin JSON syntax assets via Rust.
if native_tokenizer.load_assets then
  local assets = native_tokenizer.load_assets(DATADIR)
  for _, t in ipairs(assets) do
    syntax.add(t)
  end
end

-- Compatibility stub: builtin assets are always pre-loaded.
function syntax.add_from_asset(_asset)
  return true
end

return syntax
"##;

/// Register `core.syntax` as a Rust-owned preload.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.syntax",
        lua.create_function(|lua, ()| {
            lua.load(BOOTSTRAP).set_name("core.syntax").eval::<LuaValue>()
        })?,
    )
}
