/* 百度翻译 mock 实测：test_baidu.exe <url> <appid> <secret> <text> */
#include "fastcore.h"
#include <stdio.h>
#include <windows.h>

int wmain(int argc, wchar_t **argv)
{
    SetConsoleOutputCP(65001);
    if (argc < 5) { printf("usage: test_baidu <url> <appid> <secret> <text>\n"); return 1; }
    char url[512], appid[256], key[256], text[2048];
    WideCharToMultiByte(CP_UTF8, 0, argv[1], -1, url, sizeof(url), NULL, NULL);
    WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, appid, sizeof(appid), NULL, NULL);
    WideCharToMultiByte(CP_UTF8, 0, argv[3], -1, key, sizeof(key), NULL, NULL);
    WideCharToMultiByte(CP_UTF8, 0, argv[4], -1, text, sizeof(text), NULL, NULL);

    char *r = fy_translate_baidu(text, 0, appid, key, url);
    if (!r) { printf("ERROR: %s\n", fy_last_error()); return 2; }
    printf("RESULT: %s\n", r);
    fy_free(r);
    return 0;
}
