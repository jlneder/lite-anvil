-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Lisp",
  files = { "%.lisp$", "%.lsp$", "%.cl$", "%.el$" },
  comment = ";",
  patterns = {
    { pattern = ";.*",                               type = "comment"  },
    { pattern = { '"', '"', '\\' },                   type = "string"   },
    { pattern = "[%+%-]?%d+%.?%d*",                  type = "number"   },
    { pattern = "[`',]",                             type = "operator" },
    { pattern = "[%(%)%[%]{}]",                      type = "operator" },
    { pattern = ":[%a_][%w_%-]*",                    type = "keyword2" },
    { pattern = "[%a_][%w_%-%*%!%?<>/=+]*",          type = "symbol"   },
  },
  symbols = {
    ["defun"] = "keyword", ["lambda"] = "keyword", ["let"] = "keyword",
    ["let*"] = "keyword", ["labels"] = "keyword", ["flet"] = "keyword",
    ["if"] = "keyword", ["cond"] = "keyword", ["case"] = "keyword",
    ["when"] = "keyword", ["unless"] = "keyword", ["progn"] = "keyword",
    ["setq"] = "keyword", ["setf"] = "keyword", ["quote"] = "keyword",
    ["function"] = "keyword", ["loop"] = "keyword", ["do"] = "keyword",
    ["dolist"] = "keyword", ["dotimes"] = "keyword", ["defmacro"] = "keyword",
    ["defvar"] = "keyword", ["defparameter"] = "keyword", ["defclass"] = "keyword",
    ["defmethod"] = "keyword", ["t"] = "literal", ["nil"] = "literal",
  },
}
