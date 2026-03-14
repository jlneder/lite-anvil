-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "OCaml",
  files = { "%.ml$", "%.mli$", "%.mll$", "%.mly$" },
  block_comment = { "%(%*", "%*%)" },
  patterns = {
    { pattern = { "%(%*", "%*%)" },                  type = "comment"  },
    { pattern = { '"', '"', '\\' },                   type = "string"   },
    { pattern = { "'", "'", '\\' },                  type = "string"   },
    { pattern = "[%+%-]?%d+%.?%d*[eE]?[%+%-]?%d*",   type = "number"   },
    { pattern = "->|=>|::|:=|;;|@@|%|>",             type = "operator" },
    { pattern = "[%[%]%(%){};,.:~?=<>/%*%+%-|&]+",   type = "operator" },
    { pattern = "[%u][%w_']*",                       type = "keyword2" },
    { pattern = "[%a_][%w_']*%f[(]",                 type = "function" },
    { pattern = "[%a_][%w_']*",                      type = "symbol"   },
  },
  symbols = {
    ["and"] = "keyword", ["as"] = "keyword", ["assert"] = "keyword",
    ["begin"] = "keyword", ["class"] = "keyword", ["constraint"] = "keyword",
    ["do"] = "keyword", ["done"] = "keyword", ["downto"] = "keyword",
    ["else"] = "keyword", ["end"] = "keyword", ["exception"] = "keyword",
    ["external"] = "keyword", ["for"] = "keyword", ["fun"] = "keyword",
    ["function"] = "keyword", ["functor"] = "keyword", ["if"] = "keyword",
    ["in"] = "keyword", ["include"] = "keyword", ["inherit"] = "keyword",
    ["initializer"] = "keyword", ["lazy"] = "keyword", ["let"] = "keyword",
    ["match"] = "keyword", ["method"] = "keyword", ["module"] = "keyword",
    ["mutable"] = "keyword", ["new"] = "keyword", ["object"] = "keyword",
    ["of"] = "keyword", ["open"] = "keyword", ["or"] = "keyword",
    ["private"] = "keyword", ["rec"] = "keyword", ["sig"] = "keyword",
    ["struct"] = "keyword", ["then"] = "keyword", ["to"] = "keyword",
    ["try"] = "keyword", ["type"] = "keyword", ["val"] = "keyword",
    ["virtual"] = "keyword", ["when"] = "keyword", ["while"] = "keyword",
    ["with"] = "keyword", ["true"] = "literal", ["false"] = "literal",
  },
}
