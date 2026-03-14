-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Meson",
  files = { "meson%.build$", "meson_options%.txt$" },
  comment = "#",
  patterns = {
    { pattern = "#.*",                               type = "comment"  },
    { pattern = { '"', '"', '\\' },                   type = "string"   },
    { pattern = { "'", "'" },                        type = "string"   },
    { pattern = "%d[%d_]*%.?[%d_]*",                 type = "number"   },
    { pattern = "[%+%-=/%*%%<>!~|&%^%?%.:,()%[%]{}]+", type = "operator" },
    { pattern = "[%a_][%w_]*%f[(]",                  type = "function" },
    { pattern = "[%a_][%w_]*",                       type = "symbol"   },
  },
  symbols = {
    ["if"] = "keyword", ["elif"] = "keyword", ["else"] = "keyword",
    ["endif"] = "keyword", ["foreach"] = "keyword", ["endforeach"] = "keyword",
    ["break"] = "keyword", ["continue"] = "keyword", ["and"] = "keyword",
    ["or"] = "keyword", ["not"] = "keyword", ["true"] = "literal",
    ["false"] = "literal",
  },
}
