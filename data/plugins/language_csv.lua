-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "CSV",
  files = { "%.csv$", "%.tsv$" },
  patterns = {
    { pattern = { '"', '"', '""' },                 type = "string"   },
    { pattern = "[%+%-]?%d+%.?%d*[eE]?[%+%-]?%d*", type = "number"   },
    { pattern = "[,;\t]",                          type = "operator" },
    { pattern = "[^,;\t\r\n\"]+",                  type = "symbol"   },
  },
  symbols = {},
}
