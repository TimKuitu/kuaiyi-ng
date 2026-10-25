#ifndef FY_STR_H
#define FY_STR_H

char *fy_gbk_to_utf8(const char *s);
char *fy_utf8_to_gbk(const char *s);
int   fy_urlencode(const char *s, char *out, int cap);
int   fy_has_cjk(const char *utf8);

#endif
