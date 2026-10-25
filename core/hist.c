/* 查询历史：缓存目录下 history.txt（UTF-8，每行一条，最新在最后，上限 200） */
#include "fastcore.h"
#include "str.h"
#include <windows.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

#define HIST_MAX 200

typedef struct {
    char  *path_gbk;
    char **items;
    int    count, cap;
} Hist;

static void ensure_dir(const char *path_gbk)
{
    /* 逐级创建（GBK 路径，反斜杠分隔） */
    char buf[1024];
    snprintf(buf, sizeof(buf), "%s", path_gbk);
    for (char *p = buf + 1; *p; p++) {
        if (*p == '\\' || *p == '/') {
            char c = *p;
            *p = 0;
            CreateDirectoryA(buf, NULL);
            *p = c;
        }
    }
    CreateDirectoryA(buf, NULL);
}

static void hist_load(Hist *h)
{
    FILE *f = fopen(h->path_gbk, "rb");
    if (!f) return;
    char line[4096];
    while (fgets(line, sizeof(line), f)) {
        size_t n = strlen(line);
        while (n > 0 && (line[n - 1] == '\n' || line[n - 1] == '\r')) line[--n] = 0;
        if (!n) continue;
        if (h->count == h->cap) {
            h->cap = h->cap ? h->cap * 2 : 32;
            h->items = (char **)realloc(h->items, sizeof(char *) * (size_t)h->cap);
        }
        h->items[h->count++] = fy_strdup(line);
    }
    fclose(f);
}

static void hist_save(Hist *h)
{
    FILE *f = fopen(h->path_gbk, "wb");
    if (!f) return;
    for (int i = 0; i < h->count; i++) {
        fputs(h->items[i], f);
        fputs("\r\n", f);
    }
    fclose(f);
}

void *fy_hist_open(const char *utf8_dir)
{
    if (!utf8_dir) return NULL;
    Hist *h = (Hist *)calloc(1, sizeof(Hist));
    if (!h) return NULL;

    char *dir = fy_utf8_to_gbk(utf8_dir);
    if (!dir) { free(h); return NULL; }
    ensure_dir(dir);

    char path[1024];
    snprintf(path, sizeof(path), "%s\\history.txt", dir);
    free(dir);
    h->path_gbk = fy_strdup(path);
    if (!h->path_gbk) { free(h); return NULL; }
    hist_load(h);
    return h;
}

int fy_hist_count(void *p)
{
    Hist *h = (Hist *)p;
    return h ? h->count : 0;
}

char *fy_hist_get(void *p, int index)
{
    Hist *h = (Hist *)p;
    if (!h || index < 0 || index >= h->count) return NULL;
    return fy_strdup(h->items[index]);
}

void fy_hist_push(void *p, const char *utf8_text)
{
    Hist *h = (Hist *)p;
    if (!h || !utf8_text || !utf8_text[0]) return;

    char *text = fy_strdup(utf8_text);
    if (!text) return;
    size_t n = strlen(text);
    while (n > 0 && (text[n - 1] == ' ' || text[n - 1] == '\t' ||
                     text[n - 1] == '\r' || text[n - 1] == '\n')) text[--n] = 0;
    if (!n) { free(text); return; }

    /* 已存在则删除旧条目（去重），再追加到末尾（最新） */
    for (int i = 0; i < h->count; i++) {
        if (strcmp(h->items[i], text) == 0) {
            free(h->items[i]);
            memmove(&h->items[i], &h->items[i + 1],
                    sizeof(char *) * (size_t)(h->count - i - 1));
            h->count--;
            break;
        }
    }
    if (h->count == h->cap) {
        h->cap = h->cap ? h->cap * 2 : 32;
        h->items = (char **)realloc(h->items, sizeof(char *) * (size_t)h->cap);
    }
    h->items[h->count++] = text;
    /* 超出上限：丢弃最旧 */
    while (h->count > HIST_MAX) {
        free(h->items[0]);
        memmove(&h->items[0], &h->items[1], sizeof(char *) * (size_t)(h->count - 1));
        h->count--;
    }
    hist_save(h);
}

void fy_hist_close(void *p)
{
    Hist *h = (Hist *)p;
    if (!h) return;
    for (int i = 0; i < h->count; i++) free(h->items[i]);
    free(h->items);
    free(h->path_gbk);
    free(h);
}
