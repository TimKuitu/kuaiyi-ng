## naming_reducer 单元测试：nim c -r ui/test_naming_reducer.nim
## 全部通过退出码 0，有失败退出码 1

import std/strutils
import naming_reducer

var gPass, gFail = 0

proc check(phrase: string, style: NamingStyle, mw, mc: int, expect: string) =
  let r = reduceName(phrase, style, maxWords = mw, maxChars = mc)
  if r.ok and r.name == expect:
    inc gPass
    echo "PASS  " & phrase & "  ->  " & r.name
  else:
    inc gFail
    let got = if r.ok: r.name else: "失败(" & r.reason & ")"
    echo "FAIL  " & phrase & "  ->  got " & got & "  want " & expect

proc checkFail(phrase: string, style: NamingStyle, mw, mc: int, reasonPart: string) =
  let r = reduceName(phrase, style, maxWords = mw, maxChars = mc)
  if (not r.ok) and r.reason.contains(reasonPart):
    inc gPass
    echo "PASS  " & phrase & "  ->  失败(" & r.reason & ")"
  else:
    inc gFail
    let got = if r.ok: r.name else: r.reason
    echo "FAIL  " & phrase & "  ->  got " & got & "  want 失败(" & reasonPart & ")"

# —— 中文场景（题目给的三个用例）——
# 注：读取/状态 这类核心词按设计保留，不会为凑短静默丢词
check("串口初始化程序", nsCamel, 4, 40, "serialPortInit")
check("用户登录状态缓存管理器", nsCamel, 4, 40, "userLoginStateCache")
check("配置文件读取路径", nsCamel, 4, 40, "configFileReadPath")

# —— 英文：泛词 + 缩写 + 预算裁剪 ——
check("The quick brown fox jumps over the lazy dog", nsSnake, 4, 40,
      "quick_brown_fox_jumps")
check("Serial Port Initialization Program", nsCamel, 4, 40, "serialPortInit")
check("get the maximum value of the buffer", nsCamel, 4, 40, "getMaxValBuf")
check(" temporary configuration file reader ", nsCamel, 4, 40,
      "tmpConfigFileReader")

# —— 混排与驼峰拆词 ——
check("TCP/IP 网络连接管理器", nsPascal, 4, 40, "TcpIpNetworkConn")
check("parseXMLDocument configuration", nsCamel, 4, 40, "parseXmlDocConfig")
check("HTTP 服务器状态", nsCamel, 4, 40, "httpServerState")

# —— 其它风格 ——
check("数据库连接池", nsCamel, 3, 40, "dbConnPool")
check("配置文件读取路径", nsKebab, 4, 40, "config-file-read-path")

# —— 保留字冲突：加下划线后缀，不静默截断 ——
check("class", nsSnake, 4, 40, "class_")

# —— 失败标记 ——
checkFail("the of and", nsCamel, 4, 40, "全部为泛词或未收录词")
checkFail("", nsCamel, 4, 40, "空输入")
checkFail("internationalization configuration", nsCamel, 4, 10, "超出预算")

# —— validateIdent 直测 ——
block:
  var v = 0
  if validateIdent("userLogin_2", reservedSet): inc v else: inc gFail
  if not validateIdent("2fast", reservedSet): inc v else: inc gFail
  if not validateIdent("If", reservedSet): inc v else: inc gFail
  if validateIdent("config-file-path", reservedSet): inc v else: inc gFail
  gPass += v
  echo "PASS  validateIdent 直测 " & $v & "/4"

echo "----------------------------------------"
echo "总计: " & $gPass & " 通过, " & $gFail & " 失败"
if gFail > 0:
  echo "有测试未通过"
  quit(1)
echo "全部通过"
