#ifndef FY_HTTP_H
#define FY_HTTP_H

#define FY_HTTP_MAX (6 * 1024 * 1024)

int   fy_http_init(void);
char *fy_http_get(const char *url, const char *extra_headers);
char *fy_http_post(const char *url, const char *referer, const char *form);
char *fy_http_post_json(const char *url, const char *bearer, const char *json);
char *fy_http_get_location(const char *url);
long long fy_http_download(const char *url, const char *utf8_path, void (*progress)(long long done, long long total));

#endif
