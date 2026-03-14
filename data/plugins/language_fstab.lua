-- mod-version:4
local syntax = require "core.syntax"

syntax.add {
  name = "fstab",
  files = { "fstab$", "mtab$" },
  comment = "#",
  patterns = {
    { pattern = "#.*",                               type = "comment"  },
    { pattern = "UUID=[^%s#]+",                      type = "keyword2" },
    { pattern = "LABEL=[^%s#]+",                     type = "keyword2" },
    { pattern = "/[^%s#]+",                          type = "string"   },
    { pattern = "[%+%-]?%d+",                        type = "number"   },
    { pattern = "[,%-]",                             type = "operator" },
    { pattern = "[%a_][%w_%-%.]*",                   type = "symbol"   },
  },
  symbols = {
    ["defaults"] = "keyword", ["ro"] = "keyword", ["rw"] = "keyword",
    ["user"] = "keyword", ["users"] = "keyword", ["auto"] = "keyword",
    ["noauto"] = "keyword", ["exec"] = "keyword", ["noexec"] = "keyword",
    ["suid"] = "keyword", ["nosuid"] = "keyword", ["dev"] = "keyword",
    ["nodev"] = "keyword", ["sync"] = "keyword", ["async"] = "keyword",
    ["ext2"] = "keyword2", ["ext3"] = "keyword2", ["ext4"] = "keyword2",
    ["xfs"] = "keyword2", ["btrfs"] = "keyword2", ["vfat"] = "keyword2",
    ["ntfs"] = "keyword2", ["nfs"] = "keyword2", ["tmpfs"] = "keyword2",
    ["swap"] = "keyword2", ["proc"] = "keyword2", ["sysfs"] = "keyword2",
  },
}
