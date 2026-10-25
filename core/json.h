#ifndef FY_JSON_H
#define FY_JSON_H

int fy_json_str(const char *json, const char *path, char *out, int cap);
int fy_json_num(const char *json, const char *path, double *out);

#endif
