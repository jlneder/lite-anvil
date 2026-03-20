local View = require "core.view"
local ContextMenu = require "core.contextmenu"
local native_docview = require "docview_native"

---@class core.docview : core.view
---@field super core.view
local DocView = View:extend()

function DocView:__tostring() return "DocView" end

DocView.context = "session"
DocView._context_menu_divider = ContextMenu.DIVIDER

native_docview.populate(DocView)

return DocView
