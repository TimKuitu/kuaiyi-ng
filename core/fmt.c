/* 单词命名格式化：多词空格分隔 → 原样/小写/大写/驼峰/下划线/中划线 */
#include "fastcore.h"
#include <windows.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>

#define MAX_WORDS 32

static int utf8_len(const char *s)
{
    int n = 0;
    for (const unsigned char *p = (const unsigned char *)s; *p; p++)
        if ((*p & 0xC0) != 0x80) n++;
    return n;
}

/* 首字母大写（原地，仅处理第一字节为 ASCII 的词；非 ASCII 原样） */
static void cap_first(char *w)
{
    if (w[0] >= 'a' && w[0] <= 'z') w[0] = (char)(w[0] - 32);
}

static void lower_ascii(char *w)
{
    for (char *p = w; *p; p++)
        if (*p >= 'A' && *p <= 'Z') *p = (char)(*p + 32);
}

static void upper_ascii(char *w)
{
    for (char *p = w; *p; p++)
        if (*p >= 'a' && *p <= 'z') *p = (char)(*p - 32);
}

char *fy_format_word(const char *utf8_text, int fmt)
{
    if (!utf8_text) return NULL;
    size_t total = strlen(utf8_text) + 64;
    char *out = (char *)malloc(total);
    if (!out) return NULL;
    out[0] = 0;
    int off = 0;

    /* 复制并按空白分词 */
    char *copy = fy_strdup(utf8_text);
    if (!copy) { free(out); return NULL; }

    char *words[MAX_WORDS];
    int nw = 0;
    char *p = copy;
    while (*p && nw < MAX_WORDS) {
        while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
        if (!*p) break;
        char *start = p;
        while (*p && *p != ' ' && *p != '\t' && *p != '\r' && *p != '\n') p++;
        if (*p) *p++ = 0;
        words[nw++] = start;
    }
    if (nw == 0) { free(copy); free(out); return NULL; }

    switch (fmt) {
    case FY_FMT_LOWER:
    case FY_FMT_SNAKE:
    case FY_FMT_KEBAB:
        for (int i = 0; i < nw; i++)
            lower_ascii(words[i]);
        break;
    case FY_FMT_UPPER:
        for (int i = 0; i < nw; i++)
            upper_ascii(words[i]);
        break;
    case FY_FMT_CAMEL:
        lower_ascii(words[0]);
        for (int i = 1; i < nw; i++)
            cap_first(words[i]);
        break;
    default:
        break;
    }

    const char *sep = " ";
    if (fmt == FY_FMT_SNAKE) sep = "_";
    if (fmt == FY_FMT_KEBAB) sep = "-";
    if (fmt == FY_FMT_CAMEL) sep = "";   /* 驼峰不能有空格 */

    for (int i = 0; i < nw; i++) {
        if (i) off += snprintf(out + off, total - (size_t)off, "%s", sep);
        off += snprintf(out + off, total - (size_t)off, "%s", words[i]);
    }
    (void)utf8_len;
    free(copy);
    return out;
}

void fy_free(void *p)
{
    free(p);
}
