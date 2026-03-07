-- mod-version:4
local core    = require "core"
local common  = require "core.common"
local keymap  = require "core.keymap"
local command = require "core.command"
local style   = require "core.style"
local View    = require "core.view"

local ReplaceView = View:extend()

function ReplaceView:__tostring() return "ReplaceView" end

ReplaceView.context = "session"

function ReplaceView:new(path, search, replace, fn_find, fn_apply)
  ReplaceView.super.new(self)
  self.scrollable    = true
  self.max_h_scroll  = 0
  self.path          = path
  self.search        = search
  self.replace       = replace
  self.fn_find       = fn_find
  self.fn_apply      = fn_apply
  self.results       = {}
  self.phase         = "scanning"
  self.last_file_idx = 1
  self.selected_idx  = 0
  self.brightness    = 0
  self.replaced_count = 0
  self.replaced_files = 0
  self:begin_scan()
end

function ReplaceView:get_name()
  return "Replace Results"
end

local function collect_matches(results, filename, fn_find)
  local fp = io.open(filename)
  if not fp then return end
  local n = 1
  for line in fp:lines() do
    local s = fn_find(line)
    if s then
      local start_index = math.max(s - 80, 1)
      local text = (start_index > 1 and "..." or "") .. line:sub(start_index, 256 + start_index)
      if #line > 256 + start_index then text = text .. "..." end
      table.insert(results, { file = filename, text = text, line = n, col = s })
      core.redraw = true
    end
    if n % 100 == 0 then coroutine.yield(0) end
    n = n + 1
  end
  fp:close()
end

function ReplaceView:begin_scan()
  self.results       = {}
  self.phase         = "scanning"
  self.last_file_idx = 1

  core.add_thread(function()
    local i = 1
    for _, project in ipairs(core.projects) do
      for _, file in project:files() do
        if file.type == "file" and (not self.path or file.filename:find(self.path, 1, true) == 1) then
          collect_matches(self.results, file.filename, self.fn_find)
        end
        self.last_file_idx = i
        i = i + 1
      end
    end
    self.phase      = "confirming"
    self.brightness = 100
    core.redraw = true
  end, self.results)

  self.scroll.to.y = 0
end

function ReplaceView:apply_replace()
  self.phase          = "replacing"
  self.replaced_count = 0
  self.replaced_files = 0
  core.redraw = true

  core.add_thread(function()
    local files = {}
    local seen  = {}
    for _, item in ipairs(self.results) do
      if not seen[item.file] then
        seen[item.file] = true
        table.insert(files, item.file)
      end
    end

    for _, filename in ipairs(files) do
      local fp = io.open(filename, "rb")
      if fp then
        local content = fp:read("*a")
        fp:close()
        local new_content, count = self.fn_apply(content)
        if count > 0 then
          local out = io.open(filename, "wb")
          if out then
            out:write(new_content)
            out:close()
            self.replaced_count = self.replaced_count + count
            self.replaced_files = self.replaced_files + 1
          end
        end
      end
      coroutine.yield(0)
      core.redraw = true
    end

    self.phase      = "done"
    self.brightness = 100
    core.redraw = true
  end)
end

function ReplaceView:on_mouse_moved(mx, my, ...)
  ReplaceView.super.on_mouse_moved(self, mx, my, ...)
  self.selected_idx = 0
  for i, item, x, y, w, h in self:each_visible_result() do
    if mx >= x and my >= y and mx < x + w and my < y + h then
      self.selected_idx = i
      break
    end
  end
end

function ReplaceView:on_mouse_pressed(...)
  local caught = ReplaceView.super.on_mouse_pressed(self, ...)
  if not caught then
    return self:open_selected_result()
  end
end

function ReplaceView:open_selected_result()
  local res = self.results[self.selected_idx]
  if not res then return end
  core.try(function()
    local dv = core.root_view:open_doc(core.open_doc(res.file))
    core.root_view.root_node:update_layout()
    dv.doc:set_selection(res.line, res.col)
    dv:scroll_to_line(res.line, false, true)
  end)
  return true
end

function ReplaceView:update()
  self:move_towards("brightness", 0, 0.1)
  ReplaceView.super.update(self)
