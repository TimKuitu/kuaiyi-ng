/* 运行日志自测：轮换 / 多线程并发 / 崩溃过滤器（子进程真踩一次空指针）
 * 用法：test_log.exe <工作目录>
 *       test_log.exe --crash <工作目录>   ← 子进程模式，勿手动调
 */
#include "fastcore.h"
#include "str.h"
#include <windows.h>
#include <stdio.h>
#include <string.h>

static int count_lines(const char *utf8_path, int *has_crash)
{
    char *g = fy_utf8_to_gbk(utf8_path);
    if (!g) return -1;
    FILE *f = fopen(g, "rb");
    free(g);
    if (!f) return -1;
    char line[1024];
    int n = 0;
    *has_crash = 0;
    while (fgets(line, sizeof(line), f)) {
        n++;
        if (strstr(line, "[CRASH]")) *has_crash = 1;
    }
    fclose(f);
    return n;
}

static DWORD WINAPI worker(LPVOID arg)
{
    (void)arg;
    for (int i = 0; i < 200; i++)
        fy_log_write("线程", "tid=%lu i=%d 中文行",
                     (unsigned long)GetCurrentThreadId(), i);
    return 0;
}

static void del_gbk(const char *utf8_path)
{
    char *g = fy_utf8_to_gbk(utf8_path);
    if (g) { DeleteFileA(g); free(g); }
}

int wmain(int argc, wchar_t **argv)
{
    SetConsoleOutputCP(65001);
    if (argc < 2) { printf("usage: test_log <dir>\n"); return 1; }

    char dir[512], path[1024], last[1024];
    WideCharToMultiByte(CP_UTF8, 0, argv[1], -1, dir, sizeof(dir), NULL, NULL);
    snprintf(path, sizeof(path), "%s\\run.log", dir);
    snprintf(last, sizeof(last), "%s\\run.last.log", dir);

    /* --crash 子进程：开日志 → 装过滤器 → 踩空指针，验证 [CRASH] 落盘 */
    if (argc >= 3 && wcscmp(argv[1], L"--crash") == 0) {
        WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, dir, sizeof(dir), NULL, NULL);
        snprintf(path, sizeof(path), "%s\\run.log", dir);
        fy_log_open(path);
        fy_log_write("子进程", "马上要崩了");
        fy_log_install_crash_filter();
        SetErrorMode(SEM_NOGPFAULTERRORBOX);   /* 别让 WER 对话框挡住测试 */
        volatile int *p = NULL;
        *p = 1;
        return 0;   /* 走不到 */
    }

    del_gbk(path);
    del_gbk(last);

    /* 会话一：主线程 10 行 + 两个线程各 200 行 = 411 行（含会话头） */
    fy_log_open(path);
    for (int i = 0; i < 10; i++) fy_log_write("主", "第%d行", i);
    HANDLE t[2];
    t[0] = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    t[1] = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    WaitForMultipleObjects(2, t, TRUE, 10000);
    CloseHandle(t[0]);
    CloseHandle(t[1]);

    int c0 = 0, crash0 = 0;
    c0 = count_lines(path, &crash0);

    /* 会话二启动 → 上次内容轮换进 run.last.log */
    fy_log_open(path);
    fy_log_write("主", "第二次会话");
    int cLast = 0, crashL = 0, cNew = 0, crashN = 0;
    cLast = count_lines(last, &crashL);
    cNew = count_lines(path, &crashN);

    /* 子进程真崩一次：再轮换后 [CRASH] 应出现在 run.log */
    wchar_t self[512], cmd[1200];
    GetModuleFileNameW(NULL, self, 512);
    swprintf(cmd, 1200, L"\"%ls\" --crash \"%ls\"", self, argv[1]);
    STARTUPINFOW si;
    PROCESS_INFORMATION pi;
    ZeroMemory(&si, sizeof(si));
    si.cb = sizeof(si);
    if (!CreateProcessW(NULL, cmd, NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi)) {
        printf("FAIL: CreateProcessW err=%lu\n", (unsigned long)GetLastError());
        return 2;
    }
    WaitForSingleObject(pi.hProcess, 10000);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    int cAfter = 0, crashA = 0;
    cAfter = count_lines(path, &crashA);

    int fail = 0;
    if (c0 != 411)      { printf("FAIL: 会话一行数 %d != 411\n", c0); fail = 1; }
    if (cLast != 411 || crashL)
        { printf("FAIL: 轮换 run.last.log=%d 行 crash=%d\n", cLast, crashL); fail = 1; }
    if (cNew != 2 || crashN)
        { printf("FAIL: 新 run.log=%d 行 crash=%d\n", cNew, crashN); fail = 1; }
    if (!crashA)        { printf("FAIL: 崩溃后 run.log 没有 [CRASH] 行\n"); fail = 1; }
    if (!fail)
        printf("PASS: 轮换/并发/崩溃过滤器 通过 (会话一=%d, last=%d, 崩溃后=%d)\n",
               c0, cLast, cAfter);
    return fail;
}
