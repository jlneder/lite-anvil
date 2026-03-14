-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Svelte",
  files = { "%.svelte$" },
  block_comment = { "<!--", "-->" },
  patterns = {
    {
      pattern = {
        "<%s*[sS][cC][rR][iI][pP][tT]%f[%s>].->",
        "<%s*/%s*[sS][cC][rR][iI][pP][tT]%s*>"
      },
      syntax = ".js",
      type = "function"
    },
    {
      pattern = {
        "<%s*[sS][tT][yY][lL][eE]%f[%s>].->",
        "<%s*/%s*[sS][tT][yY][lL][eE]%s*>"
      },
      syntax = ".css",
      type = "function"
    },
    { pattern = { "<!%-%-", "%-%->" },               type = "comment"  },
    { pattern = { "{#", "}" },                       type = "keyword"  },
    { pattern = { "{/", "}" },                       type = "keyword"  },
    { pattern = { "{:", "}" },                       type = "keyword"  },
    { pattern = { "{@", "}" },                       type = "keyword2" },
    { pattern = { "{", "}" },                        type = "keyword2" },
    { pattern = { '"', '"', '\\' },                  type = "string"   },
    { pattern = { "'", "'" },                        type = "string"   },
    { pattern = "%f[^<]/[%a_][%w_%-]*",              type = "function" },
    { pattern = "%f[^<][%a_][%w_%-]*",               type = "function" },
    { pattern = "[/<>=]",                            type = "operator" },
    { pattern = "[%a_][%w_%-]*",                     type = "symbol"   },
  },
  symbols = {
    ["if"] = "keyword", ["each"] = "keyword", ["await"] = "keyword",
    ["then"] = "keyword", ["catch"] = "keyword", ["html"] = "keyword2",
    ["debug"] = "keyword2", ["const"] = "keyword2", ["template"] = "keyword",
    ["script"] = "keyword", ["style"] = "keyword",
  },
}