end

function ReplaceView:get_results_yoffset()
  return style.font:get_height() + style.padding.y * 3
end

function ReplaceView:get_line_height()
  return style.padding.y + style.font:get_height()
end

function ReplaceView:get_scrollable_size()
  return self:get_results_yoffset() + #self.results * self:get_line_height()
end

function ReplaceView:get_h_scrollable_size()
  return self.max_h_scroll
end

function ReplaceView:get_visible_results_range()
  local lh  = self:get_line_height()
  local oy  = self:get_results_yoffset()
  local min = math.max(1, math.floor((self.scroll.y + oy - style.font:get_height()) / lh))
  return min, min + math.floor(self.size.y / lh) + 1
end

function ReplaceView:each_visible_result()
  return coroutine.wrap(function()
    local lh    = self:get_line_height()
    local x, y  = self:get_content_offset()
    local min, max = self:get_visible_results_range()
    y = y + self:get_results_yoffset() + lh * (min - 1)
    for i = min, max do
      local item = self.results[i]
      if not item then break end
      local _, _, w = self:get_content_bounds()
      coroutine.yield(i, item, x, y, w, lh)
      y = y + lh
    end
  end)
end

function ReplaceView:scroll_to_make_selected_visible()
  local h = self:get_line_height()
  local y = h * (self.selected_idx - 1)
  self.scroll.to.y = math.min(self.scroll.to.y, y)
  self.scroll.to.y = math.max(self.scroll.to.y, y + h - self.size.y + self:get_results_yoffset())
end

