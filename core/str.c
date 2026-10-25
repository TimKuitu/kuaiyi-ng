/* 字符串与编码工具：UTF-8 / GBK / URL 编码 / CJK 检测 */
#include "fastcore.h"
#include <windows.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

char *fy_strdup(const char *s)
{
    if (!s) return NULL;
    size_t n = strlen(s) + 1;
    char *p = (char *)malloc(n);
    if (p) memcpy(p, s, n);
    return p;
}

/* 计算解码 UTF-16 所需字节后转 UTF-8 */
static char *wide_to_utf8(const wchar_t *w)
{
    if (!w) return NULL;
    int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
    if (n <= 0) return NULL;
    char *p = (char *)malloc((size_t)n);
    if (!p) return NULL;
    WideCharToMultiByte(CP_UTF8, 0, w, -1, p, n, NULL, NULL);
    return p;
}

static wchar_t *mb_to_wide(UINT cp, const char *s)
{
    if (!s) return NULL;
    int n = MultiByteToWideChar(cp, 0, s, -1, NULL, 0);
    if (n <= 0) return NULL;
    wchar_t *w = (wchar_t *)malloc(sizeof(wchar_t) * (size_t)n);
    if (!w) return NULL;
    MultiByteToWideChar(cp, 0, s, -1, w, n);
    return w;
}

char *fy_gbk_to_utf8(const char *s)
{
    wchar_t *w = mb_to_wide(936, s);
    if (!w) return NULL;
    char *u = wide_to_utf8(w);
    free(w);
    return u;
}

char *fy_utf8_to_gbk(const char *s)
{
    wchar_t *w = mb_to_wide(CP_UTF8, s);
    if (!w) return NULL;
    int n = WideCharToMultiByte(936, 0, w, -1, NULL, 0, NULL, NULL);
    if (n <= 0) { free(w); return NULL; }
    char *p = (char *)malloc((size_t)n);
    if (!p) { free(w); return NULL; }
    WideCharToMultiByte(936, 0, w, -1, p, n, NULL, NULL);
    free(w);
    return p;
}

/* URL 编码（表单体）：空格编码为 %20，其余非安全字符逐字节百分号编码 */
int fy_urlencode(const char *s, char *out, int cap)
{
    static const char hex[] = "0123456789ABCDEF";
    int o = 0;
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        unsigned char c = *p;
        if ((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') ||
            (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.' || c == '~') {
            if (o + 1 >= cap) return -1;
            out[o++] = (char)c;
        } else if (c == ' ') {
            if (o + 3 >= cap) return -1;
            out[o++] = '%'; out[o++] = '2'; out[o++] = '0';
        } else {
            if (o + 3 >= cap) return -1;
            out[o++] = '%';
            out[o++] = hex[c >> 4];
            out[o++] = hex[c & 15];
        }
    }
    out[o] = 0;
    return o;
}

/* 是否包含 CJK 字符（简繁中文/日文假名汉字） */
int fy_has_cjk(const char *utf8)
{
    unsigned char b;
    unsigned cp;
    for (const unsigned char *p = (const unsigned char *)utf8; *p; ) {
        b = *p;
        if (b < 0x80) { p += 1; cp = b; }
        else if ((b & 0xE0) == 0xC0) { cp = ((b & 0x1Fu) << 6) | (p[1] & 0x3F); p += 2; }
        else if ((b & 0xF0) == 0xE0) { cp = ((b & 0x0Fu) << 12) | ((p[1] & 0x3F) << 6) | (p[2] & 0x3F); p += 3; }
        else if ((b & 0xF8) == 0xF0) { cp = ((b & 0x07u) << 18) | ((p[1] & 0x3Fu) << 12) | ((p[2] & 0x3Fu) << 6) | (p[3] & 0x3F); p += 4; }
        else { p += 1; continue; }
        if ((cp >= 0x4E00 && cp <= 0x9FFF) ||   /* CJK 统一表意 */
            (cp >= 0x3400 && cp <= 0x4DBF) ||   /* 扩展 A */
            (cp >= 0xF900 && cp <= 0xFAFF) ||
            (cp >= 0x3040 && cp <= 0x30FF))     /* 假名 */
            return 1;
    }
    return 0;
}
