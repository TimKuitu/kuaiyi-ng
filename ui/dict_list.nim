## 通用虚拟滚动词库列表组件（10 槽位复用 + 实时搜索过滤，无条数上限）
## 用于设置页的「缩略词」「忽略词」「常用词」三个列表标签页。

import std/[strutils]
import winim/lean
import wNim
import modern_settings

const
  DictPadL* = SetPadL
  DictContW* = SetContW
  DictSearchY* = 94
  DictTop* = 130
  DictRowH* = 33
  DictViewRows* = 10

type
  DictListMode* = enum
    dlOneCol    ## 单列：key + 删除
    dlTwoCol    ## 双列：key → val + 删除

  DictItem* = tuple[key, val: string]

  DictSlotCtl = object
    keyEdt: wTextCtrl
    arrowLbl: wStaticText
    valEdt: wTextCtrl
    delBtn: wButton

  DictWheelHook* = proc (w: wWindow, tag: int)

  DictList* = ref object
    mode*: DictListMode
    page*: wPanel
    wheelTag*: int
    wheelHook*: DictWheelHook
    scale*: float
    items*: seq[DictItem]
    filtered*: seq[int]       ## 指向 items 的下标
    scroll*: int
    syncing: bool             ## setValue 防重入标志
    slots: array[DictViewRows, DictSlotCtl]
    lblSearch*: wStaticText
    edtSearch*: wTextCtrl
    btnClearSearch*: wButton
    sb*: wScrollBar
    lblStatus*: wStaticText

proc dpx(v: int, scale: float): int {.inline.} =
  int(float(v) * scale)

proc rebuildFilter*(dl: DictList) =
  ## 按当前搜索关键字重建 filtered 索引表
  let q = if dl.edtSearch.isNil: "" else: dl.edtSearch.getValue().strip().toLowerAscii()
  dl.filtered.setLen(0)
  for i, it in dl.items:
    if q.len == 0 or
       it.key.toLowerAscii().contains(q) or
       (dl.mode == dlTwoCol and it.val.toLowerAscii().contains(q)):
      dl.filtered.add(i)

proc render*(dl: DictList, scale: float) =
  ## 将 filtered[scroll ..< scroll + DictViewRows] 映射到固定 10 个槽位控件
  dl.scale = scale
  if dl.page.isNil: return
  var pw = int(float(dl.page.getSize().width) / scale)
  if pw < 200: pw = SetWinW

  let total = dl.filtered.len
  let maxS = max(0, total - DictViewRows)
  if dl.scroll > maxS: dl.scroll = maxS
  if dl.scroll < 0: dl.scroll = 0

  let left = DictPadL
  let right = pw - DictPadL - 18 - 6
  let delW = 60
  let arrowW = 20
  let gap = 8
  let delX = right - delW

  dl.syncing = true
  try:
    for s in 0 ..< DictViewRows:
      let fIdx = dl.scroll + s
      if fIdx < total:
        let it = dl.items[dl.filtered[fIdx]]
        let y = DictTop + s * DictRowH
        if dl.mode == dlTwoCol:
          let half = (delX - left - arrowW - 3 * gap) div 2
          dl.slots[s].keyEdt.setSize(dpx(left, scale), dpx(y, scale), dpx(half, scale), dpx(26, scale))
          dl.slots[s].arrowLbl.setSize(dpx(left + half + gap, scale), dpx(y + 4, scale), dpx(arrowW, scale), dpx(18, scale))
          dl.slots[s].valEdt.setSize(dpx(left + half + gap + arrowW + gap, scale), dpx(y, scale), dpx(half, scale), dpx(26, scale))
          dl.slots[s].delBtn.setSize(dpx(delX, scale), dpx(y, scale), dpx(delW, scale), dpx(26, scale))
          dl.slots[s].keyEdt.setValue(it.key)
          dl.slots[s].valEdt.setValue(it.val)
          dl.slots[s].keyEdt.show()
          dl.slots[s].arrowLbl.show()
          dl.slots[s].valEdt.show()
          dl.slots[s].delBtn.show()
        else:
          let wordW = delX - left - gap
          dl.slots[s].keyEdt.setSize(dpx(left, scale), dpx(y, scale), dpx(wordW, scale), dpx(26, scale))
          dl.slots[s].delBtn.setSize(dpx(delX, scale), dpx(y, scale), dpx(delW, scale), dpx(26, scale))
          dl.slots[s].keyEdt.setValue(it.key)
          dl.slots[s].keyEdt.show()
          dl.slots[s].delBtn.show()
      else:
        dl.slots[s].keyEdt.hide()
        dl.slots[s].delBtn.hide()
        if dl.mode == dlTwoCol:
          dl.slots[s].arrowLbl.hide()
          dl.slots[s].valEdt.hide()
  finally:
    dl.syncing = false

  dl.sb.setSize(dpx(pw - DictPadL - 18, scale), dpx(DictTop, scale),
                dpx(18, scale), dpx(DictViewRows * DictRowH - 8, scale))
  dl.sb.setScrollbar(dl.scroll, DictViewRows, max(total - 1, 1))
  if total > DictViewRows: dl.sb.enable() else: dl.sb.disable()
  discard InvalidateRect(cast[HWND](dl.page.getHandle()), nil, FALSE)

