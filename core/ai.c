/* OpenAI 格式 AI 翻译（DeepSeek / OpenAI / Moonshot / Ollama 等兼容接口） */
#include "fastcore.h"
#include "http.h"
#include "json.h"
#include "str.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

void fy_set_error(const char *msg);   /* bing.c 提供 */

/* JSON 字符串转义（写入 out，返回写入长度） */
static int json_escape(const char *s, char *out, int cap)
{
    int o = 0;
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        char c = *p;
        if (o + 8 >= cap) return -1;
        switch (c) {
        case '"':  out[o++] = '\\'; out[o++] = '"';  break;
        case '\\': out[o++] = '\\'; out[o++] = '\\'; break;
        case '\n': out[o++] = '\\'; out[o++] = 'n';  break;
        case '\r': out[o++] = '\\'; out[o++] = 'r';  break;
        case '\t': out[o++] = '\\'; out[o++] = 't';  break;
        default:
            if ((unsigned char)c < 0x20) {
                o += snprintf(out + o, 8, "\\u%04x", c);
            } else {
                out[o++] = c;
            }
        }
    }
    out[o] = 0;
    return o;
}

/* 拼接接口完整地址：base[/v1]/chat/completions */
static void build_url(const char *base, char *url, int cap)
{
    /* 去掉末尾斜杠 */
    size_t n = strlen(base);
    char b[512];
    snprintf(b, sizeof(b), "%s", base);
    while (n > 0 && (b[n - 1] == '/' || b[n - 1] == ' ')) b[--n] = 0;

    if (n > 17 && strcmp(b + n - 17, "/chat/completions") == 0)
        snprintf(url, cap, "%s", b);
    else if (n > 3 && strcmp(b + n - 3, "/v1") == 0)
        snprintf(url, cap, "%s/chat/completions", b);
    else
        snprintf(url, cap, "%s/v1/chat/completions", b);
}

char *fy_translate_ai(const char *utf8_text, int mode,
                      const char *base_url, const char *api_key,
                      const char *model)
{
    if (!utf8_text || !utf8_text[0]) { fy_set_error("请输入要翻译的内容"); return NULL; }
    if (!base_url || !base_url[0])   { fy_set_error("AI 接口地址为空");    return NULL; }
    if (!api_key  || !api_key[0])    { fy_set_error("API 密钥为空");       return NULL; }
    if (!model    || !model[0])      model = "deepseek-chat";

    /* 方向与提示词 */
    const char *target;
    switch (mode) {
    case 1: target = "English";  break;    /* FY_ZH2EN */
    case 2: target = "Chinese (Simplified)"; break;  /* FY_EN2ZH */
    default:
        target = fy_has_cjk(utf8_text) ? "English" : "Chinese (Simplified)";
    }
    char sys[256];
    snprintf(sys, sizeof(sys),
             "You are a translation engine. Translate the user's text into %s. "
             "Output ONLY the translation itself, with no explanations or quotes.", target);

    char eSys[512], eText[8192];
    if (json_escape(sys, eSys, sizeof(eSys)) < 0)   { fy_set_error("文本过长"); return NULL; }
    if (json_escape(utf8_text, eText, sizeof(eText)) < 0) { fy_set_error("文本过长"); return NULL; }

    char eModel[128], eKey[512];
    json_escape(model, eModel, sizeof(eModel));
    json_escape(api_key, eKey, sizeof(eKey));

    char url[768], body[10240];
    build_url(base_url, url, sizeof(url));
    snprintf(body, sizeof(body),
             "{\"model\":\"%s\",\"temperature\":0,"
             "\"messages\":[{\"role\":\"system\",\"content\":\"%s\"},"
             "{\"role\":\"user\",\"content\":\"%s\"}]}",
             eModel, eSys, eText);

    char *resp = fy_http_post_json(url, api_key, body);
    if (!resp) { fy_set_error("AI 接口连接失败（网络错误或地址不对）"); return NULL; }

    char content[8192] = "";
    if (!fy_json_str(resp, "choices[0].message.content", content, (int)sizeof(content))) {
        char msg[256] = "";
        int have = fy_json_str(resp, "error.message", msg, (int)sizeof(msg));
        int badkey = strstr(resp, "invalid_api_key") != NULL ||
                     strstr(resp, "Authentication Fallback") != NULL ||
                     strstr(resp, "401") != NULL;
        free(resp);
        if (badkey)               fy_set_error("API 密钥无效（401）");
        else if (have && msg[0])  fy_set_error(msg);
        else                      fy_set_error("AI 返回格式解析失败");
        return NULL;
    }
    free(resp);
    if (!content[0]) { fy_set_error("AI 返回了空内容"); return NULL; }

    /* 去掉可能的引号包裹与首尾空白 */
    char *s = content;
    size_t n = strlen(s);
    if (n >= 2 && s[0] == '"' && s[n - 1] == '"') { s[n - 1] = 0; s++; }
    while (*s == ' ' || *s == '\n' || *s == '\r') s++;
    char *e = s + strlen(s);
    while (e > s && (e[-1] == ' ' || e[-1] == '\n' || e[-1] == '\r')) e--;
    *e = 0;
    if (!s[0]) { fy_set_error("AI 返回了空内容"); return NULL; }

    char *out = fy_strdup(s);
    if (!out) fy_set_error("内存不足");
    return out;
}
