-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Haskell",
  files = { "%.hs$", "%.lhs$" },
  comment = "%-%-",
  block_comment = { "{%-", "%-%}" },
  patterns = {
    { pattern = "%-%-.*",                             type = "comment"  },
    { pattern = { "{%-", "%-%}" },                    type = "comment"  },
    { pattern = { '"', '"', '\\' },                    type = "string"   },
    { pattern = { "'", "'", '\\' },                   type = "string"   },
    { pattern = "[%+%-]?%d+%.?%d*[eE]?[%+%-]?%d*",    type = "number"   },
    { pattern = "::|->|=>|<-",                        type = "operator" },
    { pattern = "[%[%]%(%){}:,=|\\\\<>/%*%+%-%.]+",   type = "operator" },
    { pattern = "[%u][%w_']*",                        type = "keyword2" },
    { pattern = "[%a_][%w_']*%f[%s%(]",               type = "function" },
    { pattern = "[%a_][%w_']*",                       type = "symbol"   },
  },
  symbols = {
    ["case"] = "keyword", ["class"] = "keyword", ["data"] = "keyword",
    ["default"] = "keyword", ["deriving"] = "keyword", ["do"] = "keyword",
    ["else"] = "keyword", ["foreign"] = "keyword", ["if"] = "keyword",
    ["import"] = "keyword", ["in"] = "keyword", ["infix"] = "keyword",
    ["infixl"] = "keyword", ["infixr"] = "keyword", ["instance"] = "keyword",
    ["let"] = "keyword", ["module"] = "keyword", ["newtype"] = "keyword",
    ["of"] = "keyword", ["then"] = "keyword", ["type"] = "keyword",
    ["where"] = "keyword", ["qualified"] = "keyword", ["as"] = "keyword",
    ["hiding"] = "keyword", ["family"] = "keyword", ["forall"] = "keyword",
    ["mdo"] = "keyword", ["proc"] = "keyword", ["rec"] = "keyword",
    ["True"] = "literal", ["False"] = "literal", ["Nothing"] = "literal",
    ["Just"] = "literal",
  },
}
