-- Block height measurement for the markdown preview layout pass.
-- All functions are pure (no rendering side effects).

local M = {}

-- Simulate word-wrap and return the pixel height of the inline span list.
function M.inlines_height(inlines, width, fonts)
  if not inlines or #inlines == 0 then return 0 end
  local lh = fonts.body:get_height()
  local x, lines, last = 0, 1, false
  for _, span in ipairs(inlines) do
    if span.text == "\n" then
      x, lines, last = 0, lines + 1, false
    else
      local font = span.code and fonts.code or fonts.body
      local sw = font:get_width(" ")
      for word in span.text:gmatch("[^%s]+") do
        local ww = font:get_width(word)
        if last then
          if x + sw + ww > width then x, lines = 0, lines + 1 else x = x + sw end
        elseif x + ww > width and x > 0 then
          x, lines = 0, lines + 1
        end
        x, last = x + ww, true
      end
    end
  end
  return lines * lh
end

-- Return the pixel height of one block rendered at the given content width.
function M.block_height(blk, width, fonts, gap)
  local lh  = fonts.body:get_height()
  local clh = fonts.code:get_height()
  local t   = blk.type
  if t == "rule" then
    return math.floor(lh / 2)
  elseif t == "heading" then
    local hf = fonts["h" .. blk.level] or fonts.body
    return hf:get_height() + gap
  elseif t == "paragraph" then
    return M.inlines_height(blk.inlines, width, fonts)
  elseif t == "code_block" then
    local lines = 0
    for _ in (blk.text .. "\n"):gmatch("[^\n]+") do lines = lines + 1 end
    return math.max(1, lines) * clh + gap * 2
  elseif t == "blockquote" then
    local h = math.floor(gap / 2)
    for _, sub in ipairs(blk.blocks) do
      h = h + M.block_height(sub, width - 14, fonts, gap) + math.floor(gap / 2)
    end
    return math.max(h, lh)
  elseif t == "list" then
    local h = 0
    for _, item in ipairs(blk.items) do
      h = h + M.inlines_height(item, width - 20, fonts) + 2
    end
    return math.max(h, lh)
  elseif t == "table" then
    local n = (#blk.head > 0 and 1 or 0) + #blk.rows
    local extra = #blk.head > 0 and 3 or 0  -- 1 top border + 2 thick header divider
    return n * (lh + gap + 1) + extra + gap
  end
  return lh
end

-- Walk self.blocks, compute cumulative y offsets, store in self.layout.
-- Also sets self.content_height.
function M.compute(view, fonts, pad, gap)
  local width = view.size.x - pad * 2
  view.layout = {}
  local y = pad
  for i, blk in ipairs(view.blocks) do
    local h = M.block_height(blk, width, fonts, gap)
    view.layout[i] = { y = y, h = h }
    y = y + h + gap
  end
  view.content_height = y + pad
end

return M
