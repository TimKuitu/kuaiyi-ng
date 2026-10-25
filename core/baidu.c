/* 百度翻译开放平台 API（fanyi-api.baidu.com）
 * 文档：https://fanyi-api.baidu.com/doc/21
 * 签名：MD5(appid + q + salt + 密钥)，q 以 UTF-8 参与签名、URL 编码两次传输
 * 响应：{"from":"en","to":"zh","trans_result":[{"src":"..","dst":".."}]}
 *       错误：{"error_code":"54001","error_msg":"Invalid Sign"}
 */
#include "fastcore.h"
#include "http.h"
#include "json.h"
#include "str.h"
#include "md5.h"
#include <windows.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

void fy_set_error(const char *msg);   /* bing.c 提供 */

#define BAIDU_API "https://fanyi-api.baidu.com/api/trans/vip/translate"

char *fy_translate_baidu(const char *utf8_text, int mode,
                         const char *appid, const char *secret,
                         const char *api_url)
{
    if (!utf8_text || !utf8_text[0]) { fy_set_error("请输入要翻译的内容"); return NULL; }
    if (!appid || !appid[0])  { fy_set_error("百度 AppID 为空"); return NULL; }
    if (!secret || !secret[0]) { fy_set_error("百度密钥为空"); return NULL; }
    if (!api_url || !api_url[0]) api_url = BAIDU_API;
    /* 目标语：百度用 zh/en（to）；from 可用 auto */
    const char *from = "auto";
    const char *to;
    switch (mode) {
    case 1: to = "en"; break;                                   /* FY_ZH2EN */
    case 2: to = "zh"; break;                                   /* FY_EN2ZH */
    default: to = fy_has_cjk(utf8_text) ? "en" : "zh";
    }

    char salt[32];
    snprintf(salt, sizeof(salt), "%u", (unsigned)GetTickCount());

    /* 签名：appid+q+salt+密钥（q 为 UTF-8 原文） */
    char sign[33];
    {
        size_t n1 = strlen(appid), n2 = strlen(utf8_text), n3 = strlen(salt),
               n4 = strlen(secret);
        size_t total = n1 + n2 + n3 + n4;
        char *raw = (char *)malloc(total + 1);
        if (!raw) { fy_set_error("内存不足"); return NULL; }
        memcpy(raw, appid, n1);
        memcpy(raw + n1, utf8_text, n2);
        memcpy(raw + n1 + n2, salt, n3);
        memcpy(raw + n1 + n2 + n3, secret, n4);
        fy_md5_hex(raw, total, sign);
        free(raw);
    }

    /* q 只做一次 URL 编码随表单发送，签名用编码前原文——官方规则是
       服务端解码一次后按 MD5(appid+q原文+salt+密钥) 校验。
       之前误传两次编码，真接口必回 54001 Invalid Sign（mock 测不出） */
    if (strlen(utf8_text) > 6000) {
        fy_set_error("文本超过百度接口 6000 字节上限"); return NULL;
    }
    char qenc[18432];
    if (fy_urlencode(utf8_text, qenc, (int)sizeof(qenc)) < 0) {
        fy_set_error("文本过长"); return NULL;
    }
    char eappid[256], esalt[64], efrom[32], eto[32], esign[96];
    /* urlencode 失败（-1）时缓冲区未写入，绝不能再拿去拼表单 */
    if (fy_urlencode(appid, eappid, sizeof(eappid)) < 0 ||
        fy_urlencode(salt, esalt, sizeof(esalt)) < 0 ||
        fy_urlencode(from, efrom, sizeof(efrom)) < 0 ||
        fy_urlencode(to, eto, sizeof(eto)) < 0 ||
        fy_urlencode(sign, esign, sizeof(esign)) < 0) {
        fy_set_error("百度参数过长或含异常字符"); return NULL;
    }

    fy_log_write("百度", "请求 %u字节 to=%s", (unsigned)strlen(utf8_text), to);

    char url[1024], form[18432];
    snprintf(url, sizeof(url), "%s", api_url);
    snprintf(form, sizeof(form),
             "q=%s&from=%s&to=%s&appid=%s&salt=%s&sign=%s",
             qenc, efrom, eto, eappid, esalt, esign);

    char *resp = fy_http_post(url, "https://fanyi-api.baidu.com", form);
    if (!resp) { fy_set_error("百度接口连接失败（网络错误）"); return NULL; }

    char errcode[32] = "";
    fy_json_str(resp, "error_code", errcode, (int)sizeof(errcode));
    if (errcode[0]) {
        char msg[128] = "";
        fy_json_str(resp, "error_msg", msg, (int)sizeof(msg));
        const char *hint = msg[0] ? msg : errcode;
        char buf[256];
        if (strcmp(errcode, "54001") == 0 || strcmp(errcode, "54003") == 0)
            snprintf(buf, sizeof(buf), "百度接口错误 %s（%s）", errcode, hint);
        else if (strcmp(errcode, "52003") == 0)
            snprintf(buf, sizeof(buf), "百度接口：密钥或 AppID 无效");
        else if (strcmp(errcode, "58003") == 0)
            snprintf(buf, sizeof(buf), "百度接口：服务未开通（需在百度开放平台开通翻译）");
        else
            snprintf(buf, sizeof(buf), "百度接口错误 %s（%s）", errcode, hint);
        free(resp);
        fy_set_error(buf);
        return NULL;
    }

    /* 拼接全部 dst（多句时换行连接） */
    char out[8192];
    int off = 0, segs = 0;
    for (int i = 0; i < 32; i++) {
        char path[40], dst[2048];
        snprintf(path, sizeof(path), "trans_result[%d].dst", i);
        if (!fy_json_str(resp, path, dst, (int)sizeof(dst))) break;
        if (off) out[off++] = '\n';
        int n = snprintf(out + off, sizeof(out) - (size_t)off, "%s", dst);
        if (n < 0) break;
        off += n;
        segs++;
        if (off >= (int)sizeof(out) - 1) break;
    }
    free(resp);
    if (!off) { fy_set_error("百度返回内容解析失败"); return NULL; }
    fy_log_write("百度", "解析出 %d 段", segs);

    char *result = fy_strdup(out);
    if (!result) fy_set_error("内存不足");
    return result;
}
