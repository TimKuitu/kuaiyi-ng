## naming_reducer 单元测试：nim c -r ui/test_naming_reducer.nim
## 全部通过退出码 0，有失败退出码 1

import std/[strutils, os]
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

proc checkK(phrase: string, style: NamingStyle, kind: NameKind,
            mw, mc: int, expect: string) =
  let r = reduceName(phrase, style, kind, maxWords = mw, maxChars = mc)
  if r.ok and r.name == expect:
    inc gPass
    echo "PASS  [" & $kind & "] " & phrase & "  ->  " & r.name
  else:
    inc gFail
    let got = if r.ok: r.name else: "失败(" & r.reason & ")"
    echo "FAIL  [" & $kind & "] " & phrase & "  ->  got " & got & "  want " & expect

proc checkFailK(phrase: string, style: NamingStyle, kind: NameKind,
                mw, mc: int, reasonPart: string) =
  let r = reduceName(phrase, style, kind, maxWords = mw, maxChars = mc)
  if (not r.ok) and r.reason.contains(reasonPart):
    inc gPass
    echo "PASS  [" & $kind & "] " & phrase & "  ->  失败(" & r.reason & ")"
  else:
    inc gFail
    let got = if r.ok: r.name else: r.reason
    echo "FAIL  [" & $kind & "] " & phrase & "  ->  got " & got &
         "  want 失败(" & reasonPart & ")"

# —— 中文场景（题目给的三个用例）——
# 注：读取/状态 这类核心词按设计保留，不会为凑短静默丢词
check("串口初始化程序", nsCamel, 4, 40, "uartInit")
check("用户登录状态缓存管理器", nsCamel, 4, 40, "userLoginStateCache")
check("配置文件读取路径", nsCamel, 4, 40, "configFileReadPath")

# —— 英文：泛词 + 缩写 + 预算裁剪 ——
check("The quick brown fox jumps over the lazy dog", nsSnake, 4, 40,
      "quick_brown_fox_jumps")
check("Serial Port Initialization Program", nsCamel, 4, 40, "serialPortInit")
check("get the maximum value of the buffer", nsCamel, 4, 40, "getMaxValueBuf")
check(" temporary configuration file reader ", nsCamel, 4, 40,
      "tmpConfigFileReader")

# —— 混排与驼峰拆词 ——
check("TCP/IP 网络连接管理器", nsPascal, 4, 40, "TcpIpNetworkConn")
check("parseXMLDocument configuration", nsCamel, 4, 40, "parseXmlDocConfig")
check("HTTP 服务器状态", nsCamel, 4, 40, "httpServerState")

# —— 其它风格 ——
check("数据库连接池", nsCamel, 3, 40, "dbConnPool")
check("配置文件读取路径", nsKebab, 4, 40, "config-file-read-path")

# —— 命名对象：C 惯例前缀/大小写（全局 g、静态 s、指针 p、宏全大写、类型 _t）——
checkK("led 初始化", nsSnake, nkFunc, 4, 40, "led_init")
checkK("led 初始化", nsCamel, nkFunc, 4, 40, "ledInit")
checkK("led 状态", nsSnake, nkLocal, 4, 40, "led_state")
checkK("led 状态", nsCamel, nkLocal, 4, 40, "ledState")
checkK("用户登录状态", nsSnake, nkGlobal, 4, 40, "g_user_login_state")
checkK("用户登录状态", nsCamel, nkGlobal, 4, 40, "gUserLoginState")
checkK("led 状态", nsSnake, nkStatic, 4, 40, "s_led_state")
checkK("led 状态", nsCamel, nkStatic, 4, 40, "sLedState")
checkK("数据缓冲区", nsSnake, nkPointer, 4, 40, "p_data_buf")
checkK("数据缓冲区", nsCamel, nkPointer, 4, 40, "pDataBuf")
checkK("最大长度", nsSnake, nkMacro, 4, 40, "MAX_LEN")
checkK("最大长度", nsCamel, nkMacro, 4, 40, "MAX_LEN")  # 宏不受风格影响，恒全大写
checkK("串口控制", nsSnake, nkType, 4, 40, "UartControl_t")
checkK("串口控制", nsCamel, nkType, 4, 40, "UartControl")
# 前缀计入预算：p_ + read 6 字符 > 5 → 报超出预算而不是硬塞
checkFailK("配置文件读取路径", nsSnake, nkPointer, 4, 5, "超出预算")

