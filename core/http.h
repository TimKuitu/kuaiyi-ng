#ifndef FY_HTTP_H
#define FY_HTTP_H

#define FY_HTTP_MAX (6 * 1024 * 1024)

int   fy_http_init(void);
char *fy_http_get(const char *url, const char *extra_headers);
char *fy_http_post(const char *url, const char *referer, const char *form);

#endif
