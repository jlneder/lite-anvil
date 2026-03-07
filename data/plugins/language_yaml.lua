-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "YAML",
  files = { "%.yaml$", "%.yml$" },
  comment = "#",
  patterns = {
    { pattern = "#.*",                              type = "comment"  },
    { pattern = "^%-%-%-",                          type = "keyword"  },
    { pattern = "^%.%.%.",                          type = "keyword"  },
    { pattern = { '"', '"', '\\' },                 type = "string"   },
    { pattern = { "'", "'" },                       type = "string"   },
    { pattern = "&[%a_][%w_%-]*",                   type = "keyword2" },
    { pattern = "%*[%a_][%w_%-]*",                  type = "keyword2" },
    { pattern = "!![%a_][%w_%-%.]*",                type = "keyword"  },
    { pattern = "![%a_][%w_%-%.]*",                 type = "keyword"  },
    { pattern = "^%s*()[%a_][%w_%-%.]*()%s*():",    type = { "normal", "function", "normal", "operator" } },
    { pattern = "()[%a_][%w_%-%.]*()%s*():",        type = { "normal", "function", "normal", "operator" } },
    { pattern = "0x[%da-fA-F]+",                   type = "number"   },
    { pattern = "0o[0-7]+",                         type = "number"   },
    { pattern = "[%+%-]?%d+%.?%d*[eE]?[%+%-]?%d*", type = "number"   },
    { pattern = "[|>][%-+]?",                       type = "operator" },
    { pattern = "[-:,%[%]{}]",                      type = "operator" },
    { pattern = "[%a_][%w_%-%.]*",                  type = "symbol"   },
  },
  symbols = {
    ["true"]  = "literal",
    ["false"] = "literal",
    ["yes"]   = "literal",
    ["no"]    = "literal",
    ["on"]    = "literal",
    ["off"]   = "literal",
    ["null"]  = "literal",
    ["~"]     = "literal",
  },
}
