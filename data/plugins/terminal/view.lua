local core = require "core"
local command = require "core.command"
local common = require "core.common"
local config = require "core.config"
local keymap = require "core.keymap"
local style = require "core.style"
local View = require "core.view"
local terminal = require "terminal"
local terminal_buffer = require "terminal_buffer"
local color_schemes = require "..colors"

local TerminalView = View:extend()

function TerminalView:__tostring() return "TerminalView" end

TerminalView.context = "session"

local function rgba(color)
  return { color[1], color[2], color[3], color[4] or 0xff }
end

local function hex_to_rgba(hex)
  local r, g, b, a = common.color(hex)
  return { r, g, b, a }
end

local function make_palette(name)
  local scheme = color_schemes[name] or color_schemes[config.plugins.terminal.color_scheme] or color_schemes["eterm"]
  local palette = {}
  for i = 1, #(scheme.palette or {}) do
    palette[i - 1] = hex_to_rgba(scheme.palette[i])
  end
  return scheme, palette
end

local function parse_cwd(path)
  return path and common.normalize_path(path) or nil
end

local function default_cwd()
  local view = core.active_view
  if view and view.doc and view.doc.abs_filename then
    return parse_cwd(common.dirname(view.doc.abs_filename))
  end
  local project = core.root_project and core.root_project()
  if project and project.path then
    return parse_cwd(project.path)
  end
  return parse_cwd(os.getenv("HOME") or ".")
end

function TerminalView:new(options)
  TerminalView.super.new(self)
  self.cursor = "ibeam"
  self.scrollable = true
  self.font = style.code_font
  self.cwd = options.cwd or default_cwd()
  self.title = options.title or ("Terminal: " .. common.home_encode(self.cwd or "."))
  self.color_scheme = options.color_scheme or config.plugins.terminal.color_scheme or "eterm"
  self.cols = 0
  self.rows = 0
  self.scrollback = config.plugins.terminal.scrollback or 5000
  self.exit_notified = false
  self.last_blink = false
  self.last_dimensions = ""
  self:apply_color_scheme(self.color_scheme)
  self.buffer = terminal_buffer.new(80, 24, self.scrollback, self:palette_table(), self.default_fg)
  self:resize_screen(80, 24)
  self:spawn(options.command or self:default_command())
end

function TerminalView:get_name()
  local suffix = self.handle and self.handle:running() and "" or " [done]"
  return self.title .. suffix
end