# —— 常用词库：中文优先映射到指定词，且库内词不删泛词、不缩写 ——
check("串口发送数据", nsSnake, 4, 40, "uart_send_data")
check("温度传感器", nsCamel, 4, 40, "tempSensor")
check("led 灯闪烁", nsSnake, 4, 40, "led_blink")
check("按键扫描", nsCamel, 4, 40, "keyScan")
check("看门狗喂狗", nsSnake, 4, 40, "wdt_feed_dog")
check("按钮", nsCamel, 4, 40, "button")            # 常用词不缩写成 btn
check("buttons", nsCamel, 4, 40, "buttons")        # 常用词的复数同样不缩写
check("数值是否超时", nsSnake, 4, 40, "value_is_timeout")  # is 不被当泛词删
check("电机正转", nsSnake, 4, 40, "motor_forward")
check("舵机角度", nsCamel, 4, 40, "servoAngle")
check("占空比", nsSnake, 4, 40, "duty")
check("高电平", nsSnake, 4, 40, "high_level")
check("外部中断", nsSnake, 4, 40, "exti")
check("定时器中断", nsCamel, 4, 40, "timerIrq")
check("频率周期", nsSnake, 4, 40, "freq_period")   # 频率直接给 freq
check("高级设置", nsCamel, 4, 40, "advancedSet")   # 复合词优先于单字"高"

# —— 保留字冲突：加下划线后缀，不静默截断 ——
check("class", nsSnake, 4, 40, "class_")

# —— 失败标记 ——
checkFail("the of and", nsCamel, 4, 40, "全部为泛词或未收录词")
checkFail("", nsCamel, 4, 40, "空输入")
# config 已是常用词库动词（裁剪优先保留），长名词先被丢，预算收到 5 才彻底装不下
checkFail("internationalization configuration", nsCamel, 4, 5, "超出预算")

# —— validateIdent 直测 ——
block:
  var v = 0
  if validateIdent("userLogin_2", reservedSet): inc v else: inc gFail
  if not validateIdent("2fast", reservedSet): inc v else: inc gFail
  if not validateIdent("If", reservedSet): inc v else: inc gFail
  if validateIdent("config-file-path", reservedSet): inc v else: inc gFail
  gPass += v
  echo "PASS  validateIdent 直测 " & $v & "/4"

# —— 自定义缩写：覆盖内置 / 坏行拒绝 / 清空恢复 ——
block:
  let tmp = getTempDir() / "ky_abbr_test.txt"
  removeFile(tmp)

  var v = 0
  # 无文件 → 纯内置：configuration→config
  let n0 = loadCustomAbbreviations(tmp)
  if n0 == 0 and reduceName("configuration reader", nsCamel, maxWords = 4, maxChars = 40).name == "configReader":
    inc v
  else: dec v; echo "  [自定义] 初始内置异常: n0=" & $n0

  # 覆盖内置：configuration→cfg；service 本是泛词，自定义后豁免删除并缩写
  let s1 = saveCustomAbbreviations(tmp, "configuration = cfg\nservice = svc\n")
  if s1.badLine == 0 and s1.count == 2 and
      reduceName("configuration reader", nsCamel, maxWords = 4, maxChars = 40).name == "cfgReader" and
      reduceName("service status", nsCamel, maxWords = 4, maxChars = 40).name == "svcStatus":
    inc v
  else: dec v; echo "  [自定义] 覆盖内置异常: " & $s1.count & "," & $s1.badLine

  # 坏行（缺 =）拒绝保存，原表不受影响
  let s2 = saveCustomAbbreviations(tmp, "novalue\nservice = svc\n")
  if s2.badLine == 1 and reduceName("configuration reader", nsCamel, maxWords = 4, maxChars = 40).name == "cfgReader":
    inc v
  else: dec v; echo "  [自定义] 坏行拒绝异常: badLine=" & $s2.badLine

  # 清空文本 → 恢复纯内置
  let s3 = saveCustomAbbreviations(tmp, "# 全删\n\n")
  if s3.badLine == 0 and s3.count == 0 and
      reduceName("configuration reader", nsCamel, maxWords = 4, maxChars = 40).name == "configReader":
    inc v
  else: dec v; echo "  [自定义] 清空恢复异常: " & $s3.count

  removeFile(tmp)
  if v == 4:
    inc gPass
    echo "PASS  自定义缩写 4/4（覆盖内置/坏行拒绝/清空恢复）"
  else:
    gFail += 4 - v
    echo "FAIL  自定义缩写 " & $v & "/4"

