/* Bing 免费翻译接口（网页版逆向）：
 *   1. GET  https://cn.bing.com/translator
 *      提取 IG、IID(data-iid="translator.NNNN")、params_AbusePreventionHelper = [key,"token",ttl]
 *   2. POST https://cn.bing.com/ttranslatev3?isVertical=1&&IG=..&IID=..&SFX=n
 *      表单 fromLang/to/tone=Casual/text/token/key  → JSON
 *   3. 单词查询：GET https://cn.bing.com/dict/search?q=word
 *      解析 <meta name="description">（含 英/美音标、释义、网络释义）
 */
#include "fastcore.h"
#include "http.h"
#include "json.h"
#include "str.h"
#include <windows.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>

static char g_err[256];

const char *fy_last_error(void) { return g_err; }

static void set_err(const char *msg)
{
    snprintf(g_err, sizeof(g_err), "%s", msg);
}

/* 供 ai.c / update.c 写入同一份错误信息 */
void fy_set_error(const char *msg)
{
    set_err(msg);
}

/* 全角标点按 UTF-8 前缀比较（'，'= EF BC 8C，'；'= EF BC 9B） */
static int at_fw(const char *p, const char *fw)
{
    return strncmp(p, fw, strlen(fw)) == 0;
}

static int at_sep(const char *p)
{
    return at_fw(p, "，") || at_fw(p, "；") || *p == ',' || *p == ';';
}

static char *trim_cn(char *s)
{
    while (at_sep(s) || *s == ' ' || *s == '\t') {
        if (at_fw(s, "，") || at_fw(s, "；")) s += 3;
        else s++;
    }
    char *e = s + strlen(s);
    for (;;) {
        if (e >= s + 3 && (at_fw(e - 3, "，") || at_fw(e - 3, "；"))) e -= 3;
        else if (e > s && (e[-1] == ' ' || e[-1] == ',' || e[-1] == ';')) e -= 1;
        else break;
    }
    *e = 0;
    return s;
}

/* ---------- 会话（IG/IID/token/key，约 1 小时有效，45 分钟刷新） ---------- */
static char s_ig[64], s_iid[32], s_token[96], s_key[32];
static ULONGLONG s_tick = 0;

static int refresh_session(void)
{
    if (s_ig[0] && s_token[0] && GetTickCount64() - s_tick < 45u * 60u * 1000u)
        return 0;

    char *page = fy_http_get("https://cn.bing.com/translator", NULL);
    if (!page) { set_err("无法连接必应翻译（网络不通或被防火墙拦截）"); return -1; }

    char *p = strstr(page, "IG:\"");
    if (!p) { free(page); set_err("翻译页面解析失败：找不到 IG"); return -1; }
    if (sscanf(p + 4, "%32[A-Z0-9]", s_ig) != 1)
        { free(page); set_err("翻译页面解析失败：IG 格式异常"); return -1; }

    p = strstr(page, "data-iid=\"translator.");
    if (p) {
        int n = 0;
        if (sscanf(p, "data-iid=\"translator.%d", &n) == 1 && n > 0)
            snprintf(s_iid, sizeof(s_iid), "translator.%d", n);
    }
    if (!s_iid[0]) snprintf(s_iid, sizeof(s_iid), "translator.5023");

    p = strstr(page, "params_AbusePreventionHelper = [");
    if (!p) { free(page); set_err("翻译页面解析失败：找不到 token"); return -1; }
    unsigned long long key = 0;
    char tok[96] = "";
    if (sscanf(p + 32, "%llu,\"%95[-A-Za-z0-9_]\"", &key, tok) != 2 || !tok[0])
        { free(page); set_err("翻译页面解析失败：token 格式异常"); return -1; }
    snprintf(s_key, sizeof(s_key), "%llu", key);
    strcpy(s_token, tok);
    s_tick = GetTickCount64();

    free(page);
    return 0;
}

