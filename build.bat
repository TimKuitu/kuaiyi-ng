@echo off
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
cd /d %~dp0
set PATH=C:\msys64\mingw64\bin;D:\ProgramFiles\nim-1.6.20\bin;%PATH%
set NIM=D:\ProgramFiles\nim-1.6.20\bin\nim.exe

windres app.rc -O coff -o res\app.res
if errorlevel 1 goto :err

rem 中文路径会让链接器报错，先输出到 D:\ 再拷回
%NIM% c --warnings:off --app:gui --opt:speed -d:mingw ^
  --path:"D:\ProgramFiles\wNim" ^
  --path:"D:\ProgramFiles\winim" ^
  --path:"D:\ProgramFiles\memlib" ^
  --passC:"-Wno-error=incompatible-pointer-types" ^
  --passL:"res\app.res" --passL:"-lwininet" ^
  -o:"D:\kuaityNG_build.exe" ui\main.nim
if errorlevel 1 goto :err

copy /y D:\kuaityNG_build.exe 快译NG.exe >nul
del D:\kuaityNG_build.exe
echo.
echo === 构建成功: 快译NG.exe ===
goto :eof

:err
echo === 构建失败 ===
exit /b 1
