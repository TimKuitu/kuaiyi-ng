#ifndef FY_MD5_H
#define FY_MD5_H

#include <stddef.h>

/* 计算 MD5，输出 32 字符小写十六进制 + '\0'（out 需 33 字节） */
void fy_md5_hex(const void *data, size_t len, char out[33]);

#endif
