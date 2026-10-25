/* HTTP 客户端：WinINet 会话（自动保持 Cookie），供 bing.c 使用 */
#include "fastcore.h"
#include "http.h"
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
    if (!hFile) return NULL;
    char *body = read_all(hFile);
    InternetCloseHandle(hFile);
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
    if (!InternetCrackUrlA(url, 0, 0, &uc)) return NULL;

    HINTERNET hConn = InternetConnectA(g_session, host,
                                       uc.nPort ? uc.nPort : INTERNET_DEFAULT_HTTPS_PORT,
                                       NULL, NULL, INTERNET_SERVICE_HTTP, 0, 0);
    if (!hConn) return NULL;

    char hdr[768];
    snprintf(hdr, sizeof(hdr),
             "Content-Type: application/x-www-form-urlencoded\r\n"
             "Accept: */*\r\n"
             "Origin: https://%s\r\n"
             "Referer: %s\r\n",
             host, referer ? referer : url);

    HINTERNET hReq = HttpOpenRequestA(hConn, "POST", path, "HTTP/1.1", NULL, NULL,
                                      INTERNET_FLAG_SECURE |
                                      INTERNET_FLAG_KEEP_CONNECTION |
                                      INTERNET_FLAG_NO_CACHE_WRITE, 0);
    if (!hReq) { InternetCloseHandle(hConn); return NULL; }

    char *body = NULL;
    if (HttpSendRequestA(hReq, hdr, (DWORD)-1L, (LPVOID)form, (DWORD)strlen(form)))
        body = read_all(hReq);
    InternetCloseHandle(hReq);
    InternetCloseHandle(hConn);
    return body;
}
