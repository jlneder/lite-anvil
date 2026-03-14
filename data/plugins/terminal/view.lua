local core = require "core"
local command = require "core.command"
local common = require "core.common"
local config = require "core.config"
local keymap = require "core.keymap"
local style = require "core.style"
local View = require "core.view"
local terminal = require "terminal"
local color_schemes = require "..colors"

local TerminalView = View:extend()

function TerminalView:__tostring() return "TerminalView" end

TerminalView.context = "session"

local function rgba(color)
  return { color[1], color[2], color[3], color[4] or 0xff }
end

local function clamp(v, lo, hi)
  if v < lo then return lo end
  if v > hi then return hi end
  return v
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

local function decode_utf8_char(text, i)
  local b = text:byte(i)
  if not b then
    return nil, i + 1
  end
  if b < 0x80 then
    return text:sub(i, i), i + 1
  elseif b < 0xE0 then
    return text:sub(i, i + 1), i + 2
  elseif b < 0xF0 then
    return text:sub(i, i + 2), i + 3
  end
  return text:sub(i, i + 3), i + 4
end

local function basename(path)
  return path and path:match("([^/\\]+)$") or path
end

local function copy_cell(cell)
  return {
    char = cell.char,
    fg = cell.fg,
    bg = cell.bg,
  }
end

function TerminalView:new(options)
  TerminalView.super.new(self)
  self.cursor = "ibeam"
  self.scrollable = true
  self.font = style.code_font
  self.cwd = options.cwd or default_cwd()
  self.title = options.title or ("Terminal: " .. common.home_encode(self.cwd or "."))
  self.color_scheme = options.color_scheme or config.plugins.terminal.color_scheme or "eterm"
  self:apply_color_scheme(self.color_scheme)
  self.cols = 0
  self.rows = 0
  self.screen = {}
  self.history = {}
  self.scrollback = config.plugins.terminal.scrollback or 5000
  self.cursor_row = 1
  self.cursor_col = 1
  self.escape_state = nil
  self.escape_buffer = ""
  self.osc_esc = false
  self.exit_notified = false
  self.last_blink = false
  self.last_dimensions = ""
  self:resize_screen(80, 24)
  self:spawn(options.command or self:default_command())
end

function TerminalView:get_name()
  local suffix = self.handle and self.handle:running() and "" or " [done]"
  return self.title .. suffix
end

