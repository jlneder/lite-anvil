-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "TOML",
  files = { "%.toml$" },
  comment = "#",
  patterns = {
    { pattern = "#.*",                              type = "comment"  },
    { pattern = "%[%[.-%]%]",                       type = "keyword"  },
    { pattern = "%[.-%]",                           type = "keyword"  },
    { pattern = { "'''", "'''" },                   type = "string"   },
    { pattern = { '"""', '"""', '\\' },             type = "string"   },
    { pattern = { '"', '"', '\\' },                 type = "string"   },
    { pattern = { "'", "'" },                       type = "string"   },
    { pattern = "%d%d%d%d%-%d%d%-%d%d[T%s]?%d*:?%d*:?%d*",
                                                    type = "number"   },
    { pattern = "0x[%da-fA-F_]+",                  type = "number"   },
    { pattern = "0o[0-7_]+",                        type = "number"   },
    { pattern = "0b[01_]+",                         type = "number"   },
    { pattern = "[%+%-]?%d[%d_]*%.?[%d_]*[eE]?[%+%-]?[%d_]*",
                                                    type = "number"   },
    { pattern = "[%a_][%w_%-%.]*()%s*()=",          type = { "function", "normal", "operator" } },
    { pattern = "=",                                type = "operator" },
    { pattern = "[%a_][%w_%-]*",                    type = "symbol"   },
  },
  symbols = {
    ["true"]  = "literal",
    ["false"] = "literal",
    ["inf"]   = "literal",
    ["nan"]   = "literal",
  },
}