function ReplaceView:draw()
  self:draw_background(style.background)

  local ox, oy   = self.position.x, self.position.y
  local yoffset  = self:get_results_yoffset()
  renderer.draw_rect(ox, oy, self.size.x, yoffset, style.background)
  if self.scroll.y ~= 0 then
    renderer.draw_rect(ox, oy + yoffset, self.size.x, style.divider_size, style.divider)
  end

  local color = common.lerp(style.text, style.accent, self.brightness / 100)
  local x, y  = ox + style.padding.x, oy + style.padding.y

  local msg
  if self.phase == "scanning" then
    msg = string.format("Searching (%d files, %d matches) for %q...",
      self.last_file_idx, #self.results, self.search)
  elseif self.phase == "confirming" then
    msg = string.format("Found %d matches for %q — press F5 to replace all with %q",
      #self.results, self.search, self.replace)
  elseif self.phase == "replacing" then
    msg = string.format("Replacing... (%d files written)", self.replaced_files)
  else
    msg = string.format("Done — replaced %d occurrences in %d files (%q -> %q)",
      self.replaced_count, self.replaced_files, self.search, self.replace)
  end
  renderer.draw_text(style.font, msg, x, y, color)

  local dcolor = common.lerp(style.dim, style.text, self.brightness / 100)
  renderer.draw_rect(x, oy + yoffset - style.padding.y,
    self.size.x - style.padding.x * 2, style.divider_size, dcolor)

  local _, _, bw = self:get_content_bounds()
  core.push_clip_rect(ox, oy + yoffset + style.divider_size, bw, self.size.y - yoffset)
  for i, item, ix, iy, iw, ih in self:each_visible_result() do
    local tc = style.text
    if i == self.selected_idx then
      tc = style.accent
      renderer.draw_rect(ix, iy, iw, ih, style.line_highlight)
    end
    ix = ix + style.padding.x
    local label = string.format("%s at line %d (col %d): ",
      core.root_project():normalize_path(item.file), item.line, item.col)
    ix = common.draw_text(style.font, style.dim, label, "left", ix, iy, iw, ih)
    ix = common.draw_text(style.code_font, tc, item.text, "left", ix, iy, iw, ih)
    self.max_h_scroll = math.max(self.max_h_scroll, ix)
  end
  core.pop_clip_rect()

  self:draw_scrollbar()
end


local function plain_replace(content, search_text, replace_text)
  local parts = {}
  local count  = 0
  local pos    = 1
  while true do
    local s, e = content:find(search_text, pos, true)
    if not s then
      table.insert(parts, content:sub(pos))
      break
    end
    table.insert(parts, content:sub(pos, s - 1))
    table.insert(parts, replace_text)
    pos   = e + 1
    count = count + 1
  end
  return table.concat(parts), count
end

local function regex_replace(content, re, replace_text)
  local parts = {}
  local count  = 0
  local pos    = 1
  while pos <= #content do
    local s, e = re:cmatch(content, pos)
    if not s then
      table.insert(parts, content:sub(pos))
      break
    end
    table.insert(parts, content:sub(pos, s - 1))
    table.insert(parts, replace_text)
    count = count + 1
    if e > s then
      pos = e
    else
      table.insert(parts, content:sub(s, s))
      pos = s + 1
    end
  end
  return table.concat(parts), count
end

local function get_selected_text()
  local view = core.active_view
  local doc  = view and view.doc
  if doc then
    return doc:get_text(table.unpack({ doc:get_selection() }))
  end
end

local function open_replace_view(path, search, replace, fn_find, fn_apply)
  if search == "" then
    core.error("Expected non-empty search string")
    return
  end
  local rv = ReplaceView(path, search, replace, fn_find, fn_apply)
  core.root_view:get_active_node_default():add_view(rv)
  return rv
end


command.add(nil, {
  ["project-search:replace"] = function(path)
    core.command_view:enter("Replace Text In " .. (path or "Project"), {
      text = get_selected_text(),
      select_text = true,
      submit = function(search)
        core.command_view:enter("Replace With", {
          submit = function(replace)
            open_replace_view(path, search, replace,
              function(line_text)
                return line_text:find(search, nil, true)
              end,
              function(content)
                return plain_replace(content, search, replace)
              end
            )
          end
        })
      end
    })
  end,

  ["project-search:replace-regex"] = function(path)
    core.command_view:enter("Replace Regex In " .. (path or "Project"), {
      submit = function(search)
        local re, errmsg = regex.compile(search)
        if not re then core.log("%s", errmsg) return end
        core.command_view:enter("Replace With", {
          submit = function(replace)
            open_replace_view(path, search, replace,
              function(line_text)
                return regex.cmatch(re, line_text)
              end,
              function(content)
                return regex_replace(content, re, replace)
              end
            )
          end
        })
      end
    })
  end,
})


command.add(ReplaceView, {
  ["project-search:confirm-replace"] = function()
    local view = core.active_view
    if view.phase == "confirming" then
      view:apply_replace()
    end
  end,

  ["project-search:select-previous"] = function()
    local view = core.active_view
    view.selected_idx = math.max(view.selected_idx - 1, 1)
    view:scroll_to_make_selected_visible()
  end,

  ["project-search:select-next"] = function()
    local view = core.active_view
    view.selected_idx = math.min(view.selected_idx + 1, #view.results)
    view:scroll_to_make_selected_visible()
  end,

  ["project-search:open-selected"] = function()
    core.active_view:open_selected_result()
  end,

  ["project-search:move-to-previous-page"] = function()
    local view = core.active_view
    view.scroll.to.y = view.scroll.to.y - view.size.y
  end,

  ["project-search:move-to-next-page"] = function()
    local view = core.active_view
    view.scroll.to.y = view.scroll.to.y + view.size.y
  end,

  ["project-search:move-to-start-of-doc"] = function()
    local view = core.active_view
    view.scroll.to.y = 0
  end,

  ["project-search:move-to-end-of-doc"] = function()
    local view = core.active_view
    view.scroll.to.y = view:get_scrollable_size()
  end,
})

keymap.add {
  ["ctrl+shift+h"] = "project-search:replace",
  ["f5"]           = "project-search:confirm-replace",
  ["up"]           = "project-search:select-previous",
  ["down"]         = "project-search:select-next",
  ["return"]       = "project-search:open-selected",
  ["pageup"]       = "project-search:move-to-previous-page",
  ["pagedown"]     = "project-search:move-to-next-page",
  ["ctrl+home"]    = "project-search:move-to-start-of-doc",
  ["ctrl+end"]     = "project-search:move-to-end-of-doc",
  ["home"]         = "project-search:move-to-start-of-doc",
  ["end"]          = "project-search:move-to-end-of-doc",
}
