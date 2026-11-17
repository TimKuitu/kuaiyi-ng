@echo off
chcp 65001 >nul
rem ============================================================
rem  快译NG 构建脚本
rem  依赖（本机已装好）：
rem    gcc/windres : MSYS2 MinGW-w64 (C:\msys64\mingw64\bin)
rem    Nim 1.6.20  : D:\ProgramFiles\nim-1.6.20   (必须 1.6，2.x 与 wNim 不兼容)
rem    wNim        : D:\ProgramFiles\wNim
rem    winim       : D:\ProgramFiles\winim
rem    memlib      : D:\ProgramFiles\memlib
rem  注：wNim 已打两个本地补丁（见 说明.md）
rem ============================================================
cd /d "%~dp0"
set "PATH=C:\msys64\mingw64\bin;D:\ProgramFiles\nim-1.6.20\bin;%PATH%"
set "NIM=D:\ProgramFiles\nim-1.6.20\bin\nim.exe"
set "TMP_EXE=%TEMP%\kuaityNG_build.exe"
set "NIM_CACHE=%TEMP%\kuaityNG_nimcache"

echo [1/4] 编译资源 (windres)...
windres app.rc -O coff -o res\app.res
if errorlevel 1 goto :err

rem 中文路径会让链接器报错，先输出到纯英文路径再拷回
rem 必须 -d:release：关掉栈追踪。调试版每个 Nim 过程都要读写全局 framePtr，
rem 翻译线程（CreateThread 裸线程）会和 UI 线程竞态写坏它 -> 随机 0xC0000005 闪退
echo [2/4] 编译主程序 (nim c)...
"%NIM%" c --hints:off --warnings:off --app:gui --opt:speed -d:release -d:mingw ^
  --nimcache:"%NIM_CACHE%" ^
  --path:"D:\ProgramFiles\wNim" ^
  --path:"D:\ProgramFiles\winim" ^
  --path:"D:\ProgramFiles\memlib" ^
  --passC:"-Wno-error=incompatible-pointer-types" ^
  --passL:"res\app.res" --passL:"-lwininet" ^
  -o:"%TMP_EXE%" ui\main.nim
if errorlevel 1 goto :err

echo [3/4] 更新目标文件 (快译NG.exe)...
copy /y "%TMP_EXE%" "快译NG.exe" >nul
if errorlevel 1 (
  echo [错误] 无法覆盖 快译NG.exe，请先关闭正在运行的 快译NG.exe 后重试。
  goto :err
)

echo [4/4] 清理临时文件...
if exist "%TMP_EXE%" del /q "%TMP_EXE%"

echo.
echo === [BUILD_DONE] 构建成功: 快译NG.exe (exit=0) ===
exit /b 0

:err
if exist "%TMP_EXE%" del /q "%TMP_EXE%" >nul 2>&1
echo.
echo === [BUILD_FAILED] 构建失败 (exit=1) ===
exit /b 1
