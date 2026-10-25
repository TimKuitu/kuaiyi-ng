/* 回退链压力自测：百度必失败 → 必应兜底，循环 n 次
 * test_chain.exe <appid> <secret> <baidu_url> <text> <次数>
 */
#include "fastcore.h"
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int wmain(int argc, wchar_t **argv)
{
    SetConsoleOutputCP(65001);
    if (argc < 6) { printf("usage: test_chain <appid> <secret> <url> <text> <n>\n"); return 1; }
    char appid[256], secret[256], url[512], text[2048];
    WideCharToMultiByte(CP_UTF8, 0, argv[1], -1, appid, sizeof(appid), NULL, NULL);
    WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, secret, sizeof(secret), NULL, NULL);
    WideCharToMultiByte(CP_UTF8, 0, argv[3], -1, url, sizeof(url), NULL, NULL);
    WideCharToMultiByte(CP_UTF8, 0, argv[4], -1, text, sizeof(text), NULL, NULL);
    int n = _wtoi(argv[5]);

    for (int i = 0; i < n; i++) {
        char *r = fy_translate_chain(text, 0, "", "", "", appid, secret, url);
        if (i == 0 || !r)
            printf("[%d] %s\n", i, r ? r : fy_last_error());
        fy_free(r);
    }
    printf("done %d\n", n);
    return 0;
}
