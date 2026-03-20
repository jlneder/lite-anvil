
local os_exit = os.exit
os.exit = function(code, close)
    os_exit(code, close == nil and true or close)
end

local core
xpcall(function()
    core = require(os.getenv("LITE_ANVIL_RUNTIME") or "core")
    core.init()
    core.run()
end, function(err)
    io.stderr:write("Error: " .. tostring(err) .. "\n")
    io.stderr:write(debug.traceback(nil, 2) .. "\n")
    if core and core.on_error then
        pcall(core.on_error, err)
    end
end)

return core and core.restart_request