proc scrollBy*(dl: DictList, notches: int) =
  ## 滚轮滚动（一格 3 行；正增量＝向上滚＝行号减小）
  let total = dl.filtered.len
  if total <= DictViewRows: return
  var s = dl.scroll - notches * 3
  if s < 0: s = 0
  let maxS = total - DictViewRows
  if s > maxS: s = maxS
  if s == dl.scroll: return
  dl.scroll = s
  dl.render(dl.scale)

proc setItems*(dl: DictList, items: seq[DictItem], scale: float, preserveScroll = false) =
  dl.items = items
  if dl.items.len == 0:
    dl.items.add(("", ""))
  if not preserveScroll:
    dl.scroll = 0
    if not dl.edtSearch.isNil:
      dl.syncing = true
      dl.edtSearch.setValue("")
      dl.syncing = false
  dl.rebuildFilter()
  dl.render(scale)

proc addRow*(dl: DictList, scale: float) =
  ## 追加一行空项；若当前处于搜索过滤状态则先清空搜索以便直接看到新行
  if not dl.edtSearch.isNil and dl.edtSearch.getValue().strip().len > 0:
    dl.syncing = true
    dl.edtSearch.setValue("")
    dl.syncing = false
  dl.items.add(("", ""))
  dl.rebuildFilter()
  dl.scroll = max(0, dl.filtered.len - DictViewRows)
  dl.render(scale)
  let visSlot = dl.filtered.len - 1 - dl.scroll
  if visSlot >= 0 and visSlot < DictViewRows:
    dl.slots[visSlot].keyEdt.setFocus()

proc deleteSlot(dl: DictList, slot: int) =
  let fIdx = dl.scroll + slot
  if fIdx < 0 or fIdx >= dl.filtered.len: return
  let realIdx = dl.filtered[fIdx]
  dl.items.delete(realIdx)
  if dl.items.len == 0:
    dl.items.add(("", ""))
  dl.rebuildFilter()
  dl.render(dl.scale)

proc bindSlotEvents(dl: DictList, s: int) =
  ## 独立 proc 建闭包，避免循环变量捕获同一索引
  dl.slots[s].keyEdt.wEvent_Text do ():
    if dl.syncing: return
    let fIdx = dl.scroll + s
    if fIdx >= 0 and fIdx < dl.filtered.len:
      dl.items[dl.filtered[fIdx]].key = dl.slots[s].keyEdt.getValue().strip()

  if dl.mode == dlTwoCol:
    dl.slots[s].valEdt.wEvent_Text do ():
      if dl.syncing: return
      let fIdx = dl.scroll + s
      if fIdx >= 0 and fIdx < dl.filtered.len:
        dl.items[dl.filtered[fIdx]].val = dl.slots[s].valEdt.getValue().strip()

  dl.slots[s].delBtn.wEvent_Button do ():
    dl.deleteSlot(s)

