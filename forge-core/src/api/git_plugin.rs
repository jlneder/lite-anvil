use mlua::prelude::*;

/// Embedded Lua for `plugins.git.status` — thin wrapper over `git_native` async state.
const STATUS_BOOTSTRAP: &str = r#"
local core   = require "core"
local config = require "core.config"
local common = require "core.common"
local git_native = require "git_native"

config.plugins.git = common.merge({
  refresh_interval        = 5,
  show_branch_in_statusbar = true,
  treeview_highlighting   = true,
}, config.plugins.git)

local repos = {}  -- root -> persistent Lua table updated in-place by sync_repo

local function sync_repo(root)
  local s = git_native.get_state(root)
  if not s then return end
  if not repos[root] then repos[root] = { root = root } end
  local r = repos[root]
  r.branch       = s.branch
  r.ahead        = s.ahead
  r.behind       = s.behind
  r.detached     = s.detached
  r.dirty        = s.dirty
  r.refreshing   = s.refreshing
  r.last_refresh = s.last_refresh
  r.error        = s.error
  r.ordered      = s.ordered
  r.files        = s.files
end

-- Background polling: drain Rust results and push them into the Lua repo tables.
core.add_thread(function()
  while true do
    local updated = git_native.poll_updates()
    if updated then
      for _, root in ipairs(updated) do sync_repo(root) end
      core.redraw = true
    end
    coroutine.yield(0.1)
  end
end)

local git = {}

function git.get_repo(path)
  local root = git_native.get_root(path)
  if not root then return nil end
  if not repos[root] then
    repos[root] = {
      root = root, branch = "", ahead = 0, behind = 0, detached = false,
      dirty = false, refreshing = false, last_refresh = 0, error = nil,
      ordered = {}, files = {},
    }
  end
  return repos[root]
end

function git.get_active_repo()
  local view = core.active_view
  local path
  if view and view.doc and view.doc.abs_filename then
    path = view.doc.abs_filename
  else
    local project = core.root_project and core.root_project()
    path = project and project.path or nil
  end
  return git.get_repo(path)
end

function git.get_file_status(path)
  return git_native.get_file_status(path)
end

function git.refresh(path, force)
  local root = git_native.get_root(path)
  if not root then return nil end
  git_native.maybe_refresh(root, force or false, config.plugins.git.refresh_interval or 5)
  return git.get_repo(root)
end

function git.run(path, args, on_complete)
  local root = git_native.get_root(path)
  if not root then
    if on_complete then on_complete(false, "", "Not inside a Git repository") end
    return
  end
  local handle = git_native.start_command(root, args)
  core.add_thread(function()
    while true do
      local result = git_native.check_command(handle)
      if result then
        local ok, stdout, stderr = result[1], result[2], result[3]
        if ok and args[1] ~= "branch" then git_native.start_refresh(root) end
        if on_complete then on_complete(ok, stdout, stderr, root) end
        core.redraw = true
        return
      end
      coroutine.yield(0.05)
    end
  end)
end

