/* INI 配置：兼容原版 GBK 文件，未知段/键原样保留（方便与旧版互换） */
#include "fastcore.h"
#include "str.h"
#include <windows.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    char **lines;     /* 每行一个 UTF-8 堆串（不含换行符） */
    int    count, cap;
    char  *path_gbk;
    int    dirty;
} Cfg;

static char *read_file_all(const char *path_gbk)
{
    FILE *f = fopen(path_gbk, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (n < 0 || n > 4 * 1024 * 1024) { fclose(f); return NULL; }
    char *buf = (char *)malloc((size_t)n + 1);
    if (!buf) { fclose(f); return NULL; }
    size_t got = fread(buf, 1, (size_t)n, f);
    fclose(f);
    buf[got] = 0;
    return buf;
}

/* 判断 UTF-8 BOM */
static int is_utf8bom(const char *buf)
{
    return (unsigned char)buf[0] == 0xEF && (unsigned char)buf[1] == 0xBB &&
           (unsigned char)buf[2] == 0xBF;
}

void *fy_cfg_open(const char *utf8_path)
{
    if (!utf8_path) return NULL;
    Cfg *c = (Cfg *)calloc(1, sizeof(Cfg));
    if (!c) return NULL;

    char *pg = fy_utf8_to_gbk(utf8_path);
    if (!pg) { free(c); return NULL; }
    c->path_gbk = pg;

    char *raw = read_file_all(pg);
    if (raw) {
        char *u;
        if (is_utf8bom(raw)) u = fy_strdup(raw + 3);
        else                 u = fy_gbk_to_utf8(raw);
        free(raw);
        if (u) {
            char *p = u;
            while (*p) {
                char *nl = strpbrk(p, "\r\n");
                char *line;
                if (nl) {
                    line = (char *)malloc((size_t)(nl - p) + 1);
                    if (line) { memcpy(line, p, (size_t)(nl - p)); line[nl - p] = 0; }
                    p = nl;
                    while (*p == '\r' || *p == '\n') p++;
                } else {
                    line = fy_strdup(p);
                    p += strlen(p);
                }
                if (line) {
                    if (c->count == c->cap) {
                        c->cap = c->cap ? c->cap * 2 : 64;
                        c->lines = (char **)realloc(c->lines, sizeof(char *) * (size_t)c->cap);
                    }
                    c->lines[c->count++] = line;
                }
            }
            free(u);
        }
    }
    return c;
}

/* 查找：section 区间内 key 所在行号；找不到返回 -1；*sec_end 返回段末（下一 section 或文件尾） */
static int find_key(Cfg *c, const char *sec, const char *key, int *sec_end)
{
    int in_sec = 0, seclen = (int)strlen(sec), keylen = (int)strlen(key);
    *sec_end = c->count;
    for (int i = 0; i < c->count; i++) {
        char *l = c->lines[i];
        char *e = l + strlen(l);
        while (e > l && (e[-1] == ' ' || e[-1] == '\t')) e--;
        if (*l == '[') {
            char *cb = strchr(l, ']');
            if (cb && (int)(cb - l - 1) == seclen && strncmp(l + 1, sec, (size_t)seclen) == 0) {
                in_sec = 1;
                *sec_end = c->count;
                continue;
            }
            if (in_sec) { *sec_end = i; return -1; }   /* 进入下一段 */
            continue;
        }
        if (!in_sec) continue;
        *sec_end = c->count;
        if (strncmp(l, key, (size_t)keylen) == 0 && l[keylen] == '=') return i;
    }
    return in_sec ? -1 : -1;
}

int fy_cfg_get(void *cfg, const char *sec, const char *key,
               const char *utf8_def, char *out, int cap)
{
    Cfg *c = (Cfg *)cfg;
    if (!c || !sec || !key || !out || cap < 1) { if (out && cap > 0) out[0] = 0; return 0; }
    int sec_end;
    int i = find_key(c, sec, key, &sec_end);
    if (i < 0) {
        snprintf(out, (size_t)cap, "%s", utf8_def ? utf8_def : "");
        return 0;
    }
    char *v = c->lines[i] + strlen(key) + 1;
    snprintf(out, (size_t)cap, "%s", v);
    return 1;
}

static void cfg_insert_line(Cfg *c, int at, const char *text)
{
    if (c->count == c->cap) {
        c->cap = c->cap ? c->cap * 2 : 64;
        c->lines = (char **)realloc(c->lines, sizeof(char *) * (size_t)c->cap);
    }
    if (at < 0 || at > c->count) at = c->count;
    memmove(&c->lines[at + 1], &c->lines[at], sizeof(char *) * (size_t)(c->count - at));
    c->lines[at] = fy_strdup(text);
    c->count++;
}

void fy_cfg_set(void *cfg, const char *sec, const char *key, const char *utf8_val)
{
    Cfg *c = (Cfg *)cfg;
    if (!c || !sec || !key || !utf8_val) return;
    int sec_end;
    int i = find_key(c, sec, key, &sec_end);
    char line[2048];
    snprintf(line, sizeof(line), "%s=%s", key, utf8_val);
    if (i >= 0) {
        free(c->lines[i]);
        c->lines[i] = fy_strdup(line);
    } else if (sec_end < c->count) {
        cfg_insert_line(c, sec_end, line);           /* 段内末尾追加 */
    } else {
        /* 段不存在（或段在文件末尾）：补段头 */
        if (sec_end == c->count) {
            char hdr[256];
            snprintf(hdr, sizeof(hdr), "[%s]", sec);
            /* 判断段是否已存在但无此键 */
            int has_sec = 0;
            for (int k = 0; k < c->count; k++) {
                size_t L = strlen(c->lines[k]);
                if (L > 2 && c->lines[k][0] == '[' &&
                    strncmp(c->lines[k] + 1, sec, strlen(sec)) == 0 &&
                    c->lines[k][1 + strlen(sec)] == ']') { has_sec = 1; break; }
            }
            if (!has_sec && c->count > 0) cfg_insert_line(c, c->count, "");
            if (!has_sec) cfg_insert_line(c, c->count, hdr);
        }
        cfg_insert_line(c, c->count, line);
    }
    c->dirty = 1;
}

int fy_cfg_save(void *cfg)
{
    Cfg *c = (Cfg *)cfg;
    if (!c) return -1;
    FILE *f = fopen(c->path_gbk, "wb");
    if (!f) return -1;
    for (int i = 0; i < c->count; i++) {
        char *g = fy_utf8_to_gbk(c->lines[i]);
        if (g) { fputs(g, f); free(g); } else fputs(c->lines[i], f);
        fputs("\r\n", f);
    }
    fclose(f);
    c->dirty = 0;
    return 0;
}

void fy_cfg_close(void *cfg)
{
    Cfg *c = (Cfg *)cfg;
    if (!c) return;
    for (int i = 0; i < c->count; i++) free(c->lines[i]);
    free(c->lines);
    free(c->path_gbk);
    free(c);
}
