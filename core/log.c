/* 运行日志：fastT\run.log（UTF-8）
 * 每次启动把上次的 run.log 轮换为 run.last.log——闪退后拿这两个文件就能还原
 * 上一次的使用过程。翻译在裸线程里跑，写入必须跨线程安全（临界区）；
 * 崩溃过滤器单独直写（崩溃可能正好发生在持锁线程上，不能再进锁）。
 */
#include "fastcore.h"
#include "str.h"
#include <windows.h>
#include <string.h>
#include <stdio.h>
#include <stdarg.h>

static CRITICAL_SECTION g_lock;
static int g_lock_ok = 0;
static char g_path_gbk[1024];   /* 首字节为 0 表示未开启 */

static void log_lock_init(void)
{
    if (!g_lock_ok) {
        InitializeCriticalSection(&g_lock);
        g_lock_ok = 1;
    }
}

/* 单行落盘：打开-写-关，不缓冲，硬崩也不丢已写的行 */
static void write_line_raw(const char *line)
{
    FILE *f = fopen(g_path_gbk, "ab");
    if (!f) return;
    SYSTEMTIME st;
    GetLocalTime(&st);
    fprintf(f, "[%04u-%02u-%02u %02u:%02u:%02u.%03u]%s\n",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond,
            st.wMilliseconds, line);
    fclose(f);
}

void fy_log_open(const char *utf8_path)
{
    log_lock_init();
    if (!utf8_path || !utf8_path[0]) { g_path_gbk[0] = 0; return; }
    char *g = fy_utf8_to_gbk(utf8_path);
    if (!g) return;
    EnterCriticalSection(&g_lock);
    size_t n = strlen(g);
    char last[1024];
    if (n > 4 && n + 8 < sizeof(last) && strcmp(g + n - 4, ".log") == 0)
        snprintf(last, sizeof(last), "%.*s.last.log", (int)(n - 4), g);
    else
        snprintf(last, sizeof(last), "%s.last", g);
    MoveFileExA(g, last, MOVEFILE_REPLACE_EXISTING);
    snprintf(g_path_gbk, sizeof(g_path_gbk), "%s", g);
    LeaveCriticalSection(&g_lock);
    LeaveCriticalSection(&g_lock);
    free(g);
    write_line_raw("==== 会话开始 ====");
}

void fy_log_write(const char *tag, const char *fmt, ...)
{
    if (!g_path_gbk[0] || !fmt) return;
    char line[1024];
    int off = snprintf(line, sizeof(line), "[%s] ", tag ? tag : "log");
    if (off < 0 || off >= (int)sizeof(line) - 1) return;
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(line + off, sizeof(line) - (size_t)off, fmt, ap);
    va_end(ap);
    log_lock_init();
    EnterCriticalSection(&g_lock);
    write_line_raw(line);
    LeaveCriticalSection(&g_lock);
}

/* ---- 闪退现场：未处理异常过滤器 ---- */

static int g_in_crash = 0;

static const char *exc_name(unsigned long code)
{
    switch (code) {
    case 0xC0000005: return "访问违例";
    case 0xC00000FD: return "栈溢出";
    case 0xC0000409: return "栈保护/快速失败";
    case 0xC0000094: return "除零";
    case 0xE06D7363: return "C++异常";
    default:         return "未知异常";
    }
}

static LONG WINAPI crash_filter(EXCEPTION_POINTERS *ep)
{
    if (g_path_gbk[0] && !g_in_crash) {
        g_in_crash = 1;   /* 过滤器里再崩就随它去，别递归 */
        unsigned long code = (unsigned long)ep->ExceptionRecord->ExceptionCode;
        char line[512];
        int off = snprintf(line, sizeof(line),
                           "[CRASH] 未处理异常 0x%08lX（%s）地址=%p 线程=%lu",
                           code, exc_name(code),
                           (void *)ep->ExceptionRecord->ExceptionAddress,
                           (unsigned long)GetCurrentThreadId());
        /* 访问违例补上读/写与目标地址（x64 参数布局见 EXCEPTION_RECORD 文档） */
        if (code == 0xC0000005 && ep->ExceptionRecord->NumberParameters >= 2 &&
            off > 0 && off < (int)sizeof(line) - 32) {
            unsigned long op = (unsigned long)ep->ExceptionRecord->ExceptionInformation[0];
            snprintf(line + off, sizeof(line) - (size_t)off, " %s %p",
                     op == 0 ? "读" : (op == 1 ? "写" : "执行"),
                     (void *)ep->ExceptionRecord->ExceptionInformation[1]);
        }
        write_line_raw(line);
    }
    return EXCEPTION_CONTINUE_SEARCH;
}

void fy_log_install_crash_filter(void)
{
    SetUnhandledExceptionFilter(crash_filter);
}
