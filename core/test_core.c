/* 核心功能命令行自测（宽字符入口，避免命令行编码问题）
 * test_core.exe <mode 0/1/2> <text> [fmt]
 */
#include "fastcore.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

static char *wide_to_utf8(const wchar_t *w)
{
    int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
    char *p = (char *)malloc((size_t)n);
    WideCharToMultiByte(CP_UTF8, 0, w, -1, p, n, NULL, NULL);
    return p;
}

int wmain(int argc, wchar_t **argv)
{
    SetConsoleOutputCP(65001);
    if (argc < 3) {
        printf("usage: test_core <mode 0/1/2> <text> [fmt]\n");
        return 1;
    }
    int mode = _wtoi(argv[1]);
    char *text = wide_to_utf8(argv[2]);

    char *r = fy_translate(text, mode);
    if (!r) {
        char *err = fy_strdup(fy_last_error());
        printf("ERROR: %s\n", err);
        fy_free(err);
        return 2;
    }
    printf("%s\n", r);
    fy_free(r);

    if (argc > 3 && wcscmp(argv[3], L"fmt") == 0) {
        for (int f = 0; f <= 5; f++) {
            char *w = fy_format_word(text, f);
            if (w) { printf("fmt%d: %s\n", f, w); fy_free(w); }
        }
    }
    free(text);
    return 0;
}
