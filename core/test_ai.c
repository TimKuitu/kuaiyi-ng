/* AI 接口 + 检查更新 自测 */
#include <stdio.h>
#include <windows.h>
#include "fastcore.h"

int wmain(int argc, wchar_t** argv) {
    SetConsoleOutputCP(65001);
    if (argc >= 6 && wcscmp(argv[1], L"ai") == 0) {
        /* test_ai.exe ai <base> <key> <model> <text> */
        char base[512], key[512], model[128], text[2048];
        WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, base, sizeof(base), NULL, NULL);
        WideCharToMultiByte(CP_UTF8, 0, argv[3], -1, key, sizeof(key), NULL, NULL);
        WideCharToMultiByte(CP_UTF8, 0, argv[4], -1, model, sizeof(model), NULL, NULL);
        WideCharToMultiByte(CP_UTF8, 0, argv[5], -1, text, sizeof(text), NULL, NULL);
        char* r = fy_translate_ai(text, 0, base, key, model);
        if (!r) {
            printf("ERROR: %s\n", fy_last_error());
            return 2;
        }
        printf("AI: %s\n", r);
        fy_free(r);
        return 0;
    }
    if (argc >= 2 && wcscmp(argv[1], L"upd") == 0) {
        char ver[64], url[512];
        int r = fy_check_update("1.1", ver, sizeof(ver), url, sizeof(url));
        printf("check_update(1.1) = %d  ver=%s\n", r, ver);
        printf("url=%s\n", url);
        if (r == 1 && argc >= 3) {
            char dest[512];
            WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, dest, sizeof(dest), NULL, NULL);
            int d = fy_download_update(url, dest, NULL);
            printf("download = %d\n", d);
        }
        return 0;
    }
    if (argc >= 4 && wcscmp(argv[1], L"dl") == 0) {
        char url[512], dest[512];
        WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, url, sizeof(url), NULL, NULL);
        WideCharToMultiByte(CP_UTF8, 0, argv[3], -1, dest, sizeof(dest), NULL, NULL);
        int r = fy_download_update(url, dest, NULL);
        printf("download=%d err=%s\n", r, r == 0 ? "-" : fy_last_error());
        return r == 0 ? 0 : 3;
    }
    printf("usage: test_ai ai <base> <key> <model> <text> | test_ai upd [dest] | test_ai dl <url> <dest>\n");
    return 1;
}
