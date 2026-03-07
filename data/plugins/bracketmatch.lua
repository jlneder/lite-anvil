-- mod-version:4
local core   = require "core"
local common = require "core.common"
local config = require "core.config"
local style  = require "core.style"
local DocView = require "core.docview"

config.plugins.bracketmatch = common.merge({
  highlight_color = nil,
}, config.plugins.bracketmatch)

local PAIRS = {
  ["("] = { match = ")", dir =  1 },
  [")"] = { match = "(", dir = -1 },
  ["["] = { match = "]", dir =  1 },
  ["]"] = { match = "[", dir = -1 },
  ["{"] = { match = "}", dir =  1 },
  ["}"] = { match = "{", dir = -1 },
}

local function find_match(doc, start_line, start_col, open, close, dir)
  local depth = 1
  local lines = doc.lines
  local limit = 2000

  if dir == 1 then
    for i = start_line, math.min(start_line + limit, #lines) do
      local text = lines[i]
      local s = (i == start_line) and (start_col + 1) or 1
      for j = s, #text do
        local ch = text:sub(j, j)
        if     ch == open  then depth = depth + 1
        elseif ch == close then
          depth = depth - 1
          if depth == 0 then return i, j end
        end
      end
    end
  else
    for i = start_line, math.max(start_line - limit, 1), -1 do
      local text = lines[i]
      local e = (i == start_line) and (start_col - 1) or (#text - 1)
      for j = e, 1, -1 do
        local ch = text:sub(j, j)
        if     ch == close then depth = depth + 1
        elseif ch == open  then
          depth = depth - 1
          if depth == 0 then return i, j end
        end
      end
    end
  end

  return nil, nil
end

local function bracket_pair_at(doc, line, col)
  local ch = doc:get_char(line, col)
  local info = PAIRS[ch]
  if not info then return end

  local open  = (info.dir ==  1) and ch or info.match
  local close = (info.dir ==  1) and info.match or ch
  local ml, mc = find_match(doc, line, col, open, close, info.dir)
  if ml then return line, col, ml, mc end
end

local function update_cache(dv)
  local doc = dv.doc
  local line1, col1, line2, col2 = doc:get_selection()

  if line1 ~= line2 or col1 ~= col2 then
    dv._bm_pos = nil
    dv._bm_key = nil
    return
  end

  local change_id = doc:get_change_id()
  local key = line1 .. "," .. col1 .. "," .. change_id
  if dv._bm_key == key then return end
  dv._bm_key = key

  local t = { bracket_pair_at(doc, line1, col1) }
  if #t == 0 and col1 > 1 then
    t = { bracket_pair_at(doc, line1, col1 - 1) }
  end
  dv._bm_pos = (#t > 0) and t or nil
end


local _update = DocView.update
function DocView:update(...)
  _update(self, ...)
  if self:is(DocView) and core.active_view == self then
    update_cache(self)
  end
end


local _draw_line_body = DocView.draw_line_body
function DocView:draw_line_body(line, x, y)
  local result = _draw_line_body(self, line, x, y)

  if core.active_view == self and self._bm_pos then
    local p = self._bm_pos
    local color = config.plugins.bracketmatch.highlight_color or style.caret
    local lh = self:get_line_height()
    local uw = math.max(2, math.floor(2 * SCALE))

    for i = 1, 3, 2 do
      if p[i] == line then
        local bc = p[i + 1]
        local x1 = x + self:get_col_x_offset(line, bc)
        local x2 = x + self:get_col_x_offset(line, bc + 1)
        renderer.draw_rect(x1, y + lh - uw, x2 - x1, uw, color)
      end
    end
  end

  return result
end
