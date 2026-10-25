/* 极简 JSON 路径取值器
 * 用法：fy_json_str(json, "[0].translations[0].text", out, cap)
 *      fy_json_str(json, "detectedLanguage.language", out, cap)
 * 返回 1 = 找到，0 = 未找到。
 */
#include "fastcore.h"
#include "json.h"
#include <string.h>
#include <stdlib.h>

typedef struct {
    const char *s;
    const char *path;
} Jctx;

static void jskip(Jctx *j)
{
    while (*j->s == ' ' || *j->s == '\t' || *j->s == '\r' || *j->s == '\n')
        j->s++;
}

static int jhex4(const char *p)
{
    int v = 0;
    for (int i = 0; i < 4; i++) {
        char c = p[i];
        v <<= 4;
        if (c >= '0' && c <= '9') v |= c - '0';
        else if (c >= 'a' && c <= 'f') v |= c - 'a' + 10;
        else if (c >= 'A' && c <= 'F') v |= c - 'A' + 10;
        else return -1;
    }
    return v;
}

static void utf8_put(char *out, int *o, int cap, unsigned cp)
{
    if (*o + 4 >= cap) return;
    if (cp < 0x80) {
        out[(*o)++] = (char)cp;
    } else if (cp < 0x800) {
        out[(*o)++] = (char)(0xC0 | (cp >> 6));
        out[(*o)++] = (char)(0x80 | (cp & 0x3F));
    } else if (cp < 0x10000) {
        out[(*o)++] = (char)(0xE0 | (cp >> 12));
        out[(*o)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[(*o)++] = (char)(0x80 | (cp & 0x3F));
    } else {
        out[(*o)++] = (char)(0xF0 | (cp >> 18));
        out[(*o)++] = (char)(0x80 | ((cp >> 12) & 0x3F));
        out[(*o)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[(*o)++] = (char)(0x80 | (cp & 0x3F));
    }
}

/* 解析光标处的字符串字面量到 out（处理转义） */
static int jstring(Jctx *j, char *out, int cap)
{
    if (*j->s != '"') return 0;
    j->s++;
    int o = 0;
    while (*j->s && *j->s != '"') {
        if (*j->s == '\\') {
            j->s++;
            switch (*j->s) {
            case '"':  out[o++] = '"';  j->s++; break;
            case '\\': out[o++] = '\\'; j->s++; break;
            case '/':  out[o++] = '/';  j->s++; break;
            case 'b':  out[o++] = '\b'; j->s++; break;
            case 'f':  out[o++] = '\f'; j->s++; break;
            case 'n':  out[o++] = '\n'; j->s++; break;
            case 'r':  out[o++] = '\r'; j->s++; break;
            case 't':  out[o++] = '\t'; j->s++; break;
            case 'u': {
                int cp = jhex4(j->s + 1);
                if (cp < 0) return 0;
                j->s += 5;
                if (cp >= 0xD800 && cp <= 0xDBFF && j->s[0] == '\\' && j->s[1] == 'u') {
                    int lo = jhex4(j->s + 2);
                    if (lo >= 0xDC00 && lo <= 0xDFFF) {
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        j->s += 6;
                    }
                }
                utf8_put(out, &o, cap, (unsigned)cp);
                break;
            }
            default: return 0;
            }
        } else {
            if (o + 1 >= cap) return 0;
            out[o++] = *j->s++;
        }
        if (o + 8 >= cap) return 0;  /* 留余量 */
    }
    if (*j->s != '"') return 0;
    j->s++;
    out[o] = 0;
    return 1;
}

/* 跳过当前值（对象/数组/字符串/标量），返回结束后的位置 */
static int jskip_value(Jctx *j);

static int jskip_value(Jctx *j)
{
    jskip(j);
    if (*j->s == '"') {
        while (*++j->s && *j->s != '"')
            if (*j->s == '\\') j->s++;
        if (*j->s == '"') j->s++;
        return 1;
    }
    if (*j->s == '{') {
        int depth = 0;
        do {
            if (!*j->s) return 0;
            if (*j->s == '"') {          /* 键或字符串值，跳转义 */
                while (*++j->s && *j->s != '"')
                    if (*j->s == '\\') j->s++;
                if (*j->s == '"') j->s++;
                continue;
            }
            if (*j->s == '{') depth++;
            else if (*j->s == '}') depth--;
            j->s++;
        } while (depth > 0);
        return 1;
    }
    if (*j->s == '[') {
        int depth = 0;
        do {
            if (!*j->s) return 0;
            if (*j->s == '"') {
                while (*++j->s && *j->s != '"')
                    if (*j->s == '\\') j->s++;
                if (*j->s == '"') j->s++;
                continue;
            }
            if (*j->s == '[') depth++;
            else if (*j->s == ']') depth--;
            j->s++;
        } while (depth > 0);
        return 1;
    }
    /* 标量 */
    while (*j->s && *j->s != ',' && *j->s != '}' && *j->s != ']') j->s++;
    return 1;
}

/* 在当前对象中把光标移动到 path 指定的值处 */
static int jwalk(Jctx *j, const char *path)
{
    jskip(j);
    const char *p = path;
    while (*p) {
        jskip(j);
        if (*p == '[') {
            if (*j->s != '[') return 0;
            long idx = strtol(p + 1, (char **)&p, 10);
            if (*p == ']') p++;
            j->s++;
            jskip(j);
            if (*j->s == ']') return 0;   /* 越界 */
            for (long i = 0; i < idx; i++) {
                if (!jskip_value(j)) return 0;
                jskip(j);
                if (*j->s != ',') return 0;
                j->s++;
                jskip(j);
            }
        } else if (*p == '.') {
            p++;
            if (*p == '[') { p--; }       /* ".[0]" 归一为 "[0]" */
            else {
                /* 键名 */
                char key[128];
                int k = 0;
                while (*p && *p != '.' && *p != '[' && k < (int)sizeof(key) - 1)
                    key[k++] = *p++;
                key[k] = 0;
                if (*j->s != '{') return 0;
                j->s++;
                for (;;) {
                    jskip(j);
                    if (*j->s == '}') return 0;
                    char cur[128];
                    Jctx tmp = *j;
                    if (!jstring(&tmp, cur, (int)sizeof(cur))) return 0;
                    jskip(&tmp);
                    if (*tmp.s != ':') return 0;
                    tmp.s++;
                    jskip(&tmp);
                    if (strcmp(cur, key) == 0) {
                        *j = tmp;
                        break;
                    }
                    *j = tmp;
                    if (!jskip_value(j)) return 0;
                    jskip(j);
                    if (*j->s == ',') { j->s++; continue; }
                    if (*j->s == '}') return 0;
                }
            }
        } else if (*j->s == '{' || *j->s == '[') {
            /* path 首段无前导 '.'/'['（顶层键） */
            char key[128];
            int k = 0;
            while (*p && *p != '.' && *p != '[' && k < (int)sizeof(key) - 1)
                key[k++] = *p++;
            key[k] = 0;
            if (*j->s != '{') return 0;
            j->s++;
            for (;;) {
                jskip(j);
                if (*j->s == '}') return 0;
                char cur[128];
                Jctx tmp = *j;
                if (!jstring(&tmp, cur, (int)sizeof(cur))) return 0;
                jskip(&tmp);
                if (*tmp.s != ':') return 0;
                tmp.s++;
                jskip(&tmp);
                if (strcmp(cur, key) == 0) { *j = tmp; break; }
                *j = tmp;
                if (!jskip_value(j)) return 0;
                jskip(j);
                if (*j->s == ',') { j->s++; continue; }
                if (*j->s == '}') return 0;
            }
        } else {
            return 0;
        }
    }
    return 1;
}

int fy_json_str(const char *json, const char *path, char *out, int cap)
{
    if (!json || !path || !out || cap < 2) return 0;
    Jctx j = { json, path };
    if (!jwalk(&j, path)) return 0;
    jskip(&j);
    if (*j.s != '"') return 0;
    return jstring(&j, out, cap);
}

int fy_json_num(const char *json, const char *path, double *out)
{
    char buf[40];
    Jctx j = { json, path };
    if (!jwalk(&j, path)) return 0;
    jskip(&j);
    int i = 0;
    while ((j.s[i] >= '0' && j.s[i] <= '9') || j.s[i] == '-' || j.s[i] == '+' ||
           j.s[i] == '.' || j.s[i] == 'e' || j.s[i] == 'E') {
        if (i >= 38) return 0;
        buf[i] = j.s[i];
        i++;
    }
    buf[i] = 0;
    if (i == 0) return 0;
    *out = atof(buf);
    return 1;
}
