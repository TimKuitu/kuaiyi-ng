/* 配置/历史 模块自测：读老版 config、改写、历史读写 */
#include "fastcore.h"
#include "str.h"
#include <stdio.h>
#include <windows.h>

int wmain(void)
{
    SetConsoleOutputCP(65001);

    /* 1. 读取 GBK config */
    void *cfg = fy_cfg_open("D:\\Lenovo\\Desktop\\ToolBox\\快译\\fastT\\config");
    char v[256];
    if (!cfg) { printf("open failed\n"); return 1; }

    fy_cfg_get(cfg, "快捷按键", "隐藏界面1", "?", v, sizeof(v));
    printf("隐藏界面1 = %s\n", v);
    fy_cfg_get(cfg, "快捷按键", "隐藏界面2", "?", v, sizeof(v));
    printf("隐藏界面2 = %s\n", v);
    fy_cfg_get(cfg, "通用设置", "窗口宽", "?", v, sizeof(v));
    printf("窗口宽 = %s\n", v);
    fy_cfg_get(cfg, "通用设置", "不存在的键", "默认值", v, sizeof(v));
    printf("缺省测试 = %s\n", v);

    /* 2. 写新键 + 保存到临时文件 */
    fy_cfg_set(cfg, "通用设置", "窗口置顶", "1");
    fy_cfg_set(cfg, "通用设置", "缓存目录", "D:\\某目录");
    fy_cfg_set(cfg, "新段落", "测试键", "中文值");
    fy_cfg_set(cfg, "快捷按键", "隐藏界面2", "71");
    fy_cfg_set(cfg, "快捷按键", "新增键", "abc");

    /* 转存副本（不动原文件） */
    void *cfg2 = fy_cfg_open("C:\\Users\\Lenovo\\AppData\\Local\\Temp\\fastt_test_cfg");
    if (cfg2) fy_cfg_close(cfg2);

    /* 直接改路径再测一轮读写 */
    void *cfg3 = fy_cfg_open("C:\\Users\\Lenovo\\AppData\\Local\\Temp\\fastt_test_cfg2");
    fy_cfg_set(cfg3, "通用设置", "窗口置顶", "1");
    fy_cfg_set(cfg3, "通用设置", "缓存目录", "D:\\某目录");
    fy_cfg_set(cfg3, "新段落", "测试键", "中文值");
    fy_cfg_set(cfg3, "快捷按键", "新增键", "abc");
    fy_cfg_save(cfg3);
    fy_cfg_close(cfg3);

    /* 重新读回验证 + 检查 GBK 输出 */
    void *cfg4 = fy_cfg_open("C:\\Users\\Lenovo\\AppData\\Local\\Temp\\fastt_test_cfg2");
    fy_cfg_get(cfg4, "新段落", "测试键", "?", v, sizeof(v));
    printf("回读 新段落/测试键 = %s\n", v);
    fy_cfg_get(cfg4, "通用设置", "缓存目录", "?", v, sizeof(v));
    printf("回读 缓存目录 = %s\n", v);
    fy_cfg_close(cfg4);
    fy_cfg_close(cfg);

    /* 3. 历史 */
    void *h = fy_hist_open("C:\\Users\\Lenovo\\AppData\\Local\\Temp\\fastt_hist_test");
    fy_hist_push(h, "hello");
    fy_hist_push(h, "world");
    fy_hist_push(h, "测试");
    fy_hist_push(h, "hello");   /* 去重 */
    printf("历史条数 = %d（应为 3）\n", fy_hist_count(h));
    for (int i = 0; i < fy_hist_count(h); i++) {
        char *s = fy_hist_get(h, i);
        if (s) { printf("  [%d] %s\n", i, s); fy_free(s); }
    }
    fy_hist_close(h);
    return 0;
}
