-- Block and inline renderers for the markdown preview.

local core   = require "core"
local style  = require "core.style"
local layout = require "plugins.markdown_preview.layout"

local LINK_COLOR = { 88, 166, 255, 255 }

local M = {}

-- ── Color and font helpers ────────────────────────────────────────────────────

local function span_color(span)
  if span.href          then return LINK_COLOR                end
  if span.code          then return style.syntax["string"]    end
  if span.italic        then return style.syntax["comment"]   end
  if span.bold          then return style.syntax["keyword"]   end
  if span.strikethrough then return style.dim                 end
  return style.text
end

-- ── Inline renderer ───────────────────────────────────────────────────────────

-- Draw spans with word-wrap; appends hit regions for links.
-- Returns the y position after the final line.
function M.draw_inlines(view, inlines, x0, y0, max_x, fonts, base_font, forced_color)
  if not inlines or #inlines == 0 then return y0 end
  base_font = base_font or fonts.body
  local lh = base_font:get_height()
  local x, y = x0, y0
  local last = false  -- true after a word has been drawn on this line

  for _, span in ipairs(inlines) do
    if span.text == "\n" then
      x, y, last = x0, y + lh, false
    else
      local font = span.code and fonts.code or base_font
      local col  = forced_color or span_color(span)
      local sw   = font:get_width(" ")

      for word in span.text:gmatch("[^%s]+") do
        local ww = font:get_width(word)
        if last then
          if x + sw + ww > max_x and x > x0 then
            x, y, last = x0, y + lh, false
          else
            x = x + sw
          end
        elseif x + ww > max_x and x > x0 then
          x, y, last = x0, y + lh, false
        end

        local wx0 = x
        x = renderer.draw_text(font, word, x, y, col)
        if span.href then
          table.insert(view.link_regions, { x1=wx0, y1=y, x2=x, y2=y+lh, href=span.href })
        end
        last = true
      end
    end
  end
  return y + lh
end

-- ── Block renderers ───────────────────────────────────────────────────────────

local function draw_heading(view, blk, x, y, max_x, fonts, gap)
  local hf  = fonts["h" .. blk.level] or fonts.body
  M.draw_inlines(view, blk.inlines, x, y, max_x, fonts, hf, style.syntax["keyword"])
end

local function draw_paragraph(view, blk, x, y, max_x, fonts)
  M.draw_inlines(view, blk.inlines, x, y, max_x, fonts)
end

local function draw_code_block(view, blk, x, y, max_x, fonts, gap)
  local entry_h = layout.block_height(blk, max_x - x, fonts, gap)
  renderer.draw_rect(x - 4, y, max_x - x + 8, entry_h, style.line_highlight)
  local cy = y + math.floor(gap / 2)
  local clh = fonts.code:get_height()
  for line in (blk.text .. "\n"):gmatch("[^\n]*\n") do
    core.push_clip_rect(x, cy, max_x - x, clh)
    renderer.draw_text(fonts.code, line:sub(1, -2), x, cy, style.syntax["string"])
    core.pop_clip_rect()
    cy = cy + clh
  end
end

local function draw_rule(x, y, max_x, lh)
  local mid = math.floor(y + lh / 4)
  renderer.draw_rect(x, mid, max_x - x, 1, style.divider)
end

local function draw_blockquote(view, blk, x, y, max_x, fonts, lh, gap)
  local sx     = x + 14
  local start_y = y
  local cur_y  = y + math.floor(gap / 4)
  for _, sub in ipairs(blk.blocks) do
    local sh = layout.block_height(sub, max_x - sx, fonts, gap)
    M.draw_block(view, sub, sx, cur_y, max_x, fonts, lh, gap)
    cur_y = cur_y + sh + math.floor(gap / 2)
  end
  renderer.draw_rect(x, start_y, 3, cur_y - start_y, style.syntax["comment"])
end

local function draw_list(view, blk, x, y, max_x, fonts, lh)
  local cx    = x + 20
  local cur_y = y
  for i, item in ipairs(blk.items) do
    local bullet = blk.ordered and (tostring(blk.start + i - 1) .. ".") or "\xE2\x80\xA2"
    renderer.draw_text(fonts.body, bullet, x + 4, cur_y, style.text)
    local ih = layout.inlines_height(item, max_x - cx, fonts)
    core.push_clip_rect(cx, cur_y, max_x - cx, ih + 2)
    M.draw_inlines(view, item, cx, cur_y, max_x, fonts)
    core.pop_clip_rect()
    cur_y = cur_y + ih + 2
  end
end

local function draw_table(view, blk, x, y, max_x, fonts, lh, gap)
  local n_cols  = #blk.alignments
  if n_cols == 0 then return end
  local total_w = max_x - x
  local col_w   = math.floor(total_w / n_cols)
  local row_h   = lh + gap
  local pad     = 6

  local function draw_row(cells, ry, is_header)
    local cx = x
    for i = 1, n_cols do
      local cell = cells[i]
      if is_header then
        renderer.draw_rect(cx, ry, col_w, row_h, style.line_highlight)
      end
      if cell then
        core.push_clip_rect(cx + pad, ry, col_w - pad * 2, row_h)
        local col = is_header and style.syntax["keyword"] or style.text
        M.draw_inlines(view, cell, cx + pad, ry + math.floor(gap / 2), cx + col_w - pad, fonts, nil, col)
        core.pop_clip_rect()
      end
      renderer.draw_rect(cx + col_w, ry, 1, row_h, style.divider)
      cx = cx + col_w
    end
    renderer.draw_rect(x, ry + row_h, total_w, 1, style.divider)
  end

  renderer.draw_rect(x, y, total_w, 1, style.divider)
  local cur_y = y + 1
  if #blk.head > 0 then
    draw_row(blk.head, cur_y, true)
    cur_y = cur_y + row_h + 1
    renderer.draw_rect(x, cur_y, total_w, 2, style.divider)
    cur_y = cur_y + 2
  end
  for _, row in ipairs(blk.rows) do
    draw_row(row, cur_y, false)
    cur_y = cur_y + row_h + 1
  end
end

-- ── Dispatch ─────────────────────────────────────────────────────────────────

function M.draw_block(view, blk, x, y, max_x, fonts, lh, gap)
  local t = blk.type
  if     t == "heading"    then draw_heading(view, blk, x, y, max_x, fonts, gap)
  elseif t == "paragraph"  then draw_paragraph(view, blk, x, y, max_x, fonts)
  elseif t == "code_block" then draw_code_block(view, blk, x, y, max_x, fonts, gap)
  elseif t == "rule"       then draw_rule(x, y, max_x, lh)
  elseif t == "blockquote" then draw_blockquote(view, blk, x, y, max_x, fonts, lh, gap)
  elseif t == "list"       then draw_list(view, blk, x, y, max_x, fonts, lh)
  elseif t == "table"      then draw_table(view, blk, x, y, max_x, fonts, lh, gap)
  end
end

return M