# —— 自定义忽略词：命中即删 / 优先于缩写 / 取数排序 / 清空恢复 ——
block:
  let tmpAbbr = getTempDir() / "ky_abbr_test2.txt"
  let tmpIgn = getTempDir() / "ky_ignore_test.txt"
  removeFile(tmpAbbr)
  removeFile(tmpIgn)
  discard loadCustomAbbreviations(tmpAbbr)
  discard loadCustomIgnore(tmpIgn)

  var v = 0
  # 无文件：status 是核心词，照旧保留
  let base = reduceName("user login status cache", nsCamel, maxWords = 4, maxChars = 40).name
  if base == "userLoginStatusCache": inc v
  else: dec v; echo "  [忽略] 基线异常: " & base

  # 加入忽略：status 被直接删掉
  let n1 = saveCustomIgnore(tmpIgn, "# 测试\nstatus\n\n")
  let r1 = reduceName("user login status cache", nsCamel, maxWords = 4, maxChars = 40).name
  if n1 == 1 and r1 == "userLoginCache": inc v
  else: dec v; echo "  [忽略] 生效异常: n=" & $n1 & " -> " & r1

  # 忽略优先于自定义缩写（service 两边都填 → 删掉）；取数按字母序
  discard saveCustomAbbreviations(tmpAbbr, "service = svc\n")
  discard saveCustomIgnore(tmpIgn, "zeta\nalpha\nservice\n")
  let r2 = reduceName("service login", nsCamel, maxWords = 4, maxChars = 40)
  let words = customIgnoreWords()
  if r2.ok and r2.name == "login" and words == @["alpha", "service", "zeta"]:
    inc v
  else:
    dec v
    echo "  [忽略] 优先级/取数异常: " & $r2.ok & "," & r2.name & "," & $words

  # 清空 → 恢复
  if saveCustomIgnore(tmpIgn, "") == 0 and
      reduceName("user login status cache", nsCamel, maxWords = 4, maxChars = 40).name ==
        "userLoginStatusCache":
    inc v
  else: dec v; echo "  [忽略] 清空恢复异常"

  discard saveCustomAbbreviations(tmpAbbr, "")
  removeFile(tmpAbbr)
  removeFile(tmpIgn)
  if v == 4:
    inc gPass
    echo "PASS  自定义忽略词 4/4（生效/优先于缩写/取数排序/清空恢复）"
  else:
    gFail += 4 - v
    echo "FAIL  自定义忽略词 " & $v & "/4"

# —— 内置行可删可改：禁用缩写 "-" / 保留泛词 "+" ——
block:
  let tmpAbbr = getTempDir() / "ky_abbr_test3.txt"
  let tmpIgn = getTempDir() / "ky_ignore_test3.txt"
  removeFile(tmpAbbr)
  removeFile(tmpIgn)
  discard loadCustomAbbreviations(tmpAbbr)
  discard loadCustomIgnore(tmpIgn)

  var v = 0
  # 基线：内置把 initialization 缩成 init，detail 作为内置泛词被删
  let b0 = reduceName("initialization detail", nsCamel, maxWords = 3, maxChars = 40)
  if b0.ok and b0.name == "init": inc v
  else: dec v; echo "  [内置] 基线异常: " & b0.name

  # 删掉内置缩写行（原词=-）：该词保留、不再缩写，复数同样不缩
  discard saveCustomAbbreviations(tmpAbbr, "initialization=-\n")
  let d1 = reduceName("initialization", nsCamel, maxWords = 3, maxChars = 40)
  let d2 = reduceName("initializations", nsCamel, maxWords = 3, maxChars = 40)
  if d1.ok and d1.name == "initialization" and d2.ok and d2.name == "initializations":
    inc v
  else: dec v; echo "  [禁用缩写] 异常: " & d1.name & " / " & d2.name

  # 删掉内置泛词行（+detail）：detail 不再被删
  discard saveCustomIgnore(tmpIgn, "+detail\n")
  let k1 = reduceName("initialization detail", nsCamel, maxWords = 3, maxChars = 40)
  if k1.ok and k1.name == "initializationDetail": inc v
  else: dec v; echo "  [保留泛词] 异常: " & k1.name

  # 取数：禁用项只在 entries 里，pairs 不含；保留词单独一列；内置表有序
  var entriesHasDash = false
  for (w, a) in customAbbrEntries():
    if w == "initialization" and a == "-": entriesHasDash = true
  var pairsHasInit = false
  for (w, a) in customAbbrPairs():
    if w == "initialization": pairsHasInit = true
  let stops = builtinStopWords()
  var sortedOk = stops.len > 50
  for i in 1 ..< stops.len:
    if cmp(stops[i - 1], stops[i]) > 0: sortedOk = false
  if entriesHasDash and not pairsHasInit and
      customKeepWords() == @["detail"] and sortedOk and
      builtinAbbreviations().len > 30:
    inc v
  else:
    dec v
    echo "  [取数] 异常: dash=" & $entriesHasDash & " pairs=" & $pairsHasInit &
         " keep=" & $customKeepWords() & " stops=" & $stops.len

  # 清空 → 恢复默认
  discard saveCustomAbbreviations(tmpAbbr, "")
  discard saveCustomIgnore(tmpIgn, "")
  let z = reduceName("initialization detail", nsCamel, maxWords = 3, maxChars = 40)
  if z.ok and z.name == "init" and customKeepWords().len == 0: inc v
  else: dec v; echo "  [清空恢复] 异常: " & z.name

  removeFile(tmpAbbr)
  removeFile(tmpIgn)
  if v == 5:
    inc gPass
    echo "PASS  内置行可删可改 5/5（禁用缩写/复数/保留泛词/取数/清空恢复）"
  else:
    gFail += 5 - v
    echo "FAIL  内置行可删可改 " & $v & "/5"

echo "----------------------------------------"
echo "总计: " & $gPass & " 通过, " & $gFail & " 失败"
if gFail > 0:
  echo "有测试未通过"
  quit(1)
echo "全部通过"