proc newDictList*(page: wPanel, mode: DictListMode, wheelTag: int,
                  scale: float, hook: DictWheelHook,
                  hint1Text, hint2Text: string,
                  btnAdd: var wButton, btnSave: var wButton,
                  lblStatus: var wStaticText): DictList =
  ## 在设置页构建搜索栏、10 槽位虚拟行控件、滚动条及底部操作栏
  let dl = DictList(
    mode: mode,
    page: page,
    wheelTag: wheelTag,
    wheelHook: hook,
    scale: scale,
    items: @[],
    filtered: @[],
    scroll: 0
  )

  let h1 = StaticText(page, label=hint1Text,
                      pos=(dpx(DictPadL + 12, scale), dpx(14, scale)),
                      size=(dpx(DictContW - 24, scale), dpx(24, scale)))
  let h2 = StaticText(page, label=hint2Text,
                      pos=(dpx(DictPadL + 12, scale), dpx(38, scale)),
                      size=(dpx(DictContW - 24, scale), dpx(44, scale)))
  modernMarkStatic(cast[HWND](h1.getHandle()), inCard=true, role=trNormal)
  modernMarkStatic(cast[HWND](h2.getHandle()), inCard=true, role=trHint)

  dl.lblSearch = StaticText(page, label="搜索：",
                            pos=(dpx(DictPadL, scale), dpx(DictSearchY + 4, scale)),
                            size=(dpx(62, scale), dpx(22, scale)))
  modernMarkStatic(cast[HWND](dl.lblSearch.getHandle()), inCard=false, role=trNormal)
  dl.edtSearch = TextCtrl(page, pos=(dpx(DictPadL + 64, scale), dpx(DictSearchY, scale)),
                          size=(dpx(DictContW - 64 - 72, scale), dpx(28, scale)))
  modernStyleEdit(dl.edtSearch, inCard=false)
  dl.btnClearSearch = Button(page, label="清除",
                             pos=(dpx(DictPadL + DictContW - 64, scale), dpx(DictSearchY, scale)),
                             size=(dpx(64, scale), dpx(28, scale)))
  modernStyleButton(dl.btnClearSearch, mbNormal)

  dl.sb = ScrollBar(page, pos=(dpx(SetWinW - DictPadL - 18, scale), dpx(DictTop, scale)),
                    size=(dpx(18, scale), dpx(DictViewRows * DictRowH - 8, scale)),
                    style=wSbVertical)
  modernStyleDarkSystem(cast[HWND](dl.sb.getHandle()), "DarkMode_Explorer")

  let by = SetPageH - 48
  btnAdd = Button(page, label="添加一行",
                  pos=(dpx(DictPadL, scale), dpx(by, scale)),
                  size=(dpx(96, scale), dpx(32, scale)))
  modernStyleButton(btnAdd, mbNormal)
  lblStatus = StaticText(page, label="",
                         pos=(dpx(DictPadL + 108, scale), dpx(by + 6, scale)),
                         size=(dpx(320, scale), dpx(22, scale)))
  modernMarkStatic(cast[HWND](lblStatus.getHandle()), inCard=false, role=trHint)
  dl.lblStatus = lblStatus
  btnSave = Button(page, label="保存",
                   pos=(dpx(SetWinW - DictPadL - 96, scale), dpx(by, scale)),
                   size=(dpx(96, scale), dpx(32, scale)))
  modernStyleButton(btnSave, mbPrimary)

  for s in 0 ..< DictViewRows:
    dl.slots[s].keyEdt = TextCtrl(page, pos=(0, 0), size=(10, 10))
    dl.slots[s].delBtn = Button(page, label="删除", pos=(0, 0), size=(10, 10))
    modernStyleEdit(dl.slots[s].keyEdt, inCard=false)
    modernStyleButton(dl.slots[s].delBtn, mbDanger)
    dl.slots[s].keyEdt.hide()
    dl.slots[s].delBtn.hide()
    if mode == dlTwoCol:
      dl.slots[s].arrowLbl = StaticText(page, label="→", pos=(0, 0), size=(10, 10))
      dl.slots[s].valEdt = TextCtrl(page, pos=(0, 0), size=(10, 10))
      modernMarkStatic(cast[HWND](dl.slots[s].arrowLbl.getHandle()), inCard=false, role=trHint)
      modernStyleEdit(dl.slots[s].valEdt, inCard=false)
      dl.slots[s].arrowLbl.hide()
      dl.slots[s].valEdt.hide()
    dl.bindSlotEvents(s)
    if hook != nil:
      hook(dl.slots[s].keyEdt, wheelTag)
      hook(dl.slots[s].delBtn, wheelTag)
      if mode == dlTwoCol:
        hook(dl.slots[s].arrowLbl, wheelTag)
        hook(dl.slots[s].valEdt, wheelTag)

  if hook != nil:
    hook(page, wheelTag)
    hook(h1, wheelTag)
    hook(h2, wheelTag)
    hook(dl.lblSearch, wheelTag)
    hook(dl.edtSearch, wheelTag)
    hook(dl.btnClearSearch, wheelTag)
    hook(dl.sb, wheelTag)
    hook(btnAdd, wheelTag)
    hook(lblStatus, wheelTag)
    hook(btnSave, wheelTag)

  dl.edtSearch.wEvent_Text do ():
    if dl.syncing: return
    dl.scroll = 0
    dl.rebuildFilter()
    dl.render(dl.scale)

  dl.btnClearSearch.wEvent_Button do ():
    if dl.edtSearch.getValue().len == 0: return
    dl.syncing = true
    dl.edtSearch.setValue("")
    dl.syncing = false
    dl.scroll = 0
    dl.rebuildFilter()
    dl.render(dl.scale)

  dl.sb.wEvent_ScrollBar do ():
    dl.scroll = dl.sb.getScrollPos()
    dl.render(dl.scale)

  result = dl
