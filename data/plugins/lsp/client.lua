local core = require "core"
local protocol = require "..protocol"
local native_transport = nil

do
  local ok, mod = pcall(require, "lsp_transport")
  if ok then
    native_transport = mod
  end
end

local Client = {}
Client.__index = Client

local function default_options(root_dir, env)
  return {
    cwd = root_dir,
    stdin = process.REDIRECT_PIPE,
    stdout = process.REDIRECT_PIPE,
    stderr = process.REDIRECT_PIPE,
    env = env,
  }
end

function Client.new(name, spec, root_dir, handlers)
  local proc = nil
  local transport_id = nil
  if native_transport then
    local ok, id = pcall(native_transport.spawn, spec.command, root_dir, spec.env)
    if ok and id then
      transport_id = id
    else
      return nil, id or "failed to start language server"
    end
  else
    local ok, err
    ok, proc, err = pcall(process.start, spec.command, default_options(root_dir, spec.env))
    if not ok or not proc then
      return nil, err or "failed to start language server"
    end
  end

  local self = setmetatable({
    name = name,
    spec = spec,
    root_dir = root_dir,
    process = proc,
    transport_id = transport_id,
    handlers = handlers or {},
    next_request_id = 0,
    pending = {},
    pre_init_queue = {},
    is_initialized = false,
    is_shutting_down = false,
    capabilities = {},
    stdout_buffer = "",
    outgoing = {},
  }, Client)

  self:start_writer()
  self:start_reader()
  return self
end

function Client:is_running()
  if self.transport_id and native_transport then
    local ok, state = pcall(native_transport.poll, self.transport_id, 0)
    return ok and state and state.running
  end
  return self.process and self.process:running()
end

function Client:start_reader()
  core.add_thread(function()
    while true do
      local had_output = false

      if self.transport_id and native_transport then
        local ok, polled = pcall(native_transport.poll, self.transport_id, 64)
        if ok and polled then
          if polled.messages and #polled.messages > 0 then
            had_output = true
            for _, message in ipairs(polled.messages) do
              self:handle_message(message)
            end
          end
          if polled.stderr and #polled.stderr > 0 then
            had_output = true
            for _, line in ipairs(polled.stderr) do
              core.log_quiet("LSP %s stderr: %s", self.name, tostring(line):gsub("%s+$", ""))
            end
          end
          if not polled.running and (not polled.messages or #polled.messages == 0)
              and (not polled.stderr or #polled.stderr == 0) then
            break
          end
        end
        if not had_output then
          coroutine.yield(0.05)
        end
        goto continue
      end

      if not (self.process and self.process:running()) then
        break
      end

      local stdout = self.process:read_stdout(4096)
      if stdout and #stdout > 0 then
        had_output = true
        self.stdout_buffer = self.stdout_buffer .. stdout
        local ok, messages, remaining = pcall(protocol.decode_messages, self.stdout_buffer)
        if ok then
          self.stdout_buffer = remaining
          for _, message in ipairs(messages) do
            self:handle_message(message)
          end
        else
          core.warn("LSP %s decode error: %s", self.name, messages)
          self.stdout_buffer = ""
        end
      end

      local stderr = self.process:read_stderr(4096)
      if stderr and #stderr > 0 then
        had_output = true
        core.log_quiet("LSP %s stderr: %s", self.name, stderr:gsub("%s+$", ""))
      end

      if not had_output then
        coroutine.yield(0.05)
      end
      ::continue::
    end

    if self.handlers.on_exit then
      core.try(self.handlers.on_exit, self)
    end
  end)
end

function Client:start_writer()
  if self.transport_id and native_transport then
    return
  end
  core.add_thread(function()
    while self.process and self.process:running() do
      local payload = table.remove(self.outgoing, 1)
      if payload then
        local _, err = self.process.stdin:write(payload, { scan = 0.01 })
        if err then
          core.warn("LSP %s write failed: %s", self.name, err)
        end
      else
        coroutine.yield(0.01)
      end
    end
  end)
end

function Client:send(message)
  if self.transport_id and native_transport then
    return pcall(native_transport.send, self.transport_id, message)
  end
  if not self.process then
    return false, "LSP transport unavailable"
  end
  local payload, err = protocol.encode_message(message)
  if not payload then
    return false, err
  end
  self.outgoing[#self.outgoing + 1] = payload
  return true
end

function Client:queue_or_send(message, bypass_init)
  if not bypass_init and not self.is_initialized then
    self.pre_init_queue[#self.pre_init_queue + 1] = message
    return true
  end
  return self:send(message)
end

function Client:notify(method, params, bypass_init)
  return self:queue_or_send({
    jsonrpc = "2.0",
    method = method,
    params = params,
  }, bypass_init)
end

function Client:request(method, params, callback, bypass_init)
  self.next_request_id = self.next_request_id + 1
  local id = self.next_request_id
  if callback then
    self.pending[id] = callback
  end
  return self:queue_or_send({
    jsonrpc = "2.0",
    id = id,
    method = method,
    params = params,
  }, bypass_init)
end

function Client:flush_pre_init_queue()
  local queued = self.pre_init_queue
  self.pre_init_queue = {}
  for _, message in ipairs(queued) do
    self:send(message)
  end
end

function Client:initialize(params, on_ready)
  self:request("initialize", params, function(result, err)
    if err then
      core.warn("LSP %s initialize failed: %s", self.name, err.message or tostring(err))
      return
    end
    self.capabilities = result and result.capabilities or {}
    self.is_initialized = true
    self:notify("initialized", {}, true)
    self:flush_pre_init_queue()
    if on_ready then
      core.try(on_ready, self, result)
    end
  end, true)
end

function Client:handle_message(message)
  if message.id ~= nil then
    local callback = self.pending[message.id]
    self.pending[message.id] = nil
    if callback then
      core.try(callback, message.result, message.error, message)
    end
    return
  end

  if self.handlers.on_notification and message.method then
    core.try(self.handlers.on_notification, self, message)
  end
end

function Client:shutdown()
  if self.is_shutting_down or (not self.process and not self.transport_id) then
    return
  end
  self.is_shutting_down = true
  self:request("shutdown", nil, function()
    self:notify("exit", nil, true)
    if self.transport_id and native_transport then
      native_transport.terminate(self.transport_id)
      native_transport.remove(self.transport_id)
    else
      self.process:terminate()
    end
  end, true)
end

return Client
