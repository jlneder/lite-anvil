local core = require "core"
local common = require "core.common"
local style = require "core.style"
local View = require "core.view"
local git = require "..status"

local DiffView = View:extend()
local StatusView = View:extend()
local section_inset = 10

function DiffView:__tostring() return "GitDiffView" end
function StatusView:__tostring() return "GitStatusView" end

DiffView.context = "session"
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

function DiffView:get_name()
  return self.title
end

function DiffView:get_line_height()
  return style.code_font:get_height() + style.padding.y
end

function DiffView:get_scrollable_size()
  return #self.lines * self:get_line_height() + style.padding.y * 2
end

function DiffView:draw()
  self:draw_background(style.background)
  local ox, oy = self:get_content_offset()
  local lh = self:get_line_height()
  local min = math.max(1, math.floor(self.scroll.y / lh))
  local max = min + math.floor(self.size.y / lh) + 1
  local y = oy + style.padding.y + lh * (min - 1)
  for i = min, max do
    local line = self.lines[i]
    if not line then
      break
    end
    local color = style.text
    if line:match("^@@") then
      color = style.accent
    elseif line:match("^%+") and not line:match("^%+%+") then
      color = style.good or { 120, 220, 120, 255 }
    elseif line:match("^%-") and not line:match("^%-%-") then
      color = style.error or { 220, 120, 120, 255 }
    elseif line:match("^diff ") or line:match("^index ") or line:match("^%-%-%- ") or line:match("^%+%+%+ ") then
      color = style.dim
    end
    common.draw_text(style.code_font, color, line, "left", ox + style.padding.x, y, self.size.x, lh)
    y = y + lh
  end
  self:draw_scrollbar()
end

function StatusView:new(root)
  StatusView.super.new(self)
  self.scrollable = true
  self.repo_root = root
  self.selected_idx = 1
  git.refresh(root, true)
end

function StatusView:get_repo()
  return git.refresh(self.repo_root, false) or git.get_repo(self.repo_root)
end

function StatusView:get_items()
  local repo = self:get_repo()
  return repo and repo.ordered or {}
end

function StatusView:get_name()
  local repo = self:get_repo()
  local branch = repo and repo.branch or "git"
  return "Git Status [" .. branch .. "]"
end

function StatusView:get_line_height()
  return style.font:get_height() + style.padding.y + 2
end

function StatusView:get_header_height()
  return style.font:get_height() * 2 + style.padding.y * 3
end

function StatusView:get_scrollable_size()
  return self:get_header_height() + #self:get_items() * self:get_line_height()
end

function StatusView:each_visible_item()
  return coroutine.wrap(function()
    local items = self:get_items()
    local lh = self:get_line_height()
    local x, y = self:get_content_offset()
    local min = math.max(1, math.floor((self.scroll.y - style.font:get_height()) / lh))
    local max = min + math.floor(self.size.y / lh) + 1
    y = y + self:get_header_height() + lh * (min - 1)
    for i = min, max do
      local item = items[i]
      if not item then
        break
      end
      local _, _, w = self:get_content_bounds()
      coroutine.yield(i, item, x, y, w, lh)
      y = y + lh
    end
  end)
end

function StatusView:scroll_to_selected()
  local y = (self.selected_idx - 1) * self:get_line_height()
  self.scroll.to.y = math.min(self.scroll.to.y, y)
  self.scroll.to.y = math.max(self.scroll.to.y, y + self:get_line_height() - self.size.y + self:get_header_height())
end

function StatusView:get_selected()
  return self:get_items()[self.selected_idx]
end

function StatusView:open_selected()
  local item = self:get_selected()
  if not item then
    return
  end
  core.root_view:open_doc(core.open_doc(item.path))
end