function TerminalView:apply_color_scheme(name)
  local scheme, palette = make_palette(name)
  self.color_scheme = name
  if not self.palette then
    self.palette = palette
  else
    for i = 0, 15 do
      if self.palette[i] and palette[i] then
        for j = 1, 4 do
          self.palette[i][j] = palette[i][j]
        end
      else
        self.palette[i] = palette[i]
      end
    end
  end

  local next_default_fg = scheme.foreground and hex_to_rgba(scheme.foreground) or rgba(style.text)
  local next_default_bg = scheme.background and hex_to_rgba(scheme.background) or rgba(style.background)
  local next_cursor = scheme.cursor and hex_to_rgba(scheme.cursor) or rgba(style.caret)

  if self.default_fg then
    for i = 1, 4 do self.default_fg[i] = next_default_fg[i] end
  else
    self.default_fg = next_default_fg
  end
  if self.default_bg then
    for i = 1, 4 do self.default_bg[i] = next_default_bg[i] end
  else
    self.default_bg = next_default_bg
  end
  if self.cursor_color then
    for i = 1, 4 do self.cursor_color[i] = next_cursor[i] end
  else
    self.cursor_color = next_cursor
  end

  self.current_fg = self.default_fg
  self.current_bg = nil
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
  return (#self.history + self.rows) * self:get_line_height() + style.padding.y * 2
end

function TerminalView:supports_text_input()
  return true
end

function TerminalView:make_blank_row(cols)
  local row = {}
  for i = 1, cols do
    row[i] = { char = " ", fg = self.default_fg, bg = nil }
  end
  return row
end

function TerminalView:resize_screen(cols, rows)
  cols = math.max(1, cols)
  rows = math.max(1, rows)
  local old_screen = self.screen
  local old_rows = self.rows
  local old_cols = self.cols
  self.cols = cols
  self.rows = rows
  self.screen = {}
  for row = 1, rows do
    self.screen[row] = self:make_blank_row(cols)
  end
  if old_rows and old_rows > 0 then
    local copy_rows = math.min(old_rows, rows)
    for i = 0, copy_rows - 1 do
      local src = old_screen[old_rows - i]
      local dst = self.screen[rows - i]
      if src and dst then
        for col = 1, math.min(old_cols, cols) do
          dst[col] = copy_cell(src[col])
        end
      end
    end
  end
  self.cursor_row = clamp(self.cursor_row or 1, 1, rows)
  self.cursor_col = clamp(self.cursor_col or 1, 1, cols)
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
  self.history = {}
  self.current_fg = self.default_fg
  self.current_bg = nil
  self.cursor_row = 1
  self.cursor_col = 1
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

function TerminalView:push_history(row)
  self.history[#self.history + 1] = row
  if #self.history > self.scrollback then
    table.remove(self.history, 1)
  end
end

function TerminalView:scroll_screen()
  self:push_history(self.screen[1])
  table.remove(self.screen, 1)
  self.screen[self.rows] = self:make_blank_row(self.cols)
end

function TerminalView:put_char(ch)
  if self.cursor_col > self.cols then
    self.cursor_col = 1
    self.cursor_row = self.cursor_row + 1
  end
  if self.cursor_row > self.rows then
    self:scroll_screen()
    self.cursor_row = self.rows
  end
  local row = self.screen[self.cursor_row]
  row[self.cursor_col] = {
    char = ch,
    fg = self.current_fg,
    bg = self.current_bg,
  }
  self.cursor_col = self.cursor_col + 1
end

function TerminalView:newline()
  self.cursor_col = 1
  self.cursor_row = self.cursor_row + 1
  if self.cursor_row > self.rows then
    self:scroll_screen()
    self.cursor_row = self.rows
  end
end

function TerminalView:clear_line(mode)
  local row = self.screen[self.cursor_row]
  local start_col, end_col = 1, self.cols
  if mode == 0 then
    start_col = self.cursor_col
  elseif mode == 1 then
    end_col = self.cursor_col
  end
  for col = start_col, end_col do
    row[col] = { char = " ", fg = self.default_fg, bg = nil }
  end
end

function TerminalView:clear_screen(mode)
  if mode == 2 then
    for row = 1, self.rows do
      self.screen[row] = self:make_blank_row(self.cols)
    end
    self.cursor_row = 1
    self.cursor_col = 1
    return
  end

  if mode == 0 then
    self:clear_line(0)
    for row = self.cursor_row + 1, self.rows do
      self.screen[row] = self:make_blank_row(self.cols)
    end
  elseif mode == 1 then
    self:clear_line(1)
    for row = 1, self.cursor_row - 1 do
      self.screen[row] = self:make_blank_row(self.cols)
    end
  end
end

function TerminalView:ansi_color_256(idx)
  if idx < 16 then
    return self.palette[idx]
  elseif idx < 232 then
    idx = idx - 16
    local levels = { 0, 95, 135, 175, 215, 255 }
    local r = levels[math.floor(idx / 36) % 6 + 1]
    local g = levels[math.floor(idx / 6) % 6 + 1]
    local b = levels[idx % 6 + 1]
    return { r, g, b, 0xff }
  end
  local c = 8 + (idx - 232) * 10
  return { c, c, c, 0xff }
end

function TerminalView:apply_sgr(params)
  if #params == 0 then
    params = { 0 }
  end

  local i = 1
  while i <= #params do
    local code = tonumber(params[i]) or 0
    if code == 0 then
      self.current_fg = self.default_fg
      self.current_bg = nil
    elseif code == 39 then
      self.current_fg = self.default_fg
    elseif code == 49 then
      self.current_bg = nil
    elseif code >= 30 and code <= 37 then
      self.current_fg = self.palette[code - 30]
    elseif code >= 40 and code <= 47 then
      self.current_bg = self.palette[code - 40]
    elseif code >= 90 and code <= 97 then
      self.current_fg = self.palette[8 + code - 90]
    elseif code >= 100 and code <= 107 then
      self.current_bg = self.palette[8 + code - 100]
    elseif (code == 38 or code == 48) and params[i + 1] then
      local is_fg = code == 38
      local mode = tonumber(params[i + 1]) or 0
      if mode == 5 and params[i + 2] then
        local color = self:ansi_color_256(tonumber(params[i + 2]) or 0)
        if is_fg then self.current_fg = color else self.current_bg = color end
        i = i + 2
      elseif mode == 2 and params[i + 4] then
        local color = {
          tonumber(params[i + 2]) or 0,
          tonumber(params[i + 3]) or 0,
          tonumber(params[i + 4]) or 0,
          0xff,
        }
        if is_fg then self.current_fg = color else self.current_bg = color end
        i = i + 4
      end
    end
    i = i + 1
  end
end

function TerminalView:execute_csi(sequence)
  local final = sequence:sub(-1)
  local body = sequence:sub(1, -2)
  local params = {}
  for item in (body .. ";"):gmatch("(.-);") do
    params[#params + 1] = item
  end

  local p1 = tonumber(params[1]) or 0
  local p2 = tonumber(params[2]) or 0

  if final == "A" then
    self.cursor_row = clamp(self.cursor_row - math.max(p1, 1), 1, self.rows)
  elseif final == "B" then
    self.cursor_row = clamp(self.cursor_row + math.max(p1, 1), 1, self.rows)
  elseif final == "C" then
    self.cursor_col = clamp(self.cursor_col + math.max(p1, 1), 1, self.cols)
  elseif final == "D" then
    self.cursor_col = clamp(self.cursor_col - math.max(p1, 1), 1, self.cols)
  elseif final == "H" or final == "f" then
    self.cursor_row = clamp((tonumber(params[1]) or 1), 1, self.rows)
    self.cursor_col = clamp((tonumber(params[2]) or 1), 1, self.cols)
  elseif final == "J" then
    self:clear_screen(p1)
  elseif final == "K" then
    self:clear_line(p1)
  elseif final == "m" then
    self:apply_sgr(params)
  end
end

function TerminalView:process_output(text)
  local i = 1
  while i <= #text do
    local b = text:byte(i)
    if self.escape_state == "osc" then
      if b == 7 then
        self.escape_state = nil
      elseif b == 27 then
        self.osc_esc = true
      elseif self.osc_esc and b == 92 then
        self.escape_state = nil
        self.osc_esc = false
      else
        self.osc_esc = false
      end
      i = i + 1
    elseif self.escape_state == "esc" then
      local ch = text:sub(i, i)
      if ch == "[" then
        self.escape_state = "csi"
        self.escape_buffer = ""
      elseif ch == "]" then
        self.escape_state = "osc"
        self.osc_esc = false
      elseif ch == "c" then
        self:clear()
        self.escape_state = nil
      else
        self.escape_state = nil
      end
      i = i + 1
    elseif self.escape_state == "csi" then
      local ch = text:sub(i, i)
      self.escape_buffer = self.escape_buffer .. ch
      if ch:match("[@-~]") then
        self:execute_csi(self.escape_buffer)
        self.escape_buffer = ""
        self.escape_state = nil
      end
      i = i + 1
    elseif b == 27 then
      self.escape_state = "esc"
      i = i + 1
    elseif b == 13 then
      self.cursor_col = 1
      i = i + 1
    elseif b == 10 then
      self:newline()
      i = i + 1
    elseif b == 8 then
      self.cursor_col = math.max(1, self.cursor_col - 1)
      i = i + 1
    elseif b == 9 then
      local next_tab = math.min(self.cols + 1, self.cursor_col + (8 - ((self.cursor_col - 1) % 8)))
      while self.cursor_col < next_tab do
        self:put_char(" ")
      end
      i = i + 1
    elseif b < 32 then
      i = i + 1
    else
      local ch
      ch, i = decode_utf8_char(text, i)
      self:put_char(ch)
    end
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
      self:process_output(chunk)
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

function TerminalView:get_display_row(index)
  if index <= #self.history then
    return self.history[index]
  end
  return self.screen[index - #self.history]
end

function TerminalView:draw_row(row, x, y)
  local cell_w = self:get_char_width()
  local cell_h = self:get_line_height()
  local tx = x

  local start = 1
  while start <= self.cols do
    local bg = row[start].bg
    local finish = start
    while finish + 1 <= self.cols and row[finish + 1].bg == bg do
      finish = finish + 1
    end
    if bg then
      renderer.draw_rect(x + (start - 1) * cell_w, y, (finish - start + 1) * cell_w, cell_h, bg)
    end
    start = finish + 1
  end

  start = 1
  while start <= self.cols do
    local fg = row[start].fg
    local finish = start
    local chars = { row[start].char }
    while finish + 1 <= self.cols and row[finish + 1].fg == fg do
      finish = finish + 1
      chars[#chars + 1] = row[finish].char
    end
    renderer.draw_text(self.font, table.concat(chars), tx + (start - 1) * cell_w, y + self:get_line_text_y_offset(), fg or self.default_fg)
    start = finish + 1
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

  local row_index = #self.history + self.cursor_row
  local y = self.position.y + style.padding.y + (row_index - 1) * self:get_line_height() - self.scroll.y
  if y + self:get_line_height() < self.position.y or y > self.position.y + self.size.y then
    return
  end
  local x = self.position.x + style.padding.x + (self.cursor_col - 1) * self:get_char_width()
  renderer.draw_rect(x, y, math.max(1, style.caret_width), self:get_line_height(), self.cursor_color)
end

function TerminalView:draw()
  self:draw_background(style.background)
  renderer.draw_rect(self.position.x, self.position.y, self.size.x, self.size.y, self.default_bg)

  local total_rows = #self.history + self.rows
  local first_row = math.max(1, math.floor(self.scroll.y / self:get_line_height()) + 1)
  local last_row = math.min(total_rows, math.ceil((self.scroll.y + self.size.y) / self:get_line_height()) + 1)
  local x = self.position.x + style.padding.x

  core.push_clip_rect(self.position.x, self.position.y, self.size.x, self.size.y)
  for row_index = first_row, last_row do
    local row = self:get_display_row(row_index)
    if row then
      local y = self.position.y + style.padding.y + (row_index - 1) * self:get_line_height() - self.scroll.y
      self:draw_row(row, x, y)
    end
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
