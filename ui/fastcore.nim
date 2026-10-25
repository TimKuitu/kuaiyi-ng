## 快译NG C 核心绑定（源码直接参与编译，单一 exe）


{.compile: "../core/str.c".}
{.compile: "../core/http.c".}
{.compile: "../core/json.c".}
{.compile: "../core/bing.c".}
{.compile: "../core/cfg.c".}
{.compile: "../core/hist.c".}
{.compile: "../core/fmt.c".}

proc fy_free*(p: pointer) {.importc.}
proc fy_strdup*(s: cstring): cstring {.importc.}
proc fy_last_error*(): cstring {.importc.}
proc fy_translate*(text: cstring, mode: cint): cstring {.importc.}
proc fy_format_word*(text: cstring, fmt: cint): cstring {.importc.}
proc fy_cfg_open*(path: cstring): pointer {.importc.}
proc fy_cfg_get*(cfg: pointer, sec: cstring, key: cstring, def: cstring,
    buf: cstring, cap: cint): cint {.importc.}
proc fy_cfg_set*(cfg: pointer, sec: cstring, key: cstring, value: cstring) {.importc.}
proc fy_cfg_save*(cfg: pointer): cint {.importc.}
proc fy_cfg_close*(cfg: pointer) {.importc.}
proc fy_hist_open*(dir: cstring): pointer {.importc.}
proc fy_hist_count*(h: pointer): cint {.importc.}
proc fy_hist_get*(h: pointer, index: cint): cstring {.importc.}
proc fy_hist_push*(h: pointer, text: cstring) {.importc.}
proc fy_hist_close*(h: pointer) {.importc.}

# 常量（与 fastcore.h 一致）
const
  FY_AUTO* = 0
  FY_ZH2EN* = 1
  FY_EN2ZH* = 2
  FY_FMT_RAW* = 0
  FY_FMT_LOWER* = 1
  FY_FMT_UPPER* = 2
  FY_FMT_CAMEL* = 3
  FY_FMT_SNAKE* = 4
  FY_FMT_KEBAB* = 5
