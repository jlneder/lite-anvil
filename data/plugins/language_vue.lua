-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Vue",
  files = { "%.vue$" },
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
    { pattern = { "{{", "}}" },                      type = "keyword2" },
    { pattern = { '"', '"', '\\' },                  type = "string"   },
    { pattern = { "'", "'" },                        type = "string"   },
    { pattern = "%f[^<]![%a_][%w_]*",                type = "keyword2" },
    { pattern = "%f[^<]/[%a_][%w_%-]*",              type = "function" },
    { pattern = "%f[^<][%a_][%w_%-]*",               type = "function" },
    { pattern = "v%-%a[%w%-]*",                      type = "keyword"  },
    { pattern = ":[%a_][%w_%-]*",                    type = "keyword2" },
    { pattern = "@[%a_][%w_%-]*",                    type = "keyword2" },
    { pattern = "[/<>=]",                            type = "operator" },
    { pattern = "[%a_][%w_%-]*",                     type = "symbol"   },
  },
  symbols = {
    ["template"] = "keyword", ["script"] = "keyword", ["style"] = "keyword",
    ["setup"] = "keyword", ["scoped"] = "keyword", ["slot"] = "keyword2",
  },
}