/* ---------- 句子翻译 ---------- */
static char *
ttranslate(const char *text, const char *from, const char *to)
{
    if (refresh_session() != 0) return NULL;

    static char enc[8192];
    char efrom[32], eto[32], etok[192], ekey[32];
    if (fy_urlencode(text, enc, (int)sizeof(enc)) < 0) { set_err("文本过长"); return NULL; }
    fy_urlencode(from, efrom, sizeof(efrom));
    fy_urlencode(to, eto, sizeof(eto));
    fy_urlencode(s_token, etok, sizeof(etok));
    fy_urlencode(s_key, ekey, sizeof(ekey));

    char url[512], form[9216];
    snprintf(url, sizeof(url),
             "https://cn.bing.com/ttranslatev3?isVertical=1&&IG=%s&IID=%s&SFX=%d",
             s_ig, s_iid, (int)(GetTickCount() & 0xFFFF));
    snprintf(form, sizeof(form),
             "fromLang=%s&to=%s&tone=Casual&text=%s&token=%s&key=%s",
             efrom, eto, enc, etok, ekey);

    char *resp = fy_http_post(url, "https://cn.bing.com/translator", form);
    if (!resp) { set_err("翻译请求发送失败（网络错误）"); return NULL; }

    char trans[4096] = "", pin[4096] = "";
    if (!fy_json_str(resp, "[0].translations[0].text", trans, (int)sizeof(trans))) {
        char msg[128] = "";
        int have = fy_json_str(resp, "errorMessage", msg, (int)sizeof(msg));
        int captcha = strstr(resp, "ShowCaptcha") != NULL;
        free(resp);
        if (captcha)              set_err("必应要求人机验证，请稍后重试");
        else if (have && msg[0])  set_err(msg);
        else                      set_err("翻译结果解析失败");
        return NULL;
    }
    fy_json_str(resp, "[0].translations[0].transliteration.text", pin, (int)sizeof(pin));
    free(resp);

    size_t cap = strlen(trans) + strlen(pin) + 64;
    char *out = (char *)malloc(cap);
    if (!out) { set_err("内存不足"); return NULL; }
    if (pin[0] && strcmp(to, "zh-Hans") == 0)
        snprintf(out, cap, "%s\n拼音：%s", trans, pin);
    else
        snprintf(out, cap, "%s", trans);
    return out;
}

/* ---------- 单词词典（meta description 一次拿全） ---------- */
static void html_unescape(char *s)
{
    char *w = s;
    for (char *r = s; *r; ) {
        if (r[0] == '&' && r[1] == '#') {
            char *end = strchr(r, ';');
            if (end && end - r < 8) {
                long v = strtol(r + 2, NULL, 10);
                if (v > 0 && v < 0x110000) {
                    unsigned cp = (unsigned)v;
                    if (cp < 0x80) *w++ = (char)cp;
                    else if (cp < 0x800) {
                        *w++ = (char)(0xC0 | (cp >> 6));
                        *w++ = (char)(0x80 | (cp & 0x3F));
                    } else {
                        *w++ = (char)(0xE0 | (cp >> 12));
                        *w++ = (char)(0x80 | ((cp >> 6) & 0x3F));
                        *w++ = (char)(0x80 | (cp & 0x3F));
                    }
                    r = end + 1;
                    continue;
                }
            }
        }
        if (r[0] == '&' && strncmp(r, "&amp;", 5) == 0) { *w++ = '&'; r += 5; continue; }
        if (r[0] == '&' && strncmp(r, "&quot;", 6) == 0) { *w++ = '"'; r += 6; continue; }
        if (r[0] == '&' && strncmp(r, "&lt;", 4) == 0) { *w++ = '<'; r += 4; continue; }
        if (r[0] == '&' && strncmp(r, "&gt;", 4) == 0) { *w++ = '>'; r += 4; continue; }
        *w++ = *r++;
    }
    *w = 0;
}

