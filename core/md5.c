/* MD5（RFC 1321），用于百度翻译 API 签名 */
#include "md5.h"
#include <string.h>

static const unsigned T[64] = {
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a,
    0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
    0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340,
    0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8,
    0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
    0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
    0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92,
    0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
    0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391};

static const unsigned K[64] = {
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21};

#define F1(b, c, d) (((b) & (c)) | ((~b) & (d)))
#define F2(b, c, d) (((b) & (d)) | ((c) & (~d)))
#define F3(b, c, d) ((b) ^ (c) ^ (d))
#define F4(b, c, d) ((c) ^ (b | ~d))
#define ROL(x, s) (((x) << (s)) | ((x) >> (32 - (s))))

void fy_md5_hex(const void *data, size_t len, char out[33])
{
    const unsigned char *msg = (const unsigned char *)data;
    unsigned a0 = 0x67452301, b0 = 0xefcdab89;
    unsigned c0 = 0x98badcfe, d0 = 0x10325476;

    /* 消息带填充复制到局部缓冲（q 最长 6000 字节，缓冲 8KB 足够） */
    static unsigned char buf[8192];
    size_t newlen = len;
    if (newlen + 9 > sizeof(buf)) newlen = sizeof(buf) - 9;   /* 防溢出截断 */
    memcpy(buf, msg, newlen);
    buf[newlen] = 0x80;
    size_t pad = ((newlen + 9 + 63) / 64) * 64 - newlen;
    memset(buf + newlen + 1, 0, pad - 1);
    unsigned long long bits = (unsigned long long)len * 8;
    memcpy(buf + newlen + pad - 8, &bits, 8);
    size_t total = newlen + pad;

    for (size_t off = 0; off < total; off += 64) {
        unsigned m[16];
        memcpy(m, buf + off, 64);
        unsigned a = a0, b = b0, c = c0, d = d0;
        for (int i = 0; i < 64; i++) {
            unsigned f, g;
            if (i < 16)      { f = F1(b, c, d); g = i; }
            else if (i < 32) { f = F2(b, c, d); g = (5 * i + 1) & 15; }
            else if (i < 48) { f = F3(b, c, d); g = (3 * i + 5) & 15; }
            else             { f = F4(b, c, d); g = (7 * i) & 15; }
            unsigned tmp = d;
            d = c;
            c = b;
            b = b + ROL(a + f + T[i] + m[g], K[i]);
            a = tmp;
        }
        a0 += a; b0 += b; c0 += c; d0 += d;
    }

    static const char hex[] = "0123456789abcdef";
    unsigned state[4] = {a0, b0, c0, d0};
    for (int i = 0; i < 4; i++)
        for (int j = 0; j < 4; j++) {
            unsigned char byte = (unsigned char)(state[i] >> (j * 8));
            out[i * 8 + j * 2] = hex[byte >> 4];
            out[i * 8 + j * 2 + 1] = hex[byte & 15];
        }
    out[32] = 0;
}
