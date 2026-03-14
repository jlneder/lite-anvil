local json = require "..json"

local protocol = {}

protocol.completion_kinds = {
  [1] = "keyword2",
  [2] = "function",
  [3] = "function",
  [4] = "keyword2",
  [5] = "keyword2",
  [6] = "keyword2",
  [7] = "keyword2",
  [8] = "keyword2",
  [9] = "keyword2",
  [10] = "keyword2",
  [11] = "literal",
  [12] = "function",
  [13] = "keyword",
  [14] = "keyword",
  [15] = "string",
  [16] = "keyword",
  [17] = "file",
  [18] = "keyword",
  [19] = "keyword",
  [20] = "keyword2",
  [21] = "literal",
  [22] = "keyword2",
  [23] = "operator",
  [24] = "keyword",
  [25] = "keyword",
}

function protocol.encode_message(message)
  local body = json.encode(message)
  return string.format("Content-Length: %d\r\n\r\n%s", #body, body)
end

function protocol.decode_messages(buffer)
  local messages = {}
  while true do
    local header_end = buffer:find("\r\n\r\n", 1, true)
    if not header_end then
      break
    end
    local header = buffer:sub(1, header_end - 1)
    local content_length = tonumber(header:match("[Cc]ontent%-[Ll]ength:%s*(%d+)"))
    if not content_length then
      error("invalid LSP message without Content-Length")
    end
    local body_start = header_end + 4
    local body_end = body_start + content_length - 1
    if #buffer < body_end then
      break
    end
    messages[#messages + 1] = json.decode(buffer:sub(body_start, body_end))
    buffer = buffer:sub(body_end + 1)
  end
  return messages, buffer
end

return protocol
