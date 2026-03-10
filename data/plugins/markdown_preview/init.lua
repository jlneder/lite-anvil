-- mod-version:4
-- Markdown preview pane with clickable links and table support.
-- Toggle with Ctrl+Shift+M when a .md file is active.

local core      = require "core"
local common    = require "core.common"
local config    = require "core.config"
local style     = require "core.style"
local keymap    = require "core.keymap"
local command   = require "core.command"
local View      = require "core.view"
local DocView   = require "core.docview"
local markdown  = require "markdown"
local layout    = require "plugins.markdown_preview.layout"
local renderers = require "plugins.markdown_preview.renderers"

-- ── Config ────────────────────────────────────────────────────────────────────

config.plugins.markdown_preview = common.merge({
  padding   = 16,  -- horizontal/vertical padding (before SCALE)
  block_gap = 8,   -- vertical gap between blocks (before SCALE)
}, config.plugins.markdown_preview)

-- ── Font cache ────────────────────────────────────────────────────────────────

local HEAD_SCALE = { 2.0, 1.6, 1.3, 1.1, 1.0, 0.9 }
local fonts_cache, fonts_base_size

local function get_fonts()
  local sz = style.font:get_size()
  if not fonts_cache or fonts_base_size ~= sz then
    local f = style.font
    fonts_cache = { body = f, code = style.code_font }
    for i, scale in ipairs(HEAD_SCALE) do
      fonts_cache["h" .. i] = f:copy(math.floor(sz * scale + 0.5))
    end
    fonts_base_size = sz
  end
  return fonts_cache
end

-- ── URL opener ───────────────────────────────────────────────────────────────

local function open_url(href)
  -- Escape the URL for use in a shell command.
  local escaped = href:gsub("'", "'\\''")
  if system.get_file_info("/usr/bin/open") or system.get_file_info("/bin/open") then
    system.exec("open '" .. escaped .. "'")
  else
    system.exec("xdg-open '" .. escaped .. "'")
  end
end

-- ── MarkdownView ──────────────────────────────────────────────────────────────

local MarkdownView = View:extend()

function MarkdownView:new(doc)
  View.new(self)
  self.doc              = doc
  self.scrollable       = true
  self.cursor           = "arrow"
  self.blocks           = nil
  self.layout           = nil
  self.content_height   = 0
  self.link_regions     = {}
  self.last_change_id   = nil
  self.last_layout_width = nil
end

function MarkdownView:get_name()
  local base = self.doc.filename and common.basename(self.doc.filename) or "Untitled"
  return "Preview: " .. base
end

function MarkdownView:get_scrollable_size()
  return self.content_height
end

function MarkdownView:on_scale_change()
  fonts_cache = nil  -- force rebuild at new scale
  self.layout = nil
end

function MarkdownView:update()
  View.update(self)
  if self.doc:get_change_id() ~= self.last_change_id then
    self.last_change_id = self.doc:get_change_id()
    local text = self.doc:get_text(1, 1, math.huge, math.huge)
    self.blocks = markdown.parse(text)
    self.layout = nil
  end
  if self.blocks and (not self.layout or self.last_layout_width ~= self.size.x) then
    self.last_layout_width = self.size.x
    local f   = get_fonts()
    local pad = math.floor(config.plugins.markdown_preview.padding   * SCALE)
    local gap = math.floor(config.plugins.markdown_preview.block_gap * SCALE)
    layout.compute(self, f, pad, gap)
  end
end

-- ── Draw ──────────────────────────────────────────────────────────────────────

function MarkdownView:draw()
  if not self.layout then return end
  self:draw_background(style.background)
  self.link_regions = {}

  local f      = get_fonts()
  local lh     = f.body:get_height()
  local pad    = math.floor(config.plugins.markdown_preview.padding   * SCALE)
  local gap    = math.floor(config.plugins.markdown_preview.block_gap * SCALE)
  local x      = self.position.x + pad
  local max_x  = self.position.x + self.size.x - pad
  local base_y = self.position.y - self.scroll.y

  core.push_clip_rect(self.position.x, self.position.y, self.size.x, self.size.y)
  for i, blk in ipairs(self.blocks) do
    local entry = self.layout[i]
    local sy    = base_y + entry.y
    if sy + entry.h < self.position.y then goto continue end
    if sy > self.position.y + self.size.y then break end
    renderers.draw_block(self, blk, x, sy, max_x, f, lh, gap)
    ::continue::
  end
  core.pop_clip_rect()
  self:draw_scrollbar()
end

-- ── Mouse ─────────────────────────────────────────────────────────────────────

function MarkdownView:on_mouse_pressed(button, x, y, clicks)
  if View.on_mouse_pressed(self, button, x, y, clicks) then return true end
  if button ~= "left" then return end
  for _, r in ipairs(self.link_regions) do
    if x >= r.x1 and x <= r.x2 and y >= r.y1 and y <= r.y2 then
      open_url(r.href)
      return true
    end
  end
end

function MarkdownView:on_mouse_moved(x, y, dx, dy)
  View.on_mouse_moved(self, x, y, dx, dy)
  for _, r in ipairs(self.link_regions) do
    if x >= r.x1 and x <= r.x2 and y >= r.y1 and y <= r.y2 then
      self.cursor = "hand"
      return
    end
  end
  self.cursor = "arrow"
end

function MarkdownView:on_mouse_left()
  View.on_mouse_left(self)
  self.cursor = "arrow"
end

function MarkdownView:on_mouse_wheel(dy, dx)
  local lh = get_fonts().body:get_height()
  self.scroll.to.y = self.scroll.to.y - dy * lh * 3
  return true
end

-- ── Toggle command ────────────────────────────────────────────────────────────

local function find_preview(doc)
  for _, view in ipairs(core.root_view.root_node:get_children()) do
    if view:is(MarkdownView) and view.doc == doc then
      return view, core.root_view.root_node:get_node_for_view(view)
    end
  end
end

command.add("core.docview", {
  ["markdown-preview:toggle"] = function()
    local dv = core.active_view
    local fname = dv.doc and dv.doc.filename
    if not fname or not fname:match("%.md$") and not fname:match("%.markdown$") then
      core.warn("markdown-preview: active file is not a markdown document")
      return
    end
    local existing, node = find_preview(dv.doc)
    if existing then
      node:close_view(core.root_view.root_node, existing)
    else
      local src_node = core.root_view.root_node:get_node_for_view(dv)
      src_node:split("right", MarkdownView(dv.doc))
    end
  end,
})

keymap.add({ ["ctrl+shift+m"] = "markdown-preview:toggle" })
