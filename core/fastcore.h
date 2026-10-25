/* 快译NG C 核心 - 对外接口
 * 约定：所有字符串参数/返回值一律 UTF-8；配置文件为 GBK 编码。
 */
#ifndef FASTCORE_H
#define FASTCORE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 翻译方向 */
#define FY_AUTO   0   /* 按内容自动判断 */
#define FY_ZH2EN  1
#define FY_EN2ZH  2

/* 单词命名格式 */
#define FY_FMT_RAW    0  /* 原样 */
#define FY_FMT_LOWER  1  /* 全小写 */
#define FY_FMT_UPPER  2  /* 全大写 */
#define FY_FMT_CAMEL  3  /* 驼峰 */
#define FY_FMT_SNAKE  4  /* 下划线 */
#define FY_FMT_KEBAB  5  /* 中划线 */

/* 通用 */
void  fy_free(void *p);                 /* 释放核心返回的堆内存 */
char *fy_strdup(const char *s);         /* C 堆复制（跨线程传递用） */
const char *fy_last_error(void);        /* 最近一次错误（UTF-8） */

/* 翻译：返回多行 UTF-8 文本（'\n' 分隔），失败返回 NULL */
char *fy_translate(const char *utf8_text, int mode);

/* OpenAI 格式 AI 翻译（未配置时 UI 层自行回退到 fy_translate） */
char *fy_translate_ai(const char *utf8_text, int mode, const char *base_url,
                      const char *api_key, const char *model);

/* 检查更新（GitHub Releases）：1=有新版 0=已最新 -1=失败 */
int   fy_check_update(const char *local_ver, char *ver_out, int vcap,
                      char *url_out, int ucap);
int   fy_download_update(const char *url, const char *utf8_dest,
                         void (*progress)(long long done, long long total));

/* 单词命名格式化：多词空格分隔，失败返回 NULL */
char *fy_format_word(const char *utf8_text, int fmt);

/* INI 配置（GBK，保留未知段/键不丢） */
void *fy_cfg_open(const char *utf8_path);
int   fy_cfg_get(void *cfg, const char *sec, const char *key,
                 const char *utf8_def, char *out, int cap);
void  fy_cfg_set(void *cfg, const char *sec, const char *key, const char *utf8_val);
int   fy_cfg_save(void *cfg);           /* 0 = 成功 */
void  fy_cfg_close(void *cfg);

/* 查询历史（缓存目录下 history.txt，UTF-8，最新在最后） */
void *fy_hist_open(const char *utf8_dir);
int   fy_hist_count(void *h);
char *fy_hist_get(void *h, int index);  /* index: 0=最旧 … count-1=最新 */
void  fy_hist_push(void *h, const char *utf8_text);
void  fy_hist_close(void *h);

#ifdef __cplusplus
}
#endif
#endif /* FASTCORE_H */
