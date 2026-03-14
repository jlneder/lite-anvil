-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Gleam",
  files = { "%.gleam$" },
  comment = "//",
  patterns = {
    { pattern = "//.*",                              type = "comment"  },
    { pattern = { "\"\"\"", "\"\"\"", '\\' },       type = "string"   },
    { pattern = { '"', '"', '\\' },                  type = "string"   },
    { pattern = "0x[%da-fA-F_]+",                    type = "number"   },
    { pattern = "%d[%d_]*%.?[%d_]*",                 type = "number"   },
    { pattern = "->|=>|<-|<<|>>",                    type = "operator" },
    { pattern = "[%+%-=/%*%%<>!~|&%^%?%.:,|]+",      type = "operator" },
    { pattern = "[%u][%w_]*",                        type = "keyword2" },
    { pattern = "[%a_][%w_]*%f[(]",                  type = "function" },
    { pattern = "[%a_][%w_]*",                       type = "symbol"   },
  },
  symbols = {
    ["as"] = "keyword", ["assert"] = "keyword", ["case"] = "keyword",
    ["const"] = "keyword", ["fn"] = "keyword", ["if"] = "keyword",
    ["import"] = "keyword", ["let"] = "keyword", ["opaque"] = "keyword",
    ["panic"] = "keyword", ["pub"] = "keyword", ["todo"] = "keyword",
    ["type"] = "keyword", ["use"] = "keyword", ["true"] = "literal",
    ["false"] = "literal", ["Nil"] = "literal",
  },
}