function TerminalView:palette_table()
  local out = {}
  for i = 0, 15 do
    out[#out + 1] = self.palette[i]
  end
  return out
end

function TerminalView:apply_color_scheme(name)
  local scheme, palette = make_palette(name)
  self.color_scheme = name
  if not self.palette then
    self.palette = palette
  else
    for i = 0, 15 do
      self.palette[i] = palette[i]
    end
  end

  self.default_fg = scheme.foreground and hex_to_rgba(scheme.foreground) or rgba(style.text)
  self.default_bg = scheme.background and hex_to_rgba(scheme.background) or rgba(style.background)
  self.cursor_color = scheme.cursor and hex_to_rgba(scheme.cursor) or rgba(style.caret)

  if self.buffer then
    self.buffer:set_palette(self:palette_table(), self.default_fg)
  end
end

function TerminalView:default_command()
  local shell = config.plugins.terminal.shell or os.getenv("SHELL") or "sh"
  local command = { shell }
  for _, arg in ipairs(config.plugins.terminal.shell_args or {}) do
    command[#command + 1] = arg
  end
  return command
end

function TerminalView:get_line_height()
  return math.floor(self.font:get_height() * config.line_height)
end

function TerminalView:get_char_width()
  return self.font:get_width("M")
end

function TerminalView:get_content_size()
  return self.cols * self:get_char_width(), self.rows * self:get_line_height()
end

function TerminalView:get_scrollable_size()
  return (self.buffer:total_rows()) * self:get_line_height() + style.padding.y * 2
end

function TerminalView:supports_text_input()
  return true
end

function TerminalView:resize_screen(cols, rows)
  self.cols = math.max(1, cols)
  self.rows = math.max(1, rows)
  self.buffer:resize(self.cols, self.rows)
end

function TerminalView:spawn(command_argv)
  local ok, handle_or_err = pcall(terminal.spawn, command_argv, {
    cwd = self.cwd,
    cols = self.cols,
    rows = self.rows,
  })
  if not ok then
    core.error("Failed to start terminal: %s", handle_or_err)
    return
  end
  self.handle = handle_or_err
end

function TerminalView:try_close(do_close)
  if self.handle and self.handle:running() then
    core.nag_view:show(
      "Close Terminal",
      "This terminal is still running. Terminate it and close the tab?",
      {
        { text = "Terminate", default_yes = true },
        { text = "Cancel", default_no = true },
      },
      function(item)
        if item.text == "Terminate" then
          pcall(function() self.handle:terminate() end)
          do_close()
        end
      end
    )
    return
  end
  do_close()
end

function TerminalView:send_input(text)
  if self.handle and self.handle:running() then
    core.blink_reset()
    self.handle:write(text)
  end
end

function TerminalView:on_text_input(text)
  self:send_input(text)
end

function TerminalView:on_mouse_wheel(dy, dx)
  self.scroll.to.y = math.max(0, self.scroll.to.y - dy * self:get_line_height() * 3)
  return true
end

function TerminalView:clear()
  self.buffer:clear()
  self:resize_screen(self.cols, self.rows)
  self:scroll_to_bottom(true)
end

function TerminalView:scroll_to_bottom(force)
  local target = math.max(0, self:get_scrollable_size() - self.size.y)
  self.scroll.to.y = target
  if force then
    self.scroll.y = target
  end
end

function TerminalView:get_dimensions()
  local cols = math.max(1, math.floor((self.size.x - style.padding.x * 2) / self:get_char_width()))
  local rows = math.max(1, math.floor((self.size.y - style.padding.y * 2) / self:get_line_height()))
  return cols, rows
end

function TerminalView:update()
  TerminalView.super.update(self)

  local cols, rows = self:get_dimensions()
  local dim_key = cols .. "x" .. rows
  if dim_key ~= self.last_dimensions then
    self.last_dimensions = dim_key
    self:resize_screen(cols, rows)
    if self.handle then
      self.handle:resize(cols, rows)
    end
  end

  local at_bottom = self.scroll.to.y >= math.max(0, self:get_scrollable_size() - self.size.y - self:get_line_height())
  if self.handle then
    for _ = 1, 64 do
      local chunk = self.handle:read(4096)
      if not chunk or chunk == "" then
        break
      end
      self.buffer:process_output(chunk)
      core.redraw = true
    end

    if not self.handle:running() and not self.exit_notified then
      self.exit_notified = true
      if config.plugins.terminal.close_on_exit then
        local node = core.root_view.root_node:get_node_for_view(self)
        if node then
          node:close_view(core.root_view.root_node, self)
          return
        end
      end
      core.status_view:show_message("i", style.text, string.format(
        "Terminal exited with code %s",
        tostring(self.handle:returncode() or "?")
      ))
      core.redraw = true
    end
  end

  if at_bottom then
    self:scroll_to_bottom()
  end
end

function TerminalView:draw_row(row, x, y)
  local cell_w = self:get_char_width()
  local cell_h = self:get_line_height()
  for _, run in ipairs(row.runs or {}) do
    if run.bg then
      renderer.draw_rect(
        x + (run.start_col - 1) * cell_w,
        y,
        (run.end_col - run.start_col + 1) * cell_w,
        cell_h,
        run.bg
      )
    end
  end
  for _, run in ipairs(row.runs or {}) do
    renderer.draw_text(
      self.font,
      run.text,
      x + (run.start_col - 1) * cell_w,
      y + self:get_line_text_y_offset(),
      run.fg or self.default_fg
    )
  end
end

function TerminalView:get_line_text_y_offset()
  local lh = self:get_line_height()
  local th = self.font:get_height()
  return (lh - th) / 2
end

function TerminalView:draw_cursor()
  if core.active_view ~= self or not system.window_has_focus(core.window) then
    return
  end
  local T = config.blink_period
  local visible = config.disable_blink or (system.get_time() - core.blink_start) % T < T / 2
  if not visible then
    return
  end

  local cursor = self.buffer:cursor()
  local row_index = cursor.history + cursor.row
  local y = self.position.y + style.padding.y + (row_index - 1) * self:get_line_height() - self.scroll.y
  if y + self:get_line_height() < self.position.y or y > self.position.y + self.size.y then
    return
  end
  local x = self.position.x + style.padding.x + (cursor.col - 1) * self:get_char_width()
  renderer.draw_rect(x, y, math.max(1, style.caret_width), self:get_line_height(), self.cursor_color)
end

function TerminalView:draw()
  self:draw_background(style.background)
  renderer.draw_rect(self.position.x, self.position.y, self.size.x, self.size.y, self.default_bg)

  local total_rows = self.buffer:total_rows()
  local first_row = math.max(1, math.floor(self.scroll.y / self:get_line_height()) + 1)
  local last_row = math.min(total_rows, math.ceil((self.scroll.y + self.size.y) / self:get_line_height()) + 1)
  local x = self.position.x + style.padding.x

  core.push_clip_rect(self.position.x, self.position.y, self.size.x, self.size.y)
  for _, row in ipairs(self.buffer:render_rows(first_row, last_row)) do
    local y = self.position.y + style.padding.y + (row.index - 1) * self:get_line_height() - self.scroll.y
    self:draw_row(row, x, y)
  end
  self:draw_cursor()
  core.pop_clip_rect()

  self:draw_scrollbar()
end

function TerminalView.open(cwd, command_argv, title)
  local view = TerminalView({
    cwd = cwd or default_cwd(),
    command = command_argv,
    title = title,
  })
  core.root_view:get_active_node_default():add_view(view)
  core.root_view.root_node:update_layout()
  core.set_active_view(view)
  return view
end

function TerminalView:rename()
  core.command_view:enter("Rename Terminal", {
    text = self.title:gsub("^Terminal:%s*", ""),
    submit = function(text)
      if text == "" then
        return
      end
      self.title = text
      core.redraw = true
    end,
  })
end

function TerminalView:switch_color_scheme(direction)
  local names = {}
  for name in pairs(color_schemes) do
    names[#names + 1] = name
  end
  table.sort(names)
  local current = 1
  for i = 1, #names do
    if names[i] == self.color_scheme then
      current = i
      break
    end
  end
  current = ((current - 1 + direction) % #names) + 1
  self:apply_color_scheme(names[current])
  config.plugins.terminal.color_scheme = names[current]
  core.status_view:show_message("i", style.text, "Terminal color scheme: " .. names[current])
  core.redraw = true
end

command.add(TerminalView, {
  ["terminal:send-enter"] = function(view)
    view:send_input("\r")
  end,
  ["terminal:send-backspace"] = function(view)
    view:send_input(string.char(0x7f))
  end,
  ["terminal:send-tab"] = function(view)
    view:send_input("\t")
  end,
  ["terminal:send-escape"] = function(view)
    view:send_input("\27")
  end,
  ["terminal:send-up"] = function(view)
    view:send_input("\27[A")
  end,
  ["terminal:send-down"] = function(view)
    view:send_input("\27[B")
  end,
  ["terminal:send-right"] = function(view)
    view:send_input("\27[C")
  end,
  ["terminal:send-left"] = function(view)
    view:send_input("\27[D")
  end,
  ["terminal:send-home"] = function(view)
    view:send_input("\27[H")
  end,
  ["terminal:send-end"] = function(view)
    view:send_input("\27[F")
  end,
  ["terminal:interrupt"] = function(view)
    view:send_input(string.char(3))
  end,
  ["terminal:send-eof"] = function(view)
    view:send_input(string.char(4))
  end,
  ["terminal:suspend"] = function(view)
    view:send_input(string.char(26))
  end,
  ["terminal:clear"] = function(view)
    view:clear()
    view:send_input(string.char(12))
  end,
  ["terminal:rename"] = function(view)
    view:rename()
  end,
  ["terminal:next-colorscheme"] = function(view)
    view:switch_color_scheme(1)
  end,
  ["terminal:previous-colorscheme"] = function(view)
    view:switch_color_scheme(-1)
  end,
})

keymap.add {
  ["return"] = "terminal:send-enter",
  ["backspace"] = "terminal:send-backspace",
  ["tab"] = "terminal:send-tab",
  ["escape"] = "terminal:send-escape",
  ["up"] = "terminal:send-up",
  ["down"] = "terminal:send-down",
  ["left"] = "terminal:send-left",
  ["right"] = "terminal:send-right",
  ["home"] = "terminal:send-home",
  ["end"] = "terminal:send-end",
  ["ctrl+c"] = "terminal:interrupt",
  ["ctrl+d"] = "terminal:send-eof",
  ["ctrl+l"] = "terminal:clear",
  ["ctrl+z"] = "terminal:suspend",
  ["ctrl+alt+s"] = "terminal:rename",
  ["ctrl+alt+]"] = "terminal:next-colorscheme",
  ["ctrl+alt+["] = "terminal:previous-colorscheme",
}

return TerminalView
