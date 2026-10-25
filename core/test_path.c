/* 中文路径的历史模块验证 */
#include "fastcore.h"
#include <stdio.h>
#include <windows.h>

int wmain(void)
{
    SetConsoleOutputCP(65001);
    const char *dir = "D:\\Lenovo\\Desktop\\ToolBox\\快译\\快译NG\\fastT";
    void *h = fy_hist_open(dir);
    if (!h) { printf("fy_hist_open FAILED\n"); return 1; }
    fy_hist_push(h, "hello");
    fy_hist_push(h, "测试");
    printf("count=%d\n", fy_hist_count(h));
    char *s = fy_hist_get(h, 0);
    if (s) { printf("[0]=%s\n", s); fy_free(s); }
    fy_hist_close(h);
    printf("done\n");
    return 0;
}