/* 返回 1=命中词典；0=不是词条（调用方回退句子翻译） */
static int dict_lookup(const char *word, char **out)
{
    *out = NULL;
    char encw[512], url[1024];
    fy_urlencode(word, encw, sizeof(encw));
    snprintf(url, sizeof(url), "https://cn.bing.com/dict/search?q=%s", encw);

    char *page = fy_http_get(url, "Referer: https://cn.bing.com/dict\r\n");
    if (!page) return 0;

    char *m = strstr(page, "<meta name=\"description\" content=\"");
    int hit = 0;
    if (m) {
        m += strlen("<meta name=\"description\" content=\"");
        char *end = strstr(m, "\"");
        if (end && end - m < 6000) {
            char desc[6144];
            memcpy(desc, m, (size_t)(end - m));
            desc[end - m] = 0;
            html_unescape(desc);

            /* 必应词典为您提供X的释义，美[..]，英[..]，<释义>； 网络释义：..； */
            char *head = strstr(desc, "为您提供");
            char *de = strstr(desc, "的释义");
            if (head && de && de > head + 12) {
                size_t wlen = (size_t)(de - (head + 12));   /* “为您提供”=12 字节 */
                char w[128];
                if (wlen < sizeof(w)) {
                    memcpy(w, head + 12, wlen);
                    w[wlen] = 0;

                    char *pus = strstr(de, "，美");
                    char *puk = strstr(de, "，英");
                    char us[160] = "", uk[160] = "";
                    char *uk_end = NULL;
                    if (pus && puk && puk > pus) {
                        char *us_start = pus + strlen("，美");
                        size_t n = (size_t)(puk - us_start);
                        if (n < sizeof(us)) { memcpy(us, us_start, n); us[n] = 0; }
                        char *uk_start = puk + strlen("，英");
                        uk_end = uk_start;
                        while (*uk_end && !at_sep(uk_end)) uk_end++;
                        n = (size_t)(uk_end - uk_start);
                        if (n < sizeof(uk)) { memcpy(uk, uk_start, n); uk[n] = 0; }
                    }

                    char *pweb = strstr(desc, "网络释义：");
                    char defs[2048] = "", web[2048] = "";
                    char *pdefs = uk_end ? uk_end : de;
                    while (*pdefs && !at_sep(pdefs)) pdefs++;   /* 跳到释义段分隔符 */
                    while (at_sep(pdefs)) {
                        if (at_fw(pdefs, "，") || at_fw(pdefs, "；")) pdefs += 3;
                        else pdefs++;
                    }
                    if (pdefs && pweb && pweb > pdefs) {
                        size_t n = (size_t)(pweb - pdefs);
                        if (n >= sizeof(defs)) n = sizeof(defs) - 1;
                        memcpy(defs, pdefs, n);
                        defs[n] = 0;
                    }
                    if (pweb) {
                        const char *wt = "网络释义：";
                        char *pe = pweb + strlen(wt);
                        while (*pe && !at_fw(pe, "；") && *pe != ';') pe++;
                        size_t n = (size_t)(pe - (pweb + strlen(wt)));
                        if (n >= sizeof(web)) n = sizeof(web) - 1;
                        memcpy(web, pweb + strlen(wt), n);
                        web[n] = 0;
                    }

                    size_t cap = strlen(w) + strlen(uk) + strlen(us) +
                                 strlen(defs) + strlen(web) + 96;
                    char *buf = (char *)malloc(cap);
                    if (buf) {
                        int off = snprintf(buf, cap, "%s%s%s%s%s%s%s",
                                           w,
                                           (uk[0] || us[0]) ? "  " : "",
                                           uk[0] ? "英" : "", uk,
                                           (uk[0] && us[0]) ? " " : "",
                                           us[0] ? "美" : "", us);
                        char *tdefs = trim_cn(defs);
                        char *tweb = trim_cn(web);
                        if (tdefs[0]) off += snprintf(buf + off, cap - (size_t)off,
                                                      "\n%s", tdefs);
                        if (tweb[0]) off += snprintf(buf + off, cap - (size_t)off,
                                                     "\n网络释义：%s", tweb);
                        (void)off;
                        *out = buf;
                        hit = 1;
                    }
                }
            }
        }
    }
    free(page);
    return hit;
}

/* ---------- 对外主入口 ---------- */
static int is_plain_word(const char *s)
{
    size_t n = strlen(s);
    if (n < 1 || n > 40) return 0;
    for (const char *p = s; *p; p++)
        if (!isalpha((unsigned char)*p) && *p != '-' && *p != '\'')
            return 0;
    return 1;
}

char *fy_translate(const char *utf8_text, int mode)
{
    if (!utf8_text || !utf8_text[0]) { set_err("请输入要翻译的内容"); return NULL; }

    char *text = fy_strdup(utf8_text);
    if (!text) { set_err("内存不足"); return NULL; }
    char *e = text + strlen(text);
    while (e > text && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\r' || e[-1] == '\n')) e--;
    *e = 0;
    char *b = text;
    while (*b == ' ' || *b == '\t' || *b == '\r' || *b == '\n') b++;

    const char *from, *to;
    switch (mode) {
    case FY_ZH2EN: from = "zh-Hans"; to = "en";      break;
    case FY_EN2ZH: from = "en";      to = "zh-Hans"; break;
    default:
        if (fy_has_cjk(b)) { from = "zh-Hans"; to = "en"; }
        else               { from = "en";      to = "zh-Hans"; }
    }

    char *out = NULL;
    /* 单个英文单词：优先词典（音标+释义），失败回退句子翻译 */
    if (is_plain_word(b) && strcmp(to, "zh-Hans") == 0) {
        dict_lookup(b, &out);
        if (out) {
            char *tr = ttranslate(b, from, to);
            if (tr) {
                size_t cap = strlen(out) + strlen(tr) + 16;
                char *merged = (char *)realloc(out, cap);
                if (merged) {
                    snprintf(merged + strlen(merged), cap - strlen(merged),
                             "\n译：%s", tr);
                    out = merged;
                }
                free(tr);
            }
            free(text);
            return out;
        }
    }

    out = ttranslate(b, from, to);
    free(text);
    return out;
}
