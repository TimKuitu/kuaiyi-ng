/* 检查更新：GitHub Releases 查询 + 下载安装包 */
#include "fastcore.h"
#include "http.h"
#include "json.h"
#include "str.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

void fy_set_error(const char *msg);   /* bing.c 提供 */

#define UPDATE_REPO "TimKuitu/kuaiyi-ng"
#define UPDATE_API  "https://api.github.com/repos/" UPDATE_REPO "/releases/latest"
#define UPDATE_PAGE "https://github.com/" UPDATE_REPO "/releases/latest"

/* "1.2.1" vs "1.2" 比较：remote > local 返回 1（支持 N.N / N.N.N） */
static int version_newer(const char *remote, const char *local)
{
    int r[3] = {0, 0, 0}, l[3] = {0, 0, 0};
    if (remote[0] == 'v' || remote[0] == 'V') remote++;
    if (local[0]  == 'v' || local[0]  == 'V') local++;
    sscanf(remote, "%d.%d.%d", &r[0], &r[1], &r[2]);
    sscanf(local,  "%d.%d.%d", &l[0], &l[1], &l[2]);
    for (int i = 0; i < 3; i++)
        if (r[i] != l[i]) return r[i] > l[i];
    return 0;
}

/* 从 releases/latest 页面 HTML 里提取 "/releases/tag/vX.Y" */
static int tag_from_html(const char *html, char *tag, int cap)
{
    const char *p = html;
    while ((p = strstr(p, "/releases/tag/")) != NULL) {
        p += strlen("/releases/tag/");
        const char *end = p;
        while (*end && *end != '"' && *end != '\'' && *end != '?' &&
               *end != '/' && *end != '<' && *end != '#') end++;
        size_t n = (size_t)(end - p);
        if (n >= 2 && n < 32) {
            if (n >= (size_t)cap) n = (size_t)cap - 1;
            memcpy(tag, p, n);
            tag[n] = 0;
            return 1;
        }
        p = end;
    }
    return 0;
}

/* 返回 1=有新版本（ver/url 已填），0=已是最新，-1=出错 */
int fy_check_update(const char *local_ver, char *ver_out, int vcap,
                    char *url_out, int ucap)
{
    char tag[64] = "";

    /* 优先 API；失败或限流（解析不到 tag_name）再走网页 */
    char *resp = fy_http_get(UPDATE_API, "Accept: application/vnd.github+json\r\n");
    if (resp) {
        fy_json_str(resp, "tag_name", tag, (int)sizeof(tag));
        free(resp);
    }
    if (!tag[0]) {
        /* 兜底 1：releases/latest 会 302 到 /releases/tag/vX.Y，读 Location 头最可靠 */
        char *loc = fy_http_get_location(UPDATE_PAGE);
        if (loc) {
            char *t = strstr(loc, "/releases/tag/");
            if (t) {
                t += strlen("/releases/tag/");
                char *end = t;
                while (*end && *end != '"' && *end != '?' && *end != '/' && *end != '#') end++;
                size_t n = (size_t)(end - t);
                if (n >= 2 && n < 32) { memcpy(tag, t, n); tag[n] = 0; }
            }
            free(loc);
        }
        /* 兜底 2：页面 HTML */
        if (!tag[0]) {
            char *page = fy_http_get(UPDATE_PAGE, "Accept: text/html\r\n");
            if (!page) { fy_set_error("无法连接 GitHub（网络受限或超时）"); return -1; }
            int ok = tag_from_html(page, tag, (int)sizeof(tag));
            free(page);
            if (!ok) { fy_set_error("版本信息解析失败（可能被限流）"); return -1; }
        }
    }
    /* tag 合法性校验：v?N.N 形式 */
    {
        const char *t = tag;
        if (*t == 'v' || *t == 'V') t++;
        if (!(*t >= '0' && *t <= '9')) { fy_set_error("版本信息解析失败"); return -1; }
    }

    /* 下载地址按命名规则拼（资产名自 v1.0 起固定） */
    char v[32];
    snprintf(v, sizeof(v), "%s", tag);
    if (v[0] == 'v' || v[0] == 'V') memmove(v, v + 1, strlen(v));
    char url[512];
    snprintf(url, sizeof(url),
             "https://github.com/" UPDATE_REPO "/releases/download/%s/KuaiYiNG_v%s_Setup.exe",
             tag, v);

    if (ver_out && vcap > 0) snprintf(ver_out, vcap, "%s", tag);
    if (url_out && ucap > 0) snprintf(url_out, ucap, "%s", url);
    return version_newer(tag, local_ver) ? 1 : 0;
}

/* 下载文件到 utf8 路径；progress 可为 NULL（done/total 字节） */
int fy_download_update(const char *url, const char *utf8_dest,
                       void (*progress)(long long done, long long total))
{
    long long n = fy_http_download(url, utf8_dest, progress);
    if (n <= 0) { fy_set_error("下载失败（网络错误或写入失败）"); return -1; }
    if (n < 100000) { fy_set_error("下载的文件异常（太小）"); return -1; }
    return 0;
}
