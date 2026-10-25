/* HTTP 客户端：WinINet 会话（自动保持 Cookie），供 bing.c 使用 */
#include "fastcore.h"
#include "http.h"
#include "str.h"
#include <windows.h>
#include <wininet.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

static HINTERNET g_session;

static const char *
fy_user_agent(void)
{
    return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
           "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
}

int fy_http_init(void)
{
    if (g_session) return 0;
    g_session = InternetOpenA(fy_user_agent(), INTERNET_OPEN_TYPE_PRECONFIG,
                              NULL, NULL, 0);
    if (!g_session) return -1;
    DWORD t = 8000;
    InternetSetOptionA(g_session, INTERNET_OPTION_CONNECT_TIMEOUT, &t, sizeof(t));
    t = 15000;
    InternetSetOptionA(g_session, INTERNET_OPTION_RECEIVE_TIMEOUT, &t, sizeof(t));
    t = 15000;
    InternetSetOptionA(g_session, INTERNET_OPTION_SEND_TIMEOUT, &t, sizeof(t));
    return 0;
}

static char *
read_all(HINTERNET hFile)
{
    size_t cap = 64 * 1024, len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) return NULL;
    DWORD got;
    for (;;) {
        if (len + 4096 > cap) {
            if (cap >= FY_HTTP_MAX) break;   /* 上限 6MB */
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) { free(buf); return NULL; }
            buf = nb;
        }
        if (!InternetReadFile(hFile, buf + len, (DWORD)(cap - len - 1), &got))
            break;
        if (got == 0) break;
        len += got;
    }
    buf[len] = 0;
    return buf;
}

char *fy_http_get(const char *url, const char *extra_headers)
{
    if (fy_http_init() != 0) return NULL;
    char headers[512];
    if (extra_headers)
        snprintf(headers, sizeof(headers),
                 "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n%s",
                 extra_headers);
    else
        snprintf(headers, sizeof(headers),
                 "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n");
    HINTERNET hFile = InternetOpenUrlA(g_session, url, headers,
                                       extra_headers ? (DWORD)-1L : (DWORD)-1L,
                                       INTERNET_FLAG_RELOAD |
                                       INTERNET_FLAG_NO_CACHE_WRITE |
                                       INTERNET_FLAG_KEEP_CONNECTION, 0);
    if (!hFile) {
        fy_log_write("HTTP", "GET %s 失败 err=%lu", url, (unsigned long)GetLastError());
        return NULL;
    }
    char *body = read_all(hFile);
    InternetCloseHandle(hFile);
    if (body) fy_log_write("HTTP", "GET %s 返回 %u字节", url, (unsigned)strlen(body));
    return body;
}

char *fy_http_post(const char *url, const char *referer, const char *form)
{
    if (fy_http_init() != 0) return NULL;
    URL_COMPONENTSA uc;
    char host[256], path[1024];
    ZeroMemory(&uc, sizeof(uc));
    uc.dwStructSize = sizeof(uc);
    uc.lpszHostName = host;      uc.dwHostNameLength = sizeof(host);
    uc.lpszUrlPath = path;       uc.dwUrlPathLength = sizeof(path);
    if (!InternetCrackUrlA(url, 0, 0, &uc)) {
        fy_log_write("HTTP", "POST 地址解析失败 err=%lu %s",
                     (unsigned long)GetLastError(), url);
        return NULL;
    }

    int secure = (uc.nScheme == INTERNET_SCHEME_HTTPS);
    HINTERNET hConn = InternetConnectA(g_session, host,
                                       uc.nPort ? uc.nPort : (secure ? 443 : 80),
                                       NULL, NULL, INTERNET_SERVICE_HTTP, 0, 0);
    if (!hConn) {
        fy_log_write("HTTP", "POST 连接 %s 失败 err=%lu",
                     host, (unsigned long)GetLastError());
        return NULL;
    }

    char hdr[768];
    snprintf(hdr, sizeof(hdr),
             "Content-Type: application/x-www-form-urlencoded\r\n"
             "Accept: */*\r\n"
             "Origin: https://%s\r\n"
             "Referer: %s\r\n",
             host, referer ? referer : url);

    DWORD flags = INTERNET_FLAG_KEEP_CONNECTION | INTERNET_FLAG_NO_CACHE_WRITE;
    if (secure) flags |= INTERNET_FLAG_SECURE;
    HINTERNET hReq = HttpOpenRequestA(hConn, "POST", path, "HTTP/1.1", NULL, NULL,
                                      flags, 0);
    if (!hReq) {
        fy_log_write("HTTP", "POST 打开请求失败 err=%lu",
                     (unsigned long)GetLastError());
        InternetCloseHandle(hConn);
        return NULL;
    }

    fy_log_write("HTTP", "POST %s form=%u字节", url, (unsigned)strlen(form));

    char *body = NULL;
    DWORD send_err = 0;
    int sent = 0;
    if (HttpSendRequestA(hReq, hdr, (DWORD)-1L, (LPVOID)form, (DWORD)strlen(form))) {
        sent = 1;
        body = read_all(hReq);
    } else {
        send_err = GetLastError();
    }
    InternetCloseHandle(hReq);
    InternetCloseHandle(hConn);
    if (body)
        fy_log_write("HTTP", "POST 返回 %u字节", (unsigned)strlen(body));
    else if (sent)
        fy_log_write("HTTP", "POST 响应读取失败");
    else
        fy_log_write("HTTP", "POST 发送失败 err=%lu", (unsigned long)send_err);
    return body;
}

