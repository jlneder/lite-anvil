local json = {}

local escape_map = {
  ["\\"] = "\\\\",
  ['"'] = '\\"',
  ["\b"] = "\\b",
  ["\f"] = "\\f",
  ["\n"] = "\\n",
  ["\r"] = "\\r",
  ["\t"] = "\\t",
}

local function is_array(value)
  if type(value) ~= "table" then
    return false
  end
  local count = 0
  for key in pairs(value) do
    if type(key) ~= "number" or key < 1 or key % 1 ~= 0 then
      return false
    end
    count = count + 1
  end
  for i = 1, count do
    if value[i] == nil then
      return false
    end
  end
  return true
end

local function encode_string(value)
  return '"' .. value:gsub('[%z\1-\31\\"]', function(char)
    return escape_map[char] or string.format("\\u%04x", char:byte())
  end) .. '"'
end

local function encode_value(value)
  local value_type = type(value)
  if value_type == "nil" then
    return "null"
  end
  if value_type == "boolean" or value_type == "number" then
    return tostring(value)
  end
  if value_type == "string" then
    return encode_string(value)
  end
  if value_type ~= "table" then
    error("unsupported json value: " .. value_type)
  end

  if is_array(value) then
    local items = {}
    for i = 1, #value do
      items[i] = encode_value(value[i])
    end
    return "[" .. table.concat(items, ",") .. "]"
  end

  local items = {}
  for key, item in pairs(value) do
    if item ~= nil then
      items[#items + 1] = encode_string(tostring(key)) .. ":" .. encode_value(item)
    end
  end
  return "{" .. table.concat(items, ",") .. "}"
end

function json.encode(value)
  return encode_value(value)
end

function json.encode_safe(value)
  return pcall(json.encode, value)
end

local function decode_error(state, message)
  error(string.format("json decode error at %d: %s", state.pos, message))
end

local function skip_ws(state)
  local text = state.text
  local pos = state.pos
  while true do
    local char = text:sub(pos, pos)
    if char == "" or not char:match("%s") then
      break
    end
    pos = pos + 1
  end
  state.pos = pos
end

local function decode_string(state)
  local text = state.text
  local pos = state.pos + 1
  local out = {}
  while true do
    local char = text:sub(pos, pos)
    if char == "" then
      decode_error(state, "unterminated string")
    elseif char == '"' then
      state.pos = pos + 1
      return table.concat(out)
    elseif char == "\\" then
      local esc = text:sub(pos + 1, pos + 1)
      if esc == "" then
        decode_error(state, "unterminated escape")
      elseif esc == "u" then
        local hex = text:sub(pos + 2, pos + 5)
        if not hex:match("^%x%x%x%x$") then
          decode_error(state, "invalid unicode escape")
        end
        local code = tonumber(hex, 16)
        local consumed = 6
        if code >= 0xD800 and code <= 0xDBFF and text:sub(pos + 6, pos + 7) == "\\u" then
          local low_hex = text:sub(pos + 8, pos + 11)
          if low_hex:match("^%x%x%x%x$") then
            local low = tonumber(low_hex, 16)
            if low >= 0xDC00 and low <= 0xDFFF then
              code = 0x10000 + ((code - 0xD800) * 0x400) + (low - 0xDC00)
              consumed = 12
            end
          end
        end
        out[#out + 1] = string.uchar(code)
        pos = pos + consumed
      else
        local replacements = {
          ['"'] = '"',
          ["\\"] = "\\",
          ["/"] = "/",
          b = "\b",
          f = "\f",
          n = "\n",
          r = "\r",
          t = "\t",
        }
        if not replacements[esc] then
          decode_error(state, "invalid escape sequence")
        end
        out[#out + 1] = replacements[esc]
        pos = pos + 2
      end
    else
      out[#out + 1] = char
      pos = pos + 1
    end
  end
end

local decode_value

local function decode_array(state)
  local res = {}
  state.pos = state.pos + 1
  skip_ws(state)
  if state.text:sub(state.pos, state.pos) == "]" then
    state.pos = state.pos + 1
    return res
  end
  while true do
    res[#res + 1] = decode_value(state)
    skip_ws(state)
    local char = state.text:sub(state.pos, state.pos)
    if char == "]" then
      state.pos = state.pos + 1
      return res
    elseif char ~= "," then
      decode_error(state, "expected ',' or ']'")
    end
    state.pos = state.pos + 1
    skip_ws(state)
  end
end

local function decode_object(state)
  local res = {}
  state.pos = state.pos + 1
  skip_ws(state)
  if state.text:sub(state.pos, state.pos) == "}" then
    state.pos = state.pos + 1
    return res
  end
  while true do
    if state.text:sub(state.pos, state.pos) ~= '"' then
      decode_error(state, "expected string key")
    end
    local key = decode_string(state)
    skip_ws(state)
    if state.text:sub(state.pos, state.pos) ~= ":" then
      decode_error(state, "expected ':' after key")
    end
    state.pos = state.pos + 1
    skip_ws(state)
    res[key] = decode_value(state)
    skip_ws(state)
    local char = state.text:sub(state.pos, state.pos)
    if char == "}" then
      state.pos = state.pos + 1
      return res
    elseif char ~= "," then
      decode_error(state, "expected ',' or '}'")
    end
    state.pos = state.pos + 1
    skip_ws(state)
  end
end

local function decode_number(state)
  local start_pos = state.pos
  local number = state.text:sub(start_pos):match("^-?%d+%.?%d*[eE]?[+-]?%d*")
  if not number or number == "" then
    decode_error(state, "invalid number")
  end
  state.pos = start_pos + #number
  return tonumber(number)
end

decode_value = function(state)
  skip_ws(state)
  local char = state.text:sub(state.pos, state.pos)
  if char == '"' then
    return decode_string(state)
  elseif char == "{" then
    return decode_object(state)
  elseif char == "[" then
    return decode_array(state)
  elseif char == "-" or char:match("%d") then
    return decode_number(state)
  elseif state.text:sub(state.pos, state.pos + 3) == "true" then
    state.pos = state.pos + 4
    return true
  elseif state.text:sub(state.pos, state.pos + 4) == "false" then
    state.pos = state.pos + 5
    return false
  elseif state.text:sub(state.pos, state.pos + 3) == "null" then
    state.pos = state.pos + 4
    return nil
  end
  decode_error(state, "unexpected token")
end

function json.decode(text)
  local state = { text = text, pos = 1 }
  local value = decode_value(state)
  skip_ws(state)
  if state.pos <= #text then
    decode_error(state, "trailing content")
  end
  return value
end

function json.decode_safe(text)
  return pcall(json.decode, text)
end

return json
