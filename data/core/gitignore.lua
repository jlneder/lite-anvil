local common = require "core.common"
local config = require "core.config"

local gitignore = {
  cache = {},
}

local function normalize(path)
  return path and common.normalize_path(path):gsub("\\", "/") or nil
end

local function dirname(path)
  return path and path:match("^(.*)[/\\][^/\\]+$") or nil
end

local function parent_dir(path)
  local parent = dirname(path)
  if not parent or parent == path then
    return nil
  end
  return parent
end

local function path_exists(path)
  return system.get_file_info(path) ~= nil
end

local function glob_to_lua(glob)
  local i = 1
  local out = {}
  while i <= #glob do
    local ch = glob:sub(i, i)
    local next2 = glob:sub(i, i + 1)
    if next2 == "**" then
      out[#out + 1] = ".*"
      i = i + 2
    elseif ch == "*" then
      out[#out + 1] = "[^/]*"
      i = i + 1
    elseif ch == "?" then
      out[#out + 1] = "[^/]"
      i = i + 1
    else
      out[#out + 1] = ch:gsub("([%%%+%-%^%$%(%)%.%[%]%?])", "%%%1")
      i = i + 1
    end
  end
  return table.concat(out)
end

local function parse_rule(line, base_dir)
  if line == "" or line:match("^%s*#") then
    return nil
  end

  local negated = false
  if line:sub(1, 1) == "!" then
    negated = true
    line = line:sub(2)
  end

  local anchored = line:sub(1, 1) == "/"
  if anchored then
    line = line:sub(2)
  end

  local dir_only = line:sub(-1) == "/"
  if dir_only then
    line = line:sub(1, -2)
  end

  if line == "" then
    return nil
  end

  local has_slash = line:find("/", 1, true) ~= nil
  local prefix = anchored and "^" or (has_slash and "^(.-/)?"
    or "^([^/]+/)*")
  local pattern = prefix .. glob_to_lua(line)
  if dir_only then
    pattern = pattern .. "(/.*)?$"
  else
    pattern = pattern .. "$"
  end

  return {
    base_dir = normalize(base_dir),
    negated = negated,
    dir_only = dir_only,
    anchored = anchored,
    has_slash = has_slash,
    pattern = pattern,
    raw = line,
  }
end

local function load_rules(dir)
  dir = normalize(dir)
  if not dir then
    return {}
  end

  local ignore_path = dir .. "/.gitignore"
  local info = system.get_file_info(ignore_path)
  local modified = info and info.modified or false
  local cached = gitignore.cache[ignore_path]
  if cached and cached.modified == modified then
    return cached.rules
  end

  local rules = {}
  if info and info.type == "file" then
    local fp = io.open(ignore_path, "rb")
    if fp then
      for line in fp:lines() do
        local rule = parse_rule(line, dir)
        if rule then
          rules[#rules + 1] = rule
        end
      end
      fp:close()
    end
  end

  gitignore.cache[ignore_path] = {
    modified = modified,
    rules = rules,
  }
  return rules
end

function gitignore.find_root(start_path)
  local current = normalize(start_path)
  if not current then
    return nil
  end
  local info = system.get_file_info(current)
  if info and info.type == "file" then
    current = dirname(current)
  end
  while current do
    if path_exists(current .. "/.git") then
      return current
    end
    current = parent_dir(current)
  end
  return nil
end

local function collect_dirs(root, path)
  local dirs = {}
  local current = normalize(path)
  if not current then
    return dirs
  end
  local info = system.get_file_info(current)
  if info and info.type == "file" then
    current = dirname(current)
  end
  root = normalize(root)
  while current and common.path_belongs_to(current, root) do
    table.insert(dirs, 1, current)
    if current == root then
      break
    end
    current = parent_dir(current)
  end
  return dirs
end

function gitignore.match(root, path, info)
  if config.gitignore and config.gitignore.enabled == false then
    return false
  end

  root = normalize(root)
  path = normalize(path)
  if not root or not path or not common.path_belongs_to(path, root) then
    return false
  end

  local ignored = false
  local dirs = collect_dirs(root, path)
  for _, dir in ipairs(dirs) do
    local rel = common.relative_path(dir, path):gsub("\\", "/")
    for _, rule in ipairs(load_rules(dir)) do
      local target = rel
      if not rule.has_slash and not rule.anchored then
        target = common.basename(path)
      end
      if target and target:match(rule.pattern) then
        if not rule.dir_only or (info and info.type == "dir") then
          ignored = not rule.negated
        end
      end
    end
  end

  if config.gitignore and type(config.gitignore.additional_patterns) == "table" then
    local rel = common.relative_path(root, path)
    for _, pattern in ipairs(config.gitignore.additional_patterns) do
      if common.match_pattern(rel, pattern) or common.match_pattern(common.basename(path), pattern) then
        ignored = true
        break
      end
    end
  end

  return ignored
end

return gitignore