function git.list_branches(path, on_complete)
  git.run(path, { "branch", "--all", "--format=%(refname:short)" }, function(ok, stdout, stderr)
    if not ok then on_complete(nil, stderr); return end
    local branches, seen = {}, {}
    for line in stdout:gmatch("[^\r\n]+") do
      if line ~= "" and not seen[line] then seen[line] = true; branches[#branches + 1] = line end
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
  local rel   = entry and entry.rel or common.basename(path)
  local args  = { "diff" }
  if cached then args[#args + 1] = "--cached" end
  args[#args + 1] = "--"
  args[#args + 1] = rel
  git.run(path, args, on_complete)
end

function git.diff_repo(path, cached, on_complete)
  local args = { "diff" }
  if cached then args[#args + 1] = "--cached" end
  git.run(path, args, on_complete)
end

return git
"#;

/// Embedded Lua for `plugins.git.ui` — DiffView and StatusView.
const UI_BOOTSTRAP: &str = r#"
local core   = require "core"
local common = require "core.common"
local style  = require "core.style"
local View   = require "core.view"
local git    = require "plugins.git.status"

local DiffView   = View:extend()
local StatusView = View:extend()
local section_inset = 10

function DiffView:__tostring()   return "GitDiffView"   end
function StatusView:__tostring() return "GitStatusView" end

DiffView.context   = "session"
StatusView.context = "session"

local function active_repo_root()
  local repo = git.get_active_repo()
  return repo and repo.root or nil
end

local function open_diff(title, text)
  local view = DiffView(title, text)
  core.root_view:get_active_node_default():add_view(view)
  return view
end

function DiffView:new(title, text)
  DiffView.super.new(self)
  self.scrollable = true
  self.title = title
  self.lines = {}
  for line in (text .. "\n"):gmatch("([^\n]*)\n") do
    self.lines[#self.lines + 1] = line
  end
end

function DiffView:get_name()         return self.title end
function DiffView:get_line_height()  return style.code_font:get_height() + style.padding.y end
function DiffView:get_scrollable_size()
  return #self.lines * self:get_line_height() + style.padding.y * 2
end

function DiffView:draw()
  self:draw_background(style.background)
  local ox, oy = self:get_content_offset()
  local lh  = self:get_line_height()
  local min = math.max(1, math.floor(self.scroll.y / lh))
  local max = min + math.floor(self.size.y / lh) + 1
  local y   = oy + style.padding.y + lh * (min - 1)
  for i = min, max do
    local line = self.lines[i]
    if not line then break end
    local color = style.text
    if     line:match("^@@")                                       then color = style.accent
    elseif line:match("^%+") and not line:match("^%+%+")          then color = style.good or { 120, 220, 120, 255 }
    elseif line:match("^%-") and not line:match("^%-%-")          then color = style.error or { 220, 120, 120, 255 }
    elseif line:match("^diff ") or line:match("^index ")
        or line:match("^%-%-%- ")  or line:match("^%+%+%+ ")      then color = style.dim
    end
    common.draw_text(style.code_font, color, line, "left", ox + style.padding.x, y, self.size.x, lh)
    y = y + lh
  end
  self:draw_scrollbar()
end

function StatusView:new(root)
  StatusView.super.new(self)
  self.scrollable  = true
  self.repo_root   = root
  self.selected_idx = 1
  git.refresh(root, true)
end

function StatusView:get_repo()  return git.refresh(self.repo_root, false) or git.get_repo(self.repo_root) end
function StatusView:get_items() local r = self:get_repo(); return r and r.ordered or {} end

function StatusView:get_name()
  local r = self:get_repo()
  return "Git Status [" .. (r and r.branch or "git") .. "]"
end

function StatusView:get_line_height()   return style.font:get_height() + style.padding.y + 2 end
function StatusView:get_header_height() return style.font:get_height() * 2 + style.padding.y * 3 end
function StatusView:get_scrollable_size()
  return self:get_header_height() + #self:get_items() * self:get_line_height()
end

function StatusView:each_visible_item()
  return coroutine.wrap(function()
    local items = self:get_items()
    local lh    = self:get_line_height()
    local x, y  = self:get_content_offset()
    local min   = math.max(1, math.floor((self.scroll.y - style.font:get_height()) / lh))
    local max   = min + math.floor(self.size.y / lh) + 1
    y = y + self:get_header_height() + lh * (min - 1)
    for i = min, max do
      local item = items[i]
      if not item then break end
      local _, _, w = self:get_content_bounds()
      coroutine.yield(i, item, x, y, w, lh)
      y = y + lh
    end
  end)
end

function StatusView:scroll_to_selected()
  local y = (self.selected_idx - 1) * self:get_line_height()
  self.scroll.to.y = math.min(self.scroll.to.y, y)
  self.scroll.to.y = math.max(self.scroll.to.y,
    y + self:get_line_height() - self.size.y + self:get_header_height())
end

function StatusView:get_selected() return self:get_items()[self.selected_idx] end

function StatusView:open_selected()
  local item = self:get_selected()
  if not item then return end
  core.root_view:open_doc(core.open_doc(item.path))
end

function StatusView:draw()
  self:draw_background(style.background)
  local repo   = self:get_repo()
  local header = "No repository"
  local detail = ""
  if repo then
    if repo.error and repo.error ~= "" then
      header = "Git error: " .. repo.error
    else
      local summary = {}
      if repo.ahead  > 0 then summary[#summary + 1] = "ahead "  .. repo.ahead  end
      if repo.behind > 0 then summary[#summary + 1] = "behind " .. repo.behind end
      header = repo.branch ~= "" and repo.branch
            or (repo.refreshing and "Refreshing Git status..." or "(no branch)")
      detail = table.concat(summary, "  ")
      if repo.refreshing and repo.branch ~= "" then
        detail = detail ~= "" and (detail .. "  refreshing...") or "refreshing..."
      end
    end
  end

  local ox, oy   = self.position.x, self.position.y
  local header_h = self:get_header_height()
  renderer.draw_rect(ox, oy, self.size.x, header_h, style.background)
  renderer.draw_text(style.font, header, ox + section_inset, oy + style.padding.y, style.text)
  if detail ~= "" then
    renderer.draw_text(style.font, detail,
      ox + section_inset, oy + style.padding.y + style.font:get_height() + 2, style.dim)
  end
  renderer.draw_rect(
    ox + section_inset, oy + header_h - style.padding.y,
    self.size.x - section_inset * 2, style.divider_size, style.divider)

  local items = self:get_items()
  if repo and repo.error and repo.error ~= "" then
    renderer.draw_text(style.font, repo.error,
      ox + section_inset, oy + header_h + style.padding.y, style.error or style.text)
  elseif #items == 0 and repo and not repo.refreshing then
    renderer.draw_text(style.font, "Working tree clean",
      ox + section_inset, oy + header_h + style.padding.y, style.dim)
  end

  for i, item, x, y, w, h in self:each_visible_item() do
    if i == self.selected_idx then
      local hi = { table.unpack(style.line_highlight) }
      hi[4] = math.max(hi[4] or 0, 190)
      renderer.draw_rect(x, y, w, h, hi)
    elseif i % 2 == 0 then
      local stripe = { table.unpack(style.background2) }
      stripe[4] = 70
      renderer.draw_rect(x, y, w, h, stripe)
    end
    local code_color = style.dim
    if     item.kind == "staged"    then code_color = style.accent
    elseif item.kind == "changed"   then code_color = style.text
    elseif item.kind == "untracked" then code_color = style.good  or { 120, 220, 120, 255 }
    elseif item.kind == "conflict"  then code_color = style.error or { 220, 120, 120, 255 }
    end
    local rel = common.home_encode(item.rel)
    local dx  = x + section_inset
    common.draw_text(style.code_font, code_color, item.code, "center",
      dx, y, style.font:get_height() * 1.4, h)
    common.draw_text(style.font, style.text, rel, "left",
      dx + style.font:get_height() * 1.7, y, math.max(0, w - dx - section_inset), h)
  end
  self:draw_scrollbar()
end

local ui = {}

function ui.open_status(root)
  root = root or active_repo_root()
  if not root then core.error("Not inside a Git repository"); return nil end
  local view = StatusView(root)
  core.root_view:get_active_node_default():add_view(view)
  return view
end

function ui.open_repo_diff(root, cached)
  root = root or active_repo_root()
  if not root then core.error("Not inside a Git repository"); return end
  git.diff_repo(root, cached, function(ok, stdout, stderr)
    if not ok then core.error(stderr ~= "" and stderr or "git diff failed"); return end
    open_diff("Git Diff" .. (cached and " [staged]" or ""), stdout ~= "" and stdout or "No diff")
  end)
end

function ui.open_file_diff(path, cached)
  git.diff_file(path, cached, function(ok, stdout, stderr)
    if not ok then core.error(stderr ~= "" and stderr or "git diff failed"); return end
    open_diff(common.basename(path) .. ".diff", stdout ~= "" and stdout or "No diff")
  end)
end

ui.StatusView = StatusView
ui.DiffView   = DiffView
return ui
"#;

/// Embedded Lua for `plugins.git` — commands, keymaps, and TreeView integration.
const INIT_BOOTSTRAP: &str = r#"
local core          = require "core"
local command       = require "core.command"
local common        = require "core.common"
local config        = require "core.config"
local keymap        = require "core.keymap"
local style         = require "core.style"
local native_picker = require "picker"
local TreeView      = require "plugins.treeview"
local git           = require "plugins.git.status"
local ui            = require "plugins.git.ui"

local function active_path()
  local view = core.active_view
  if view and view.doc and view.doc.abs_filename then return view.doc.abs_filename end
  local project = core.root_project and core.root_project()
  return project and project.path or nil
end

local function show_git_result(ok, stdout, stderr, success)
  if ok then
    if success then core.status_view:show_message("i", style.text, success) end
  else
    core.error(stderr ~= "" and stderr or "Git command failed")
  end
end

local function with_selected_file(fn)
  local view = core.active_view
  if view and view.context == "session" and view.get_selected then
    local item = view:get_selected()
    if item then fn(item); return end
  end
  local path  = active_path()
  local entry = path and git.get_file_status(path)
  if entry then fn(entry) else core.error("No Git-tracked change selected") end
end

local function prompt_branch_checkout()
  git.list_branches(active_path(), function(branches, err)
    if not branches then core.error(err or "Unable to list branches"); return end
    core.command_view:enter("Checkout Branch", {
      suggest = function(text)
        if not text or text == "" then return branches end
        return native_picker.rank_strings(branches, text)
      end,
      submit = function(text, item)
        local branch = item and item.text or text
        if branch == "" then return end
        git.run(active_path(), { "checkout", branch }, function(ok, stdout, stderr)
          show_git_result(ok, stdout, stderr, "Checked out " .. branch)
        end)
      end,
    })
  end)
end

local function prompt_branch_create()
  core.command_view:enter("Create Branch", {
    submit = function(text)
      if text == "" then return end
      git.run(active_path(), { "checkout", "-b", text }, function(ok, stdout, stderr)
        show_git_result(ok, stdout, stderr, "Created branch " .. text)
      end)
    end,
  })
end

local function prompt_commit()
  core.command_view:enter("Commit Message", {
    submit = function(text)
      if text == "" then return end
      git.run(active_path(), { "commit", "-m", text }, function(ok, stdout, stderr)
        show_git_result(ok, stdout, stderr, "Committed changes")
      end)
    end,
  })
end

local function prompt_stash()
  core.command_view:enter("Stash Message (optional)", {
    submit = function(text)
      local args = { "stash", "push" }
      if text ~= "" then args[#args + 1] = "-m"; args[#args + 1] = text end
      git.run(active_path(), args, function(ok, stdout, stderr)
        show_git_result(ok, stdout, stderr, "Stashed changes")
      end)
    end,
  })
end

command.add(nil, {
  ["git:status"]  = function() ui.open_status() end,
  ["git:refresh"] = function() git.refresh(active_path(), true) end,
  ["git:commit"]  = function() prompt_commit() end,
  ["git:pull"] = function()
    git.run(active_path(), { "pull", "--ff-only" }, function(ok, stdout, stderr)
      show_git_result(ok, stdout, stderr, "Pulled latest changes")
    end)
  end,
  ["git:push"] = function()
    git.run(active_path(), { "push" }, function(ok, stdout, stderr)
      show_git_result(ok, stdout, stderr, "Pushed changes")
    end)
  end,
  ["git:checkout"]        = function() prompt_branch_checkout() end,
  ["git:branch"]          = function() prompt_branch_create() end,
  ["git:stash"]           = function() prompt_stash() end,
  ["git:diff-repo"]        = function() ui.open_repo_diff(nil, false) end,
  ["git:diff-repo-staged"] = function() ui.open_repo_diff(nil, true) end,
  ["git:diff-file"] = function()
    with_selected_file(function(item) ui.open_file_diff(item.path, item.kind == "staged") end)
  end,
  ["git:stage-file"] = function()
    with_selected_file(function(item)
      git.stage(item.path, function(ok, stdout, stderr)
        show_git_result(ok, stdout, stderr, "Staged " .. item.rel)
      end)
    end)
  end,
  ["git:unstage-file"] = function()
    with_selected_file(function(item)
      git.unstage(item.path, function(ok, stdout, stderr)
        show_git_result(ok, stdout, stderr, "Unstaged " .. item.rel)
      end)
    end)
  end,
})

command.add(ui.StatusView, {
  ["git:select-next"] = function()
    local v = core.active_view
    v.selected_idx = math.min(v.selected_idx + 1, #v:get_items())
    v:scroll_to_selected()
  end,
  ["git:select-previous"] = function()
    local v = core.active_view
    v.selected_idx = math.max(v.selected_idx - 1, 1)
    v:scroll_to_selected()
  end,
  ["git:open-selected"] = function() core.active_view:open_selected() end,
})

keymap.add {
  ["ctrl+shift+g"] = "git:status",
  ["return"]       = "git:open-selected",
  ["up"]           = "git:select-previous",
  ["down"]         = "git:select-next",
}

if not TreeView.__git_highlighting_patched then
  TreeView.__git_highlighting_patched = true
  local get_item_text = TreeView.get_item_text
  function TreeView:get_item_text(item, active, hovered)
    local text, font, color = get_item_text(self, item, active, hovered)
    if not active and not hovered and not item.ignored and item.type == "file"
        and config.plugins.git.treeview_highlighting ~= false then
      local entry = git.get_file_status(item.abs_filename)
      if entry then
        if     entry.kind == "staged"    then color = style.accent
        elseif entry.kind == "untracked" then color = style.good  or color
        elseif entry.kind == "conflict"  then color = style.error or color
        else                                  color = style.text
        end
      end
    end
    return text, font, color
  end
end

return { status = git, ui = ui }
"#;

/// Registers the three `plugins.git.*` preloads — all Lua logic embedded in Rust.
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "plugins.git.status",
        lua.create_function(|lua, ()| {
            lua.load(STATUS_BOOTSTRAP).set_name("plugins.git.status").eval::<LuaValue>()
        })?,
    )?;
    preload.set(
        "plugins.git.ui",
        lua.create_function(|lua, ()| {
            lua.load(UI_BOOTSTRAP).set_name("plugins.git.ui").eval::<LuaValue>()
        })?,
    )?;
    preload.set(
        "plugins.git",
        lua.create_function(|lua, ()| {
            lua.load(INIT_BOOTSTRAP).set_name("plugins.git").eval::<LuaValue>()
        })?,
    )
}