char *fy_http_post_json(const char *url, const char *bearer, const char *json)
{
    /* OpenAI 格式接口：POST JSON，Bearer 认证；支持 http 与 https */
    if (fy_http_init() != 0) return NULL;
    URL_COMPONENTSA uc;
    char host[256], path[1024];
    ZeroMemory(&uc, sizeof(uc));
    uc.dwStructSize = sizeof(uc);
    uc.lpszHostName = host;      uc.dwHostNameLength = sizeof(host);
    uc.lpszUrlPath = path;       uc.dwUrlPathLength = sizeof(path);
    if (!InternetCrackUrlA(url, 0, 0, &uc)) return NULL;

    int secure = (uc.nScheme == INTERNET_SCHEME_HTTPS);
    HINTERNET hConn = InternetConnectA(g_session, host,
                                       uc.nPort ? uc.nPort : (secure ? 443 : 80),
                                       NULL, NULL, INTERNET_SERVICE_HTTP, 0, 0);
    if (!hConn) return NULL;

    char hdr[1024];
    snprintf(hdr, sizeof(hdr),
             "Content-Type: application/json\r\n"
             "Accept: application/json\r\n"
             "Authorization: Bearer %s\r\n",
             bearer ? bearer : "");

    DWORD flags = INTERNET_FLAG_KEEP_CONNECTION | INTERNET_FLAG_NO_CACHE_WRITE;
    if (secure) flags |= INTERNET_FLAG_SECURE;

    HINTERNET hReq = HttpOpenRequestA(hConn, "POST", path, "HTTP/1.1", NULL, NULL,
                                      flags, 0);
    if (!hReq) { InternetCloseHandle(hConn); return NULL; }

    char *body = NULL;
    if (HttpSendRequestA(hReq, hdr, (DWORD)-1L, (LPVOID)json, (DWORD)strlen(json)))
        body = read_all(hReq);
    InternetCloseHandle(hReq);
    InternetCloseHandle(hConn);
    return body;
}

/* 二进制安全下载：直接写文件（utf8 路径），progress 回调可为 NULL */
long long fy_http_download(const char *url, const char *utf8_path,
                           void (*progress)(long long done, long long total))
{
    if (fy_http_init() != 0) return -1;
    char *path = fy_utf8_to_gbk(utf8_path);
    if (!path) return -1;
    FILE *f = fopen(path, "wb");
    if (!f) { free(path); return -1; }

    HINTERNET hFile = InternetOpenUrlA(g_session, url, NULL, 0,
                                       INTERNET_FLAG_RELOAD |
                                       INTERNET_FLAG_NO_CACHE_WRITE |
                                       INTERNET_FLAG_KEEP_CONNECTION, 0);
    if (!hFile) { fclose(f); free(path); return -1; }

    long long total = -1;
    DWORD cbSize = sizeof(total);
    char hdrBuf[64];
    if (HttpQueryInfoA(hFile, HTTP_QUERY_CONTENT_LENGTH | HTTP_QUERY_FLAG_NUMBER,
                       &total, &cbSize, NULL)) {
        if (total == 0) total = -1;
    }
    (void)hdrBuf;

    char buf[65536];
    DWORD got;
    long long done = 0;
    while (InternetReadFile(hFile, buf, sizeof(buf), &got) && got > 0) {
        if (fwrite(buf, 1, got, f) != got) {
            InternetCloseHandle(hFile); fclose(f); free(path);
            return -1;
        }
        done += got;
        if (progress) progress(done, total);
    }
    InternetCloseHandle(hFile);
    fclose(f);
    free(path);
    return done;
}

/* 请求（禁用自动重定向），返回 Location 头（堆字符串）；无重定向返回 NULL */
char *fy_http_get_location(const char *url)
{
    if (fy_http_init() != 0) return NULL;
    HINTERNET hFile = InternetOpenUrlA(g_session, url, NULL, 0,
                                       INTERNET_FLAG_RELOAD |
                                       INTERNET_FLAG_NO_CACHE_WRITE |
                                       INTERNET_FLAG_KEEP_CONNECTION |
                                       INTERNET_FLAG_NO_AUTO_REDIRECT, 0);
    if (!hFile) return NULL;
    char loc[1024];
    DWORD cb = sizeof(loc) - 1;
    loc[0] = 0;
    char *result = NULL;
    if (HttpQueryInfoA(hFile, HTTP_QUERY_LOCATION, loc, &cb, NULL) && loc[0]) {
        loc[cb] = 0;
        result = fy_strdup(loc);
    }
    InternetCloseHandle(hFile);
    return result;
}
