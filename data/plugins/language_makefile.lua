-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "Makefile",
  files = { "[Mm]akefile$", "%.mk$", "GNUmakefile$", "%.mak$" },
  comment = "#",
  patterns = {
    { pattern = "#.*",                               type = "comment"  },
    { pattern = { '"', '"', '\\' },                   type = "string"   },
    { pattern = { "'", "'" },                        type = "string"   },
    { pattern = "%$%b()",                            type = "keyword2" },
    { pattern = "%${[^}]*}",                         type = "keyword2" },
    { pattern = "^%s*()[^:%s][^:]*():",              type = { "normal", "function", "operator" } },
    { pattern = "[|&;<>]",                           type = "operator" },
    { pattern = "[:?+!]?=",                          type = "operator" },
    { pattern = "[%a_][%w_%-%.]*",                   type = "symbol"   },
  },
  symbols = {
    ["ifeq"] = "keyword", ["ifneq"] = "keyword", ["ifdef"] = "keyword",
    ["ifndef"] = "keyword", ["else"] = "keyword", ["endif"] = "keyword",
    ["include"] = "keyword", ["override"] = "keyword", ["export"] = "keyword",
    ["unexport"] = "keyword", ["define"] = "keyword", ["endef"] = "keyword",
    ["vpath"] = "keyword", ["private"] = "keyword", ["undefine"] = "keyword",
  },
}
