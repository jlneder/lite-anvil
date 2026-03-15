-- mod-version:4.0.0
local core = require "core"
local common = require "core.common"
local config = require "core.config"
local keymap = require "core.keymap"
local Doc = require "core.doc"
local DocView = require "core.docview"
local style = require "core.style"

config.plugins.lsp = common.merge({
  config_spec = {
    name = "LSP",
    {
      label = "Load On Startup",
      description = "Load the LSP plugin during editor startup.",
      path = "load_on_startup",
      type = "toggle",
      default = true,
    },
    {
      label = "Semantic Highlighting",
      description = "Apply semantic token overlays from LSP servers.",
      path = "semantic_highlighting",
      type = "toggle",
      default = true,
    },
    {
      label = "Inline Diagnostics",
      description = "Render LSP diagnostics in the editor gutter and text area.",
      path = "inline_diagnostics",
      type = "toggle",
      default = true,
    },
  },
  load_on_startup = config.lsp.load_on_startup ~= false,
  semantic_highlighting = config.lsp.semantic_highlighting ~= false,
  inline_diagnostics = config.lsp.inline_diagnostics ~= false,
}, config.plugins.lsp)

local manager = require ".server-manager"

manager.reload_config()
manager.start_semantic_refresh_loop()

local old_open_doc = core.open_doc
function core.open_doc(filename)
  local doc = old_open_doc(filename)
  if doc and doc.abs_filename and not doc.large_file_mode then
    manager.open_doc(doc)
  end
  return doc
end

local old_on_text_change = Doc.on_text_change
function Doc:on_text_change(change_type)
  old_on_text_change(self, change_type)
  if self.abs_filename and not self.large_file_mode then
    manager.on_doc_change(self)
  end
end

local RootView = require "core.rootview"
local old_on_text_input = RootView.on_text_input
RootView.on_text_input = function(self, text, ...)
  old_on_text_input(self, text, ...)
  manager.maybe_trigger_signature_help(text)
end

local function diagnostic_color(severity)
  if severity == 1 then
    return style.lint.error or style.error
  elseif severity == 2 then
    return style.lint.warning or style.warn
  elseif severity == 3 then
    return style.lint.info or style.accent
  end
  return style.lint.hint or style.good or style.accent
end

local old_draw_line_gutter = DocView.draw_line_gutter
function DocView:draw_line_gutter(line, x, y, width)
  local lh = old_draw_line_gutter(self, line, x, y, width)
  if config.plugins.lsp.inline_diagnostics == false or not self.doc.abs_filename then
    return lh
  end
  if self.doc.large_file_mode then
    return lh
  end

  local severity = manager.get_line_diagnostic_severity(self.doc, line)
  if severity then
    local marker_size = math.max(4, math.floor(self:get_line_height() * 0.22))
    local marker_x = x + math.max(2, style.padding.x - marker_size - 2)
    local marker_y = y + math.floor((self:get_line_height() - marker_size) / 2)
    renderer.draw_rect(marker_x, marker_y, marker_size, marker_size, diagnostic_color(severity))
  end

  return lh
end

local old_draw_overlay = DocView.draw_overlay
function DocView:draw_overlay()
  old_draw_overlay(self)
  if config.plugins.lsp.inline_diagnostics == false or not self.doc.abs_filename then
    return
  end
  if self.doc.large_file_mode then
    return
  end

  local minline, maxline = self:get_visible_line_range()
  local line_size = math.max(1, style.caret_width)
  for line = minline, maxline do
    local segments = manager.get_line_diagnostic_segments(self.doc, line)
    if segments then
      local _, y = self:get_line_screen_position(line)
      local lh = self:get_line_height()
      for i = 1, #segments do
        local segment = segments[i]
        local start_x = self:get_line_screen_position(line, segment.col1)
        local end_x = self:get_line_screen_position(line, segment.col2)
        local width = math.max(math.abs(end_x - start_x), math.max(2, style.caret_width * 2))
        renderer.draw_rect(
          math.min(start_x, end_x),
          y + lh - line_size,
          width,
          line_size,
          diagnostic_color(segment.severity)
        )
      end
    end
  end
end

local old_on_close = Doc.on_close
function Doc:on_close()
  if not self.large_file_mode then
    manager.on_doc_close(self)
  end
  old_on_close(self)
end

local old_save = Doc.save
function Doc:save(...)
  local result = table.pack(old_save(self, ...))
  if not self.large_file_mode then
    manager.on_doc_save(self)
  end
  return table.unpack(result, 1, result.n)
end

for _, doc in ipairs(core.docs) do
  if doc.abs_filename and not doc.large_file_mode then
    manager.open_doc(doc)
  end
end

keymap.add {
  ["ctrl+space"] = "lsp:complete",
  ["f12"] = "lsp:goto-definition",
  ["ctrl+f12"] = "lsp:goto-type-definition",
  ["shift+f12"] = "lsp:find-references",
  ["f8"] = "lsp:next-diagnostic",
  ["shift+f8"] = "lsp:previous-diagnostic",
  ["ctrl+t"] = "lsp:show-document-symbols",
  ["ctrl+alt+t"] = "lsp:workspace-symbols",
  ["ctrl+shift+a"] = "lsp:code-action",
  ["ctrl+shift+space"] = "lsp:signature-help",
  ["alt+shift+f"] = "lsp:format-document",
  ["f2"] = "lsp:rename-symbol",
  ["ctrl+k"] = "lsp:hover",
}

return manager