function StatusView:draw()
  self:draw_background(style.background)
  local repo = self:get_repo()
  local header = "No repository"
  local detail = ""
  if repo then
    if repo.error and repo.error ~= "" then
      header = "Git error: " .. repo.error
    else
      local summary = {}
      if repo.ahead > 0 then summary[#summary + 1] = "ahead " .. repo.ahead end
      if repo.behind > 0 then summary[#summary + 1] = "behind " .. repo.behind end
      local branch = repo.branch ~= "" and repo.branch or (repo.refreshing and "Refreshing Git status..." or "(no branch)")
      header = branch
      detail = table.concat(summary, "  ")
      if repo.refreshing and repo.branch ~= "" then
        detail = detail ~= "" and (detail .. "  refreshing…") or "refreshing…"
      end
    end
  end

  local ox, oy = self.position.x, self.position.y
  local header_h = self:get_header_height()
  renderer.draw_rect(ox, oy, self.size.x, header_h, style.background)
  renderer.draw_text(style.font, header, ox + section_inset, oy + style.padding.y, style.text)
  if detail ~= "" then
    renderer.draw_text(style.font, detail, ox + section_inset, oy + style.padding.y + style.font:get_height() + 2, style.dim)
  end
  renderer.draw_rect(
    ox + section_inset,
    oy + header_h - style.padding.y,
    self.size.x - section_inset * 2,
    style.divider_size,
    style.divider
  )

  local items = self:get_items()
  if repo and repo.error and repo.error ~= "" then
    renderer.draw_text(style.font, repo.error, ox + section_inset, oy + header_h + style.padding.y, style.error or style.text)
  elseif #items == 0 and repo and not repo.refreshing then
    renderer.draw_text(style.font, "Working tree clean", ox + section_inset, oy + header_h + style.padding.y, style.dim)
  end

  for i, item, x, y, w, h in self:each_visible_item() do
    if i == self.selected_idx then
      local highlight = { table.unpack(style.line_highlight) }
      highlight[4] = math.max(highlight[4] or 0, 190)
      renderer.draw_rect(x, y, w, h, highlight)
    elseif i % 2 == 0 then
      local stripe = { table.unpack(style.background2) }
      stripe[4] = 70
      renderer.draw_rect(x, y, w, h, stripe)
    end
    local code_color = style.dim
    if item.kind == "staged" then
      code_color = style.accent
    elseif item.kind == "changed" then
      code_color = style.text
    elseif item.kind == "untracked" then
      code_color = style.good or { 120, 220, 120, 255 }
    elseif item.kind == "conflict" then
      code_color = style.error or { 220, 120, 120, 255 }
    end
    local rel = common.home_encode(item.rel)
    local dx = x + section_inset
    common.draw_text(style.code_font, code_color, item.code, "center", dx, y, style.font:get_height() * 1.4, h)
    common.draw_text(style.font, style.text, rel, "left", dx + style.font:get_height() * 1.7, y, math.max(0, w - dx - section_inset), h)
  end
  self:draw_scrollbar()
end

local ui = {}

function ui.open_status(root)
  root = root or active_repo_root()
  if not root then
    core.error("Not inside a Git repository")
    return nil
  end
  local view = StatusView(root)
  core.root_view:get_active_node_default():add_view(view)
  return view
end

function ui.open_repo_diff(root, cached)
  root = root or active_repo_root()
  if not root then
    core.error("Not inside a Git repository")
    return
  end
  git.diff_repo(root, cached, function(ok, stdout, stderr)
    if not ok then
      core.error(stderr ~= "" and stderr or "git diff failed")
      return
    end
    open_diff("Git Diff" .. (cached and " [staged]" or ""), stdout ~= "" and stdout or "No diff")
  end)
end

function ui.open_file_diff(path, cached)
  git.diff_file(path, cached, function(ok, stdout, stderr)
    if not ok then
      core.error(stderr ~= "" and stderr or "git diff failed")
      return
    end
    open_diff(common.basename(path) .. ".diff", stdout ~= "" and stdout or "No diff")
  end)
end

ui.StatusView = StatusView
ui.DiffView = DiffView

return ui
