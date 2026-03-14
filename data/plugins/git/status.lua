local core = require "core"
local common = require "core.common"
local config = require "core.config"
local gitignore = require "core.gitignore"
local process = require "process"

config.plugins.git = common.merge({
  refresh_interval = 5,
  show_branch_in_statusbar = true,
  treeview_highlighting = true,
}, config.plugins.git)

local git = {
  repos = {},
}

local function normalize(path)
  return path and common.normalize_path(path):gsub("\\", "/") or nil
end

local function dirname(path)
  return path and path:match("^(.*)[/\\][^/\\]+$") or nil
end

local function path_to_repo(path)
  if not path then
    local project = core.root_project and core.root_project()
    path = project and project.path or nil
  end
  return path and gitignore.find_root(path) or nil
end

local function active_path()
  local view = core.active_view
  if view and view.doc and view.doc.abs_filename then
    return view.doc.abs_filename
  end
  local project = core.root_project and core.root_project()
  return project and project.path or nil
end

local function repo_state(root)
  root = normalize(root)
  if not root then
    return nil
  end
  local repo = git.repos[root]
  if not repo then
    repo = {
      root = root,
      branch = "",
      ahead = 0,
      behind = 0,
      detached = false,
      files = {},
      ordered = {},
      dirty = false,
      refreshing = false,
      last_refresh = 0,
      error = nil,
    }
    git.repos[root] = repo
  end
  return repo
end

local function read_process_output(proc)
  local stdout, stderr = {}, {}
  while proc:running() do
    local out = proc.stdout:read(4096)
    if out and #out > 0 then
      stdout[#stdout + 1] = out
    end
    local err = proc.stderr:read(4096)
    if err and #err > 0 then
      stderr[#stderr + 1] = err
    end
    coroutine.yield(1 / config.fps)
  end
  local out = proc.stdout:read("all")
  if out and #out > 0 then
    stdout[#stdout + 1] = out
  end
  local err = proc.stderr:read("all")
  if err and #err > 0 then
    stderr[#stderr + 1] = err
  end
  return proc:returncode() or proc:wait(), table.concat(stdout), table.concat(stderr)
end

local function run_git(root, args, on_complete)
  core.add_thread(function()
    local cmd = { "git", "-C", root }
    for _, arg in ipairs(args) do
      cmd[#cmd + 1] = arg
    end
    local ok, proc = pcall(process.start, cmd, { cwd = root })
    if not ok then
      on_complete(false, "", tostring(proc))
      return
    end
    local code, stdout, stderr = read_process_output(proc)
    on_complete(code == 0, stdout or "", stderr or "")
  end)
end

local function parse_branch(repo, line)
  local head = line:sub(4)
  repo.ahead = tonumber(head:match("ahead (%d+)")) or 0
  repo.behind = tonumber(head:match("behind (%d+)")) or 0
  repo.detached = head:match("^HEAD") ~= nil
  repo.branch = head
    :gsub("%s+%[.*$", "")
    :gsub("%.%.+.*$", "")
  if repo.branch == "HEAD (no branch)" or repo.branch:match("^HEAD %(detached") then
    repo.branch = "detached"
    repo.detached = true
  end
end

local function classify_status(entry)
  if entry.code == "??" then
    entry.kind = "untracked"
  elseif entry.index == "U" or entry.worktree == "U" then
    entry.kind = "conflict"
  elseif entry.index ~= " " and entry.index ~= "?" then
    entry.kind = "staged"
  elseif entry.worktree ~= " " then
    entry.kind = "changed"
  else
    entry.kind = "unknown"
  end
end

local function parse_status(repo, stdout)
  repo.files = {}
  repo.ordered = {}
  repo.dirty = false
  repo.error = nil

  for line in stdout:gmatch("[^\r\n]+") do
    if line:sub(1, 2) == "##" then
      parse_branch(repo, line)
    elseif line:sub(1, 2) ~= "!!" and #line >= 4 then
      local rel = line:sub(4)
      local old_rel, renamed_rel = rel:match("^(.-) %-%> (.+)$")
      rel = renamed_rel or rel
      local abs = normalize(repo.root .. "/" .. rel)
      local entry = {
        root = repo.root,
        rel = rel,
        path = abs,
        old_rel = old_rel,
        index = line:sub(1, 1),
        worktree = line:sub(2, 2),
        code = line:sub(1, 2),
      }
      classify_status(entry)
      repo.files[abs] = entry
      repo.ordered[#repo.ordered + 1] = entry
      repo.dirty = true
    end
  end

  table.sort(repo.ordered, function(a, b)
    if a.kind ~= b.kind then
      return a.kind < b.kind
    end
    return a.rel < b.rel
  end)
end

function git.get_repo(path)
  local root = path_to_repo(path)
  return root and repo_state(root) or nil
end

function git.get_active_repo()
  return git.get_repo(active_path())
end

function git.get_file_status(path)
  local repo = git.get_repo(path)
  if not repo then
    return nil
  end
  return repo.files[normalize(path)]
end

function git.refresh(path, force)
  local root = path_to_repo(path)
  if not root then
    return nil
  end
  local repo = repo_state(root)
  if repo.refreshing then
    return repo
  end
  local interval = config.plugins.git.refresh_interval or 5
  if not force and repo.last_refresh > 0 and (system.get_time() - repo.last_refresh) < interval then
    return repo
  end

  repo.refreshing = true
  run_git(root, { "status", "--branch", "--porcelain=v1" }, function(ok, stdout, stderr)
    repo.refreshing = false
    repo.last_refresh = system.get_time()
    if ok then
      parse_status(repo, stdout)
    else
      repo.error = stderr ~= "" and stderr or "git status failed"
    end
    core.redraw = true
  end)
  return repo
end

function git.run(path, args, on_complete)
  local root = path_to_repo(path)
  if not root then
    if on_complete then
      on_complete(false, "", "Not inside a Git repository")
    end
    return
  end
  run_git(root, args, function(ok, stdout, stderr)
    if ok and args[1] ~= "branch" then
      git.refresh(root, true)
    end
    if on_complete then
      on_complete(ok, stdout, stderr, root)
    end
  end)
end

function git.list_branches(path, on_complete)
  git.run(path, { "branch", "--all", "--format=%(refname:short)" }, function(ok, stdout, stderr)
    if not ok then
      on_complete(nil, stderr)
      return
    end
    local branches, seen = {}, {}
    for line in stdout:gmatch("[^\r\n]+") do
      if line ~= "" and not seen[line] then
        seen[line] = true
        branches[#branches + 1] = line
      end
    end
    table.sort(branches)
    on_complete(branches)
  end)
end

function git.stage(path, on_complete)
  local entry = git.get_file_status(path)
  local rel = entry and entry.rel or common.basename(path)
  git.run(path, { "add", "--", rel }, on_complete)
end

function git.unstage(path, on_complete)
  local entry = git.get_file_status(path)
  local rel = entry and entry.rel or common.basename(path)
  git.run(path, { "reset", "HEAD", "--", rel }, on_complete)
end

function git.diff_file(path, cached, on_complete)
  local entry = git.get_file_status(path)
  local rel = entry and entry.rel or common.basename(path)
  local args = { "diff" }
  if cached then
    args[#args + 1] = "--cached"
  end
  args[#args + 1] = "--"
  args[#args + 1] = rel
  git.run(path, args, on_complete)
end

function git.diff_repo(path, cached, on_complete)
  local args = { "diff" }
  if cached then
    args[#args + 1] = "--cached"
  end
  git.run(path, args, on_complete)
end

return git
