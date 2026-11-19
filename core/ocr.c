/* 快译NG 截屏 OCR 与原图中文替换渲染
 * - OCR：动态加载 combase.dll 调用 Windows 10/11 原生 Windows.Media.Ocr，零外部依赖
 * - 翻译：复用 fy_translate_chain(FY_EN2ZH)，多块合并单次请求，行数不一致时逐块回退
 * - 渲染：采样周边背景色擦除原英文，按框尺寸自适应字号绘制微软雅黑中文
 */
#define COBJMACROS
#include "fastcore.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

/* ---- WinRT 最小 COM ABI 声明（避免依赖 Windows SDK WinRT 头文件） ---- */
typedef void *FY_HSTRING;
typedef struct { void *reserved; char data[24]; } FY_HSTRING_HEADER;
typedef struct { float X, Y, Width, Height; } FY_WinRect;

typedef HRESULT (WINAPI *PFN_RoInitialize)(int);
typedef void    (WINAPI *PFN_RoUninitialize)(void);
typedef HRESULT (WINAPI *PFN_RoGetActivationFactory)(FY_HSTRING, REFIID, void **);
typedef HRESULT (WINAPI *PFN_WindowsCreateStringReference)(PCWSTR, UINT32, FY_HSTRING_HEADER *, FY_HSTRING *);
typedef PCWSTR  (WINAPI *PFN_WindowsGetStringRawBuffer)(FY_HSTRING, UINT32 *);
typedef HRESULT (WINAPI *PFN_WindowsDeleteString)(FY_HSTRING);

typedef struct {
    HRESULT (STDMETHODCALLTYPE *QueryInterface)(void *, REFIID, void **);
    ULONG   (STDMETHODCALLTYPE *AddRef)(void *);
    ULONG   (STDMETHODCALLTYPE *Release)(void *);
    HRESULT (STDMETHODCALLTYPE *GetIids)(void *, ULONG *, IID **);
    HRESULT (STDMETHODCALLTYPE *GetRuntimeClassName)(void *, FY_HSTRING *);
    HRESULT (STDMETHODCALLTYPE *GetTrustLevel)(void *, int *);
} FY_IInspectableVtbl;

static const GUID FY_IID_IAsyncInfo = {0x00000036, 0x0000, 0x0000, {0xC0,0x00,0x00,0x00,0x00,0x00,0x00,0x46}};
typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *get_Id)(void *, UINT32 *);
    HRESULT (STDMETHODCALLTYPE *get_Status)(void *, int *);
    HRESULT (STDMETHODCALLTYPE *get_ErrorCode)(void *, HRESULT *);
    HRESULT (STDMETHODCALLTYPE *Cancel)(void *);
    HRESULT (STDMETHODCALLTYPE *Close)(void *);
} FY_IAsyncInfoVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *put_Completed)(void *, void *);
    HRESULT (STDMETHODCALLTYPE *get_Completed)(void *, void **);
    HRESULT (STDMETHODCALLTYPE *GetResults)(void *, void **);
} FY_IAsyncOperationVtbl;

static const GUID FY_IID_ILanguageFactory = {0x9B0252AC, 0x0C27, 0x44F8, {0xB7,0x92,0x97,0x93,0xFB,0x66,0xC6,0x3E}};
typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *CreateLanguage)(void *, FY_HSTRING, void **);
} FY_ILanguageFactoryVtbl;

static const GUID FY_IID_IOcrEngineStatics = {0x5BFFA85A, 0x3384, 0x3540, {0x99,0x40,0x69,0x91,0x20,0xD4,0x28,0xA8}};
typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *get_MaxImageDimension)(void *, UINT32 *);
    HRESULT (STDMETHODCALLTYPE *get_AvailableRecognizerLanguages)(void *, void **);
    HRESULT (STDMETHODCALLTYPE *IsLanguageSupported)(void *, void *, BOOLEAN *);
    HRESULT (STDMETHODCALLTYPE *TryCreateFromLanguage)(void *, void *, void **);
    HRESULT (STDMETHODCALLTYPE *TryCreateFromUserProfileLanguages)(void *, void **);
} FY_IOcrEngineStaticsVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *RecognizeAsync)(void *, void *bitmap, void **asyncOp);
    HRESULT (STDMETHODCALLTYPE *get_RecognizerLanguage)(void *, void **);
} FY_IOcrEngineVtbl;

static const GUID FY_IID_ISoftwareBitmapStatics = {0xDF0385DB, 0x672F, 0x4A9D, {0x80,0x6E,0xC2,0x44,0x2F,0x34,0x3E,0x86}};
typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *Copy)(void *, void *, void **);
    HRESULT (STDMETHODCALLTYPE *Convert)(void *, void *, int, void **);
    HRESULT (STDMETHODCALLTYPE *ConvertWithAlpha)(void *, void *, int, int, void **);
    HRESULT (STDMETHODCALLTYPE *CreateCopyFromBuffer)(void *, void *src, int fmt, int w, int h, void **out);
} FY_ISoftwareBitmapStaticsVtbl;

static const GUID FY_IID_IBufferFactory = {0x71AF914D, 0xC10F, 0x484B, {0xBC,0x50,0x14,0xBC,0x62,0x3B,0x3A,0x27}};
typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *Create)(void *, UINT32 capacity, void **out);
} FY_IBufferFactoryVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *get_Capacity)(void *, UINT32 *);
    HRESULT (STDMETHODCALLTYPE *get_Length)(void *, UINT32 *);
    HRESULT (STDMETHODCALLTYPE *put_Length)(void *, UINT32);
} FY_IBufferVtbl;

static const GUID FY_IID_IBufferByteAccess = {0x905A0FEF, 0xBC53, 0x11DF, {0x8C,0x49,0x00,0x1E,0x4F,0xC6,0x86,0xDA}};
typedef struct {
    HRESULT (STDMETHODCALLTYPE *QueryInterface)(void *, REFIID, void **);
    ULONG   (STDMETHODCALLTYPE *AddRef)(void *);
    ULONG   (STDMETHODCALLTYPE *Release)(void *);
    HRESULT (STDMETHODCALLTYPE *Buffer)(void *, BYTE **);
} FY_IBufferByteAccessVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *get_Lines)(void *, void **);
    HRESULT (STDMETHODCALLTYPE *get_TextAngle)(void *, void **);
    HRESULT (STDMETHODCALLTYPE *get_Text)(void *, FY_HSTRING *);
} FY_IOcrResultVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *GetAt)(void *, UINT32 idx, void **item);
    HRESULT (STDMETHODCALLTYPE *get_Size)(void *, UINT32 *size);
} FY_IVectorViewVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *get_Words)(void *, void **);
    HRESULT (STDMETHODCALLTYPE *get_Text)(void *, FY_HSTRING *);
} FY_IOcrLineVtbl;

typedef struct {
    FY_IInspectableVtbl base;
    HRESULT (STDMETHODCALLTYPE *get_BoundingRect)(void *, FY_WinRect *);
    HRESULT (STDMETHODCALLTYPE *get_Text)(void *, FY_HSTRING *);
} FY_IOcrWordVtbl;

#define SAFE_REL(p) do { if (p) { (*(FY_IInspectableVtbl **)(p))->Release(p); (p) = NULL; } } while (0)

typedef struct {
    float x, y, w, h;
    char *text;
    int need_trans;
} OcrLineBox;

typedef struct {
    int x, y, w, h;
    int line_h;
    int n_lines;
    int skip_translate;
    char *en;
    char *zh;
} TextBlock;

static int is_cjk_cp(unsigned int cp) {
    return (cp >= 0x4E00u && cp <= 0x9FFFu) ||
           (cp >= 0x3400u && cp <= 0x4DBFu) ||
           (cp >= 0xF900u && cp <= 0xFAFFu) ||
           (cp >= 0x20000u && cp <= 0x2A6DFu);
}

static int is_cjk_or_fullwidth_w(WCHAR c) {
    return (c >= 0x4E00 && c <= 0x9FFF) ||
           (c >= 0x3400 && c <= 0x4DBF) ||
           (c >= 0xF900 && c <= 0xFAFF) ||
           (c >= 0x3000 && c <= 0x303F) ||
           (c >= 0xFF00 && c <= 0xFFEF);
}

/* 判断文本块是否需要英译中：
 * - 无拉丁字母（纯中文/纯数字/纯符号）-> 跳过
 * - 汉字为主（汉字等效宽度 >= 拉丁字母数）-> 跳过，原图保留
 * - 孤立单字母（如关闭按钮 X、序号 A）-> 跳过
 */
static int should_translate_block(const char *utf8) {
    if (!utf8 || !utf8[0]) return 0;
    int cjk_cnt = 0;
    int latin_cnt = 0;
    const unsigned char *p = (const unsigned char *)utf8;
    while (*p) {
        unsigned int cp = 0;
        if (*p < 0x80) {
            cp = *p++;
        } else if ((*p & 0xE0) == 0xC0 && p[1]) {
            cp = ((unsigned int)(p[0] & 0x1F) << 6) | (unsigned int)(p[1] & 0x3F);
            p += 2;
        } else if ((*p & 0xF0) == 0xE0 && p[1] && p[2]) {
            cp = ((unsigned int)(p[0] & 0x0F) << 12) |
                 ((unsigned int)(p[1] & 0x3F) << 6) |
                 (unsigned int)(p[2] & 0x3F);
            p += 3;
        } else if ((*p & 0xF8) == 0xF0 && p[1] && p[2] && p[3]) {
            cp = ((unsigned int)(p[0] & 0x07) << 18) |
                 ((unsigned int)(p[1] & 0x3F) << 12) |
                 ((unsigned int)(p[2] & 0x3F) << 6) |
                 (unsigned int)(p[3] & 0x3F);
            p += 4;
        } else {
            p++;
            continue;
        }
        if ((cp >= 'A' && cp <= 'Z') || (cp >= 'a' && cp <= 'z') ||
            (cp >= 0x00C0u && cp <= 0x024Fu)) {
            latin_cnt++;
        } else if (is_cjk_cp(cp)) {
            cjk_cnt++;
        }
    }
    if (latin_cnt < 2) return 0;
    if (cjk_cnt > 0 && cjk_cnt * 2 >= latin_cnt) return 0;
    return 1;
}

static char *wide_to_utf8(PCWSTR ws, int wlen) {
    if (!ws || wlen == 0) return fy_strdup("");
    int n = WideCharToMultiByte(CP_UTF8, 0, ws, wlen, NULL, 0, NULL, NULL);
    if (n <= 0) return fy_strdup("");
    char *s = (char *)malloc((size_t)n + 1);
    if (!s) return NULL;
    WideCharToMultiByte(CP_UTF8, 0, ws, wlen, s, n, NULL, NULL);
    s[n] = '\0';
    return s;
}

static int cmp_line_box(const void *a, const void *b) {
    const OcrLineBox *la = (const OcrLineBox *)a;
    const OcrLineBox *lb = (const OcrLineBox *)b;
    float min_h = la->h < lb->h ? la->h : lb->h;
    float y_diff = la->y - lb->y;
    if (fabsf(y_diff) <= min_h * 0.45f) {
        return (la->x < lb->x) ? -1 : (la->x > lb->x ? 1 : 0);
    }
    return (la->y < lb->y) ? -1 : 1;
}

/* 对 32-bit BGRA 位图做 2x 放大（小图提升 OCR 识别率），返回 malloc 缓冲 */
static void *upscale_bgra_2x(const void *src, int w, int h, int *out_w, int *out_h) {
    int nw = w * 2, nh = h * 2;
    HDC hdc_scr = GetDC(NULL);
    HDC hdc_s = CreateCompatibleDC(hdc_scr);
    HDC hdc_d = CreateCompatibleDC(hdc_scr);
    BITMAPINFO bmi_s = {0}, bmi_d = {0};
    bmi_s.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi_s.bmiHeader.biWidth = w;
    bmi_s.bmiHeader.biHeight = -h;
    bmi_s.bmiHeader.biPlanes = 1;
    bmi_s.bmiHeader.biBitCount = 32;
    bmi_s.bmiHeader.biCompression = BI_RGB;

    bmi_d = bmi_s;
    bmi_d.bmiHeader.biWidth = nw;
    bmi_d.bmiHeader.biHeight = -nh;

    void *bits_s = NULL, *bits_d = NULL;
    HBITMAP hbm_s = CreateDIBSection(hdc_s, &bmi_s, DIB_RGB_COLORS, &bits_s, NULL, 0);
    HBITMAP hbm_d = CreateDIBSection(hdc_d, &bmi_d, DIB_RGB_COLORS, &bits_d, NULL, 0);
    if (!hbm_s || !hbm_d || !bits_s || !bits_d) {
        if (hbm_s) DeleteObject(hbm_s);
        if (hbm_d) DeleteObject(hbm_d);
        DeleteDC(hdc_s); DeleteDC(hdc_d); ReleaseDC(NULL, hdc_scr);
        return NULL;
    }
    memcpy(bits_s, src, (size_t)w * (size_t)h * 4);
    HBITMAP old_s = (HBITMAP)SelectObject(hdc_s, hbm_s);
    HBITMAP old_d = (HBITMAP)SelectObject(hdc_d, hbm_d);
    SetStretchBltMode(hdc_d, HALFTONE);
    SetBrushOrgEx(hdc_d, 0, 0, NULL);
    StretchBlt(hdc_d, 0, 0, nw, nh, hdc_s, 0, 0, w, h, SRCCOPY);
    GdiFlush();

    void *out = malloc((size_t)nw * (size_t)nh * 4);
    if (out) memcpy(out, bits_d, (size_t)nw * (size_t)nh * 4);

    SelectObject(hdc_s, old_s);
    SelectObject(hdc_d, old_d);
    DeleteObject(hbm_s); DeleteObject(hbm_d);
    DeleteDC(hdc_s); DeleteDC(hdc_d); ReleaseDC(NULL, hdc_scr);
    *out_w = nw; *out_h = nh;
    return out;
}

/* 全局缓存的 WinRT OCR 上下文（MTA 进程级复用，避免每次截屏冷启动） */
static CRITICAL_SECTION g_ocr_cs;
static LONG g_ocr_cs_init = 0;
static struct {
    int ready;
    HMODULE combase;
    PFN_RoInitialize pRoInit;
    PFN_RoUninitialize pRoUninit;
    PFN_RoGetActivationFactory pRoFactory;
    PFN_WindowsCreateStringReference pWinStrRef;
    PFN_WindowsGetStringRawBuffer pWinStrBuf;
    PFN_WindowsDeleteString pWinStrDel;
    void *ocrEngine; /* 主引擎：优先 en-US（英文识别精度高） */
    void *zhEngine;  /* 辅引擎：可选 zh-Hans-CN/zh-CN（用于精准识别中文区域并跳过） */
    void *bufFactory;
    void *sbStatics;
} g_ocr = {0};

static void ensure_ocr_cs(void) {
    if (InterlockedCompareExchange(&g_ocr_cs_init, 1, 0) == 0) {
        InitializeCriticalSection(&g_ocr_cs);
        InterlockedExchange(&g_ocr_cs_init, 2);
    } else {
        while (InterlockedCompareExchange(&g_ocr_cs_init, 2, 2) != 2) Sleep(0);
    }
}

static void reset_ocr_ctx_locked(void) {
    SAFE_REL(g_ocr.sbStatics);
    SAFE_REL(g_ocr.bufFactory);
    SAFE_REL(g_ocr.zhEngine);
    SAFE_REL(g_ocr.ocrEngine);
    g_ocr.ready = 0;
}

static void *try_create_ocr_lang(void *ocrStatics, void *langFactory, const WCHAR *wtag, UINT32 wlen) {
    void *lang = NULL, *eng = NULL;
    FY_HSTRING_HEADER hh;
    FY_HSTRING hs = NULL;
    g_ocr.pWinStrRef(wtag, wlen, &hh, &hs);
    if (SUCCEEDED((*(FY_ILanguageFactoryVtbl **)langFactory)->CreateLanguage(langFactory, hs, &lang)) && lang) {
        (*(FY_IOcrEngineStaticsVtbl **)ocrStatics)->TryCreateFromLanguage(ocrStatics, lang, &eng);
        SAFE_REL(lang);
    }
    return eng;
}

static int init_ocr_ctx_locked(char **out_err) {
    if (g_ocr.ready) return 0;

    if (!g_ocr.combase) {
        g_ocr.combase = LoadLibraryA("combase.dll");
        if (!g_ocr.combase) {
            *out_err = fy_strdup("当前系统不支持 Windows OCR（缺少 combase.dll）");
            return -1;
        }
        g_ocr.pRoInit    = (PFN_RoInitialize)GetProcAddress(g_ocr.combase, "RoInitialize");
        g_ocr.pRoUninit  = (PFN_RoUninitialize)GetProcAddress(g_ocr.combase, "RoUninitialize");
        g_ocr.pRoFactory = (PFN_RoGetActivationFactory)GetProcAddress(g_ocr.combase, "RoGetActivationFactory");
        g_ocr.pWinStrRef = (PFN_WindowsCreateStringReference)GetProcAddress(g_ocr.combase, "WindowsCreateStringReference");
        g_ocr.pWinStrBuf = (PFN_WindowsGetStringRawBuffer)GetProcAddress(g_ocr.combase, "WindowsGetStringRawBuffer");
        g_ocr.pWinStrDel = (PFN_WindowsDeleteString)GetProcAddress(g_ocr.combase, "WindowsDeleteString");
        if (!g_ocr.pRoInit || !g_ocr.pRoFactory || !g_ocr.pWinStrRef || !g_ocr.pWinStrBuf || !g_ocr.pWinStrDel) {
            *out_err = fy_strdup("当前系统版本过低，不支持 Windows OCR（需 Win10/11）");
            return -1;
        }
        /* 保留一份 MTA 引用不 Uninit，确保缓存的 WinRT 对象不因计数归零被拆掉 */
        g_ocr.pRoInit(1);
    }

    void *ocrStatics = NULL, *langFactory = NULL;
    FY_HSTRING_HEADER hh;
    FY_HSTRING hs = NULL;

    g_ocr.pWinStrRef(L"Windows.Media.Ocr.OcrEngine", 27, &hh, &hs);
    if (FAILED(g_ocr.pRoFactory(hs, &FY_IID_IOcrEngineStatics, &ocrStatics)) || !ocrStatics) {
        *out_err = fy_strdup("无法初始化 Windows OCR 引擎");
        goto fail;
    }

    g_ocr.pWinStrRef(L"Windows.Globalization.Language", 28, &hh, &hs);
    const char *main_lang = "user-profile";
    const char *zh_lang = "none";
    if (SUCCEEDED(g_ocr.pRoFactory(hs, &FY_IID_ILanguageFactory, &langFactory)) && langFactory) {
        g_ocr.ocrEngine = try_create_ocr_lang(ocrStatics, langFactory, L"en-US", 5);
        if (g_ocr.ocrEngine) main_lang = "en-US";

        void *zh = try_create_ocr_lang(ocrStatics, langFactory, L"zh-Hans-CN", 10);
        if (zh) {
            zh_lang = "zh-Hans-CN";
        } else {
            zh = try_create_ocr_lang(ocrStatics, langFactory, L"zh-CN", 5);
            if (zh) zh_lang = "zh-CN";
        }

        if (!g_ocr.ocrEngine && zh) {
            g_ocr.ocrEngine = zh;
            main_lang = zh_lang;
            zh_lang = "same-as-main";
        } else {
            g_ocr.zhEngine = zh;
        }
    }
    if (!g_ocr.ocrEngine) {
        (*(FY_IOcrEngineStaticsVtbl **)ocrStatics)->TryCreateFromUserProfileLanguages(ocrStatics, &g_ocr.ocrEngine);
    }
    if (!g_ocr.ocrEngine) {
        *out_err = fy_strdup("系统未安装 OCR 语言包，请在 Windows 设置中安装中文或英语识别");
        goto fail;
    }
    fy_log_write("OCR", "已初始化 Windows OCR 引擎 (main=%s, zh_mask=%s)", main_lang, zh_lang);

    g_ocr.pWinStrRef(L"Windows.Storage.Streams.Buffer", 30, &hh, &hs);
    if (FAILED(g_ocr.pRoFactory(hs, &FY_IID_IBufferFactory, &g_ocr.bufFactory)) || !g_ocr.bufFactory) {
        *out_err = fy_strdup("无法创建 OCR 图像缓冲");
        goto fail;
    }

    g_ocr.pWinStrRef(L"Windows.Graphics.Imaging.SoftwareBitmap", 39, &hh, &hs);
    if (FAILED(g_ocr.pRoFactory(hs, &FY_IID_ISoftwareBitmapStatics, &g_ocr.sbStatics)) || !g_ocr.sbStatics) {
        *out_err = fy_strdup("无法初始化 SoftwareBitmap");
        goto fail;
    }

    SAFE_REL(langFactory);
    SAFE_REL(ocrStatics);
    g_ocr.ready = 1;
    return 0;

fail:
    SAFE_REL(langFactory);
    SAFE_REL(ocrStatics);
    reset_ocr_ctx_locked();
    return -1;
}

static int has_any_cjk(const char *utf8) {
    if (!utf8) return 0;
    const unsigned char *p = (const unsigned char *)utf8;
    while (*p) {
        unsigned int cp = 0;
        if (*p < 0x80) { cp = *p++; }
        else if ((*p & 0xE0) == 0xC0 && p[1]) { cp = ((unsigned int)(p[0] & 0x1F) << 6) | (unsigned int)(p[1] & 0x3F); p += 2; }
        else if ((*p & 0xF0) == 0xE0 && p[1] && p[2]) { cp = ((unsigned int)(p[0] & 0x0F) << 12) | ((unsigned int)(p[1] & 0x3F) << 6) | (unsigned int)(p[2] & 0x3F); p += 3; }
        else if ((*p & 0xF8) == 0xF0 && p[1] && p[2] && p[3]) { cp = ((unsigned int)(p[0] & 0x07) << 18) | ((unsigned int)(p[1] & 0x3F) << 12) | ((unsigned int)(p[2] & 0x3F) << 6) | (unsigned int)(p[3] & 0x3F); p += 4; }
        else { p++; continue; }
        if (is_cjk_cp(cp)) return 1;
    }
    return 0;
}

/* 对指定 OcrEngine 运行一次异步识别并提取行列表 */
static int recognize_lines_with_engine(void *engine, void *sb, float inv_scale,
                                       PFN_WindowsGetStringRawBuffer pWinStrBuf,
                                       PFN_WindowsDeleteString pWinStrDel,
                                       OcrLineBox **out_arr, int *out_n, char **out_err) {
    *out_arr = NULL;
    *out_n = 0;
    void *asyncOp = NULL, *asyncInfo = NULL, *ocrRes = NULL, *lines = NULL;

    if (FAILED((*(FY_IOcrEngineVtbl **)engine)->RecognizeAsync(engine, sb, &asyncOp)) || !asyncOp) {
        if (out_err) *out_err = fy_strdup("OCR 识别启动失败");
        goto done_err;
    }
    if (FAILED((*(FY_IInspectableVtbl **)asyncOp)->QueryInterface(asyncOp, &FY_IID_IAsyncInfo, &asyncInfo)) || !asyncInfo) {
        if (out_err) *out_err = fy_strdup("OCR 状态查询失败");
        goto done_err;
    }
    int status = 0;
    for (int tries = 0; tries < 2000; tries++) {
        (*(FY_IAsyncInfoVtbl **)asyncInfo)->get_Status(asyncInfo, &status);
        if (status != 0) break;
        Sleep(tries < 20 ? 1 : 5);
    }
    if (status != 1) {
        if (out_err) *out_err = fy_strdup("OCR 识别超时或失败");
        goto done_err;
    }
    if (FAILED((*(FY_IAsyncOperationVtbl **)asyncOp)->GetResults(asyncOp, &ocrRes)) || !ocrRes) {
        if (out_err) *out_err = fy_strdup("获取 OCR 结果失败");
        goto done_err;
    }
    if (FAILED((*(FY_IOcrResultVtbl **)ocrRes)->get_Lines(ocrRes, &lines)) || !lines) {
        if (out_err) *out_err = fy_strdup("读取 OCR 行列表失败");
        goto done_err;
    }

    UINT32 nLines = 0;
    (*(FY_IVectorViewVtbl **)lines)->get_Size(lines, &nLines);
    if (nLines == 0) goto done_ok;

    OcrLineBox *arr = (OcrLineBox *)calloc(nLines, sizeof(OcrLineBox));
    int valid = 0;
    for (UINT32 i = 0; i < nLines; i++) {
        void *line = NULL;
        if (FAILED((*(FY_IVectorViewVtbl **)lines)->GetAt(lines, i, &line)) || !line) continue;

        void *words = NULL;
        float min_x = 1e9f, min_y = 1e9f, max_x = -1e9f, max_y = -1e9f;
        WCHAR *wbuf = NULL;
        size_t wbuf_len = 0, wbuf_cap = 0;
        WCHAR prev_last = 0;
        float prev_right = 0.0f, prev_h = 0.0f;
        int has_prev = 0;

        if (SUCCEEDED((*(FY_IOcrLineVtbl **)line)->get_Words(line, &words)) && words) {
            UINT32 nWords = 0;
            (*(FY_IVectorViewVtbl **)words)->get_Size(words, &nWords);
            for (UINT32 j = 0; j < nWords; j++) {
                void *word = NULL;
                if (SUCCEEDED((*(FY_IVectorViewVtbl **)words)->GetAt(words, j, &word)) && word) {
                    FY_WinRect wr = {0, 0, 0, 0};
                    int got_rc = SUCCEEDED((*(FY_IOcrWordVtbl **)word)->get_BoundingRect(word, &wr));
                    if (got_rc) {
                        if (wr.X < min_x) min_x = wr.X;
                        if (wr.Y < min_y) min_y = wr.Y;
                        if (wr.X + wr.Width > max_x) max_x = wr.X + wr.Width;
                        if (wr.Y + wr.Height > max_y) max_y = wr.Y + wr.Height;
                    }
                    FY_HSTRING wt = NULL;
                    if (SUCCEEDED((*(FY_IOcrWordVtbl **)word)->get_Text(word, &wt)) && wt) {
                        UINT32 wtl = 0;
                        PCWSTR wts = pWinStrBuf(wt, &wtl);
                        if (wts && wtl > 0) {
                            while (wtl > 0 && (wts[0] == L' ' || wts[0] == L'\t')) { wts++; wtl--; }
                            while (wtl > 0 && (wts[wtl - 1] == L' ' || wts[wtl - 1] == L'\t')) { wtl--; }
                        }
                        if (wts && wtl > 0) {
                            int need_sp = 0;
                            if (has_prev) {
                                WCHAR cur_first = wts[0];
                                int prev_cjk = is_cjk_or_fullwidth_w(prev_last);
                                int cur_cjk  = is_cjk_or_fullwidth_w(cur_first);
                                float gap = got_rc ? (wr.X - prev_right) : 0.0f;
                                float ref_h = prev_h > 1.0f ? prev_h : (got_rc ? wr.Height : 14.0f);
                                if (!prev_cjk && !cur_cjk) {
                                    int tight_cur = (cur_first == L',' || cur_first == L'.' || cur_first == L';' ||
                                                     cur_first == L':' || cur_first == L'!' || cur_first == L'?' ||
                                                     cur_first == L')' || cur_first == L']' || cur_first == L'}' ||
                                                     cur_first == L'\'');
                                    int tight_prev = (prev_last == L'(' || prev_last == L'[' || prev_last == L'{' ||
                                                      prev_last == L'/' || prev_last == L'-' || prev_last == L'\'');
                                    /* 中文 OCR 引擎常把同一英文单词内相邻字母拆成多个 word，仅当水平间隙明显时才补空格 */
                                    if (!got_rc) {
                                        if (!tight_cur && !tight_prev) need_sp = 1;
                                    } else if (gap >= ref_h * 0.14f && !((tight_cur || tight_prev) && gap < ref_h * 0.25f)) {
                                        need_sp = 1;
                                    }
                                } else if (prev_cjk && cur_cjk) {
                                    if (gap > ref_h * 0.70f) need_sp = 1;
                                } else {
                                    if (gap > ref_h * 0.25f) need_sp = 1;
                                }
                            }
                            size_t need = wbuf_len + (size_t)need_sp + (size_t)wtl + 1;
                            if (need > wbuf_cap) {
                                size_t ncap = wbuf_cap ? wbuf_cap * 2 : 64;
                                while (ncap < need) ncap *= 2;
                                WCHAR *nbuf = (WCHAR *)realloc(wbuf, ncap * sizeof(WCHAR));
                                if (nbuf) { wbuf = nbuf; wbuf_cap = ncap; }
                            }
                            if (wbuf && wbuf_cap >= need) {
                                if (need_sp) wbuf[wbuf_len++] = L' ';
                                memcpy(wbuf + wbuf_len, wts, (size_t)wtl * sizeof(WCHAR));
                                wbuf_len += wtl;
                                wbuf[wbuf_len] = 0;
                                prev_last = wts[wtl - 1];
                                if (got_rc) {
                                    prev_right = wr.X + wr.Width;
                                    prev_h = wr.Height;
                                }
                                has_prev = 1;
                            }
                        }
                        pWinStrDel(wt);
                    }
                    SAFE_REL(word);
                }
            }
            SAFE_REL(words);
        }

        char *utf8 = NULL;
        if (wbuf && wbuf_len > 0) {
            utf8 = wide_to_utf8(wbuf, (int)wbuf_len);
        } else {
            FY_HSTRING lt = NULL;
            (*(FY_IOcrLineVtbl **)line)->get_Text(line, &lt);
            UINT32 wlen = 0;
            PCWSTR ws = lt ? pWinStrBuf(lt, &wlen) : NULL;
            utf8 = wide_to_utf8(ws, (int)wlen);
            if (lt) pWinStrDel(lt);
        }
        free(wbuf);
        SAFE_REL(line);

        if (utf8 && utf8[0] && max_x > min_x && max_y > min_y) {
            arr[valid].x = min_x * inv_scale;
            arr[valid].y = min_y * inv_scale;
            arr[valid].w = (max_x - min_x) * inv_scale;
            arr[valid].h = (max_y - min_y) * inv_scale;
            arr[valid].text = utf8;
            arr[valid].need_trans = should_translate_block(utf8);
            valid++;
        } else {
            free(utf8);
        }
    }
    *out_arr = arr;
    *out_n = valid;

done_ok:
    SAFE_REL(lines);
    SAFE_REL(ocrRes);
    SAFE_REL(asyncInfo);
    SAFE_REL(asyncOp);
    return 0;

done_err:
    SAFE_REL(lines);
    SAFE_REL(ocrRes);
    SAFE_REL(asyncInfo);
    SAFE_REL(asyncOp);
    return -1;
}

/* 判断矩形 a 是否与中文区域 b 存在显著重叠（说明 a 实际落在了中文行上） */
static int box_overlaps_zh(const OcrLineBox *a, const OcrLineBox *b) {
    float x1 = a->x > b->x ? a->x : b->x;
    float y1 = a->y > b->y ? a->y : b->y;
    float x2 = (a->x + a->w) < (b->x + b->w) ? (a->x + a->w) : (b->x + b->w);
    float y2 = (a->y + a->h) < (b->y + b->h) ? (a->y + a->h) : (b->y + b->h);
    if (x2 <= x1 || y2 <= y1) return 0;
    float inter_w = x2 - x1;
    float inter_h = y2 - y1;
    float min_h = a->h < b->h ? a->h : b->h;
    if (min_h <= 0.0f || inter_h < min_h * 0.45f) return 0;
    if (a->w > 0.0f && inter_w >= a->w * 0.35f) return 1;
    return 0;
}

/* 调用 Windows.Media.Ocr 识别文本行及外接矩形（英文主引擎 + 中文辅引擎交叉过滤） */
static int run_winrt_ocr(const void *bgra, int w, int h,
                         OcrLineBox **out_lines, int *out_count, char **out_err) {
    *out_lines = NULL;
    *out_count = 0;

    ensure_ocr_cs();
    EnterCriticalSection(&g_ocr_cs);
    int init_rc = init_ocr_ctx_locked(out_err);
    void *ocrEngine = g_ocr.ocrEngine;
    void *zhEngine = g_ocr.zhEngine;
    void *bufFactory = g_ocr.bufFactory;
    void *sbStatics = g_ocr.sbStatics;
    PFN_RoInitialize pRoInit = g_ocr.pRoInit;
    PFN_RoUninitialize pRoUninit = g_ocr.pRoUninit;
    PFN_WindowsGetStringRawBuffer pWinStrBuf = g_ocr.pWinStrBuf;
    PFN_WindowsDeleteString pWinStrDel = g_ocr.pWinStrDel;
    if (init_rc == 0) {
        if (ocrEngine) (*(FY_IInspectableVtbl **)ocrEngine)->AddRef(ocrEngine);
        if (zhEngine) (*(FY_IInspectableVtbl **)zhEngine)->AddRef(zhEngine);
        if (bufFactory) (*(FY_IInspectableVtbl **)bufFactory)->AddRef(bufFactory);
        if (sbStatics) (*(FY_IInspectableVtbl **)sbStatics)->AddRef(sbStatics);
    }
    LeaveCriticalSection(&g_ocr_cs);
    if (init_rc != 0) return -1;

    HRESULT hr_init = pRoInit(1); /* RO_INIT_MULTITHREADED */
    int need_uninit = (hr_init == S_OK || hr_init == S_FALSE);

    void *buf = NULL, *byteAccess = NULL, *sb = NULL;
    void *scaled_buf = NULL;

    /* 2. 小图 2x 放大喂给 OCR，提升小字识别率 */
    int ocr_w = w, ocr_h = h;
    const void *ocr_src = bgra;
    float inv_scale = 1.0f;
    if ((w < 400 || h < 120) && w * 2 <= 2400 && h * 2 <= 2400) {
        scaled_buf = upscale_bgra_2x(bgra, w, h, &ocr_w, &ocr_h);
        if (scaled_buf) {
            ocr_src = scaled_buf;
            inv_scale = 0.5f;
        }
    }

    /* 3. 构造 SoftwareBitmap (Bgra8 = 87) */
    UINT32 byte_sz = (UINT32)ocr_w * (UINT32)ocr_h * 4u;
    if (FAILED((*(FY_IBufferFactoryVtbl **)bufFactory)->Create(bufFactory, byte_sz, &buf)) || !buf) {
        *out_err = fy_strdup("分配 OCR 缓冲失败");
        goto cleanup;
    }
    (*(FY_IBufferVtbl **)buf)->put_Length(buf, byte_sz);
    if (FAILED((*(FY_IInspectableVtbl **)buf)->QueryInterface(buf, &FY_IID_IBufferByteAccess, &byteAccess)) || !byteAccess) {
        *out_err = fy_strdup("访问 OCR 缓冲失败");
        goto cleanup;
    }
    BYTE *dst = NULL;
    (*(FY_IBufferByteAccessVtbl **)byteAccess)->Buffer(byteAccess, &dst);
    memcpy(dst, ocr_src, byte_sz);

    if (FAILED((*(FY_ISoftwareBitmapStaticsVtbl **)sbStatics)->CreateCopyFromBuffer(sbStatics, buf, 87, ocr_w, ocr_h, &sb)) || !sb) {
        *out_err = fy_strdup("构造 OCR 图像失败");
        goto cleanup;
    }

    /* 4. 主引擎识别（en-US 保证英文完整准确） */
    OcrLineBox *arr = NULL;
    int valid = 0;
    if (recognize_lines_with_engine(ocrEngine, sb, inv_scale, pWinStrBuf, pWinStrDel,
                                    &arr, &valid, out_err) != 0) {
        goto cleanup;
    }

    /* 5. 若存在中文辅引擎，用中文引擎找出汉字为主的区域，屏蔽主引擎在中文上的误识 */
    if (zhEngine) {
        OcrLineBox *zh_arr = NULL;
        int zh_n = 0;
        if (recognize_lines_with_engine(zhEngine, sb, inv_scale, pWinStrBuf, pWinStrDel,
                                        &zh_arr, &zh_n, NULL) == 0 && zh_n > 0) {
            for (int i = 0; i < valid; i++) {
                if (!arr[i].need_trans) continue;
                for (int j = 0; j < zh_n; j++) {
                    if (!zh_arr[j].need_trans && has_any_cjk(zh_arr[j].text) &&
                        box_overlaps_zh(&arr[i], &zh_arr[j])) {
                        arr[i].need_trans = 0;
                        break;
                    }
                }
            }
            /* 把未被主引擎覆盖的中文行作为占位块加入（skip_translate=1），供排版避让不压中文 */
            int add_cnt = 0;
            for (int j = 0; j < zh_n; j++) {
                if (!zh_arr[j].need_trans && has_any_cjk(zh_arr[j].text)) add_cnt++;
            }
            if (add_cnt > 0) {
                OcrLineBox *merged = (OcrLineBox *)realloc(arr, (size_t)(valid + add_cnt) * sizeof(OcrLineBox));
                if (merged) {
                    arr = merged;
                    for (int j = 0; j < zh_n; j++) {
                        if (!zh_arr[j].need_trans && has_any_cjk(zh_arr[j].text)) {
                            arr[valid] = zh_arr[j];
                            arr[valid].need_trans = 0;
                            zh_arr[j].text = NULL;
                            valid++;
                        }
                    }
                }
            }
            for (int j = 0; j < zh_n; j++) free(zh_arr[j].text);
            free(zh_arr);
        }
    }

    if (valid > 1) qsort(arr, (size_t)valid, sizeof(OcrLineBox), cmp_line_box);
    *out_lines = arr;
    *out_count = valid;

    SAFE_REL(sb);
    SAFE_REL(sbStatics);
    SAFE_REL(byteAccess);
    SAFE_REL(buf);
    SAFE_REL(bufFactory);
    SAFE_REL(zhEngine);
    SAFE_REL(ocrEngine);
    free(scaled_buf);
    if (need_uninit && pRoUninit) pRoUninit();
    return 0;

cleanup:
    SAFE_REL(sb);
    SAFE_REL(sbStatics);
    SAFE_REL(byteAccess);
    SAFE_REL(buf);
    SAFE_REL(bufFactory);
    SAFE_REL(zhEngine);
    SAFE_REL(ocrEngine);
    free(scaled_buf);
    if (need_uninit && pRoUninit) pRoUninit();
    return -1;
}

/* 判断两行是否属于同一连续段落（同语言属性、左对齐接近、行高相近、行距紧凑） */
static int should_merge_lines(const TextBlock *blk, const OcrLineBox *ln) {
    /* 中文行与英文行不合并，避免中文把相邻英文拖入跳过或反之 */
    if (blk->skip_translate != (!ln->need_trans)) return 0;
    int prev_bottom = blk->y + blk->h;
    int gap = (int)ln->y - prev_bottom;
    int ref_h = blk->line_h > (int)ln->h ? blk->line_h : (int)ln->h;
    int min_h = blk->line_h < (int)ln->h ? blk->line_h : (int)ln->h;
    if (ref_h <= 0 || min_h <= 0) return 0;
    if (gap > (int)(ref_h * 0.65f) || gap < -(int)(min_h * 0.3f)) return 0;
    if ((float)min_h / (float)ref_h < 0.68f) return 0;
    int left_diff = abs((int)ln->x - blk->x);
    if (left_diff <= (int)(ref_h * 1.4f)) return 1;
    return 0;
}

static TextBlock *group_lines(const OcrLineBox *lines, int n, int *out_n) {
    if (n <= 0) { *out_n = 0; return NULL; }
    TextBlock *blks = (TextBlock *)calloc((size_t)n, sizeof(TextBlock));
    int nb = 0;
    for (int i = 0; i < n; i++) {
        if (nb > 0 && should_merge_lines(&blks[nb - 1], &lines[i])) {
            TextBlock *b = &blks[nb - 1];
            int nx1 = b->x < (int)lines[i].x ? b->x : (int)lines[i].x;
            int ny1 = b->y < (int)lines[i].y ? b->y : (int)lines[i].y;
            int bx2 = b->x + b->w;
            int lx2 = (int)(lines[i].x + lines[i].w);
            int by2 = b->y + b->h;
            int ly2 = (int)(lines[i].y + lines[i].h);
            int nx2 = bx2 > lx2 ? bx2 : lx2;
            int ny2 = by2 > ly2 ? by2 : ly2;
            b->x = nx1; b->y = ny1; b->w = nx2 - nx1; b->h = ny2 - ny1;
            b->line_h = (b->line_h * b->n_lines + (int)lines[i].h) / (b->n_lines + 1);
            b->n_lines++;
            size_t old_len = strlen(b->en);
            size_t add_len = strlen(lines[i].text);
            char *ns = (char *)realloc(b->en, old_len + 1 + add_len + 1);
            if (ns) {
                ns[old_len] = ' ';
                memcpy(ns + old_len + 1, lines[i].text, add_len + 1);
                b->en = ns;
            }
            b->skip_translate = !should_translate_block(b->en);
        } else {
            TextBlock *b = &blks[nb++];
            b->x = (int)floorf(lines[i].x);
            b->y = (int)floorf(lines[i].y);
            b->w = (int)ceilf(lines[i].w);
            b->h = (int)ceilf(lines[i].h);
            b->line_h = b->h > 0 ? b->h : 14;
            b->n_lines = 1;
            b->en = fy_strdup(lines[i].text);
            b->skip_translate = !lines[i].need_trans;
        }
    }
    *out_n = nb;
    return blks;
}

/* 清理译文中的词典音标/拼音附加行，适合直接覆盖在截图上 */
static char *clean_zh_for_overlay(const char *raw, int single_line) {
    if (!raw) return fy_strdup("");
    const char *p = raw;
    /* 跳过回退提示首行 */
    if (strncmp(p, "\xEF\xBC\x88", 3) == 0) {
        const char *nl = strchr(p, '\n');
        const char *hit = strstr(p, "已用必应翻译");
        if (nl && hit && hit < nl) p = nl + 1;
    }
    size_t len = strlen(p);
    char *out = (char *)malloc(len + 1);
    if (!out) return fy_strdup(p);
    char *dst = out;
    const char *cur = p;
    while (*cur) {
        const char *nl = strchr(cur, '\n');
        size_t l = nl ? (size_t)(nl - cur) : strlen(cur);
        while (l > 0 && cur[l - 1] == '\r') l--;
        /* 过滤空行、拼音行、音标行 */
        int skip = (l == 0);
        if (!skip && l >= 9 && strncmp(cur, "拼音：", 9) == 0) skip = 1;
        if (!skip && l >= 4 && (strncmp(cur, "美[", 4) == 0 || strncmp(cur, "英[", 4) == 0)) skip = 1;
        if (!skip && cur[0] == '[') skip = 1;
        if (!skip) {
            if (dst != out) *dst++ = '\n';
            memcpy(dst, cur, l);
            dst += l;
            if (single_line) break;
        }
        if (!nl) break;
        cur = nl + 1;
    }
    *dst = '\0';
    if (out[0] == '\0') {
        free(out);
        return fy_strdup(p);
    }
    return out;
}

/* ---------------- 本地短语翻译记忆库（O(1) 哈希 + LRU + 可校对） ---------------- */
#define OCR_HASH_BUCKETS 16384
#define OCR_CACHE_MAX    8000

typedef struct OcrCacheEntry {
    char *key;
    char *raw_en;
    char *zh;
    int   locked; /* 1 = '*' 人工校对，永久保留不淘汰 */
    struct OcrCacheEntry *hnext;
    struct OcrCacheEntry *prev;
    struct OcrCacheEntry *next;
} OcrCacheEntry;

static CRITICAL_SECTION g_cache_cs;
static LONG g_cache_cs_init = 0;
static struct {
    int inited;
    char dir[1024];
    wchar_t wpath[1024];
    FILETIME mtime;
    int has_mtime;
    int auto_count;
    OcrCacheEntry *buckets[OCR_HASH_BUCKETS];
    OcrCacheEntry *head;
    OcrCacheEntry *tail;
} g_cache = {0};

static void ensure_cache_cs(void) {
    if (InterlockedCompareExchange(&g_cache_cs_init, 1, 0) == 0) {
        InitializeCriticalSection(&g_cache_cs);
        InterlockedExchange(&g_cache_cs_init, 2);
    } else {
        while (InterlockedCompareExchange(&g_cache_cs_init, 2, 2) != 2) Sleep(0);
    }
}

static unsigned int fnv1a_hash(const char *s) {
    unsigned int h = 2166136261u;
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        h ^= *p;
        h *= 16777619u;
    }
    return h;
}

static void normalize_cache_key(const char *src, char *dst, size_t cap) {
    if (!src || cap == 0) { if (cap) dst[0] = 0; return; }
    size_t n = 0;
    int prev_sp = 1;
    for (const unsigned char *p = (const unsigned char *)src; *p && n + 1 < cap; p++) {
        if (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') {
            if (!prev_sp) { dst[n++] = ' '; prev_sp = 1; }
        } else {
            unsigned char c = *p;
            if (c >= 'A' && c <= 'Z') c = (unsigned char)(c - 'A' + 'a');
            dst[n++] = (char)c;
            prev_sp = 0;
        }
    }
    while (n > 0 && dst[n - 1] == ' ') n--;
    dst[n] = '\0';
}

static void lru_unlink(OcrCacheEntry *e) {
    if (e->prev) e->prev->next = e->next; else g_cache.head = e->next;
    if (e->next) e->next->prev = e->prev; else g_cache.tail = e->prev;
    e->prev = e->next = NULL;
}

static void lru_push_front(OcrCacheEntry *e) {
    e->prev = NULL;
    e->next = g_cache.head;
    if (g_cache.head) g_cache.head->prev = e;
    g_cache.head = e;
    if (!g_cache.tail) g_cache.tail = e;
}

static void cache_clear_locked(void) {
    OcrCacheEntry *cur = g_cache.head;
    while (cur) {
        OcrCacheEntry *nx = cur->next;
        free(cur->key);
        free(cur->raw_en);
        free(cur->zh);
        free(cur);
        cur = nx;
    }
    memset(g_cache.buckets, 0, sizeof(g_cache.buckets));
    g_cache.head = g_cache.tail = NULL;
    g_cache.auto_count = 0;
}

static OcrCacheEntry *cache_find_locked(const char *key) {
    unsigned int idx = fnv1a_hash(key) & (OCR_HASH_BUCKETS - 1);
    for (OcrCacheEntry *e = g_cache.buckets[idx]; e; e = e->hnext) {
        if (strcmp(e->key, key) == 0) return e;
    }
    return NULL;
}

static void cache_insert_mem_locked(const char *raw_en, const char *zh, int locked) {
    char key[128];
    normalize_cache_key(raw_en, key, sizeof(key));
    if (!key[0]) return;

    OcrCacheEntry *ex = cache_find_locked(key);
    if (ex) {
        if (ex->locked && !locked) return;
        free(ex->zh);
        ex->zh = fy_strdup(zh);
        if (locked && !ex->locked) {
            ex->locked = 1;
            if (g_cache.auto_count > 0) g_cache.auto_count--;
        }
        lru_unlink(ex);
        lru_push_front(ex);
        return;
    }

    OcrCacheEntry *e = (OcrCacheEntry *)calloc(1, sizeof(OcrCacheEntry));
    if (!e) return;
    e->key = fy_strdup(key);
    e->raw_en = fy_strdup(raw_en);
    e->zh = fy_strdup(zh);
    e->locked = locked;
    unsigned int idx = fnv1a_hash(key) & (OCR_HASH_BUCKETS - 1);
    e->hnext = g_cache.buckets[idx];
    g_cache.buckets[idx] = e;
    lru_push_front(e);
    if (!locked) g_cache.auto_count++;
}

static void cache_update_mtime_locked(void) {
    WIN32_FILE_ATTRIBUTE_DATA fad;
    if (g_cache.wpath[0] && GetFileAttributesExW(g_cache.wpath, GetFileExInfoStandard, &fad)) {
        g_cache.mtime = fad.ftLastWriteTime;
        g_cache.has_mtime = 1;
    } else {
        g_cache.has_mtime = 0;
    }
}

static void cache_rewrite_file_locked(void) {
    if (!g_cache.wpath[0]) return;
    FILE *f = _wfopen(g_cache.wpath, L"wb");
    if (!f) return;
    fputs("# 快译NG 截屏翻译本地记忆库（UTF-8，Tab 分隔：原文<Tab>译文；开头加 *<Tab> 表示人工校对永久保留）\r\n", f);
    for (OcrCacheEntry *e = g_cache.head; e; e = e->next) {
        if (e->locked) {
            fprintf(f, "*\t%s\t%s\r\n", e->raw_en, e->zh);
        } else {
            fprintf(f, "%s\t%s\r\n", e->raw_en, e->zh);
        }
    }
    fclose(f);
    cache_update_mtime_locked();
}

static void cache_ensure_dir_w(const char *utf8_dir) {
    wchar_t wdir[1024];
    int n = MultiByteToWideChar(CP_UTF8, 0, utf8_dir, -1, wdir, 1024);
    if (n <= 0) return;
    for (wchar_t *p = wdir + 1; *p; p++) {
        if (*p == L'\\' || *p == L'/') {
            wchar_t c = *p;
            *p = 0;
            CreateDirectoryW(wdir, NULL);
            *p = c;
        }
    }
    CreateDirectoryW(wdir, NULL);
}

static void cache_load_locked(const char *cache_dir) {
    if (!cache_dir || !cache_dir[0]) return;

    int dir_changed = (!g_cache.inited || strcmp(g_cache.dir, cache_dir) != 0);
    if (dir_changed) {
        cache_clear_locked();
        snprintf(g_cache.dir, sizeof(g_cache.dir), "%s", cache_dir);
        char full[1024];
        size_t dl = strlen(cache_dir);
        if (dl > 0 && (cache_dir[dl - 1] == '\\' || cache_dir[dl - 1] == '/')) {
            snprintf(full, sizeof(full), "%socr_cache.txt", cache_dir);
        } else {
            snprintf(full, sizeof(full), "%s\\ocr_cache.txt", cache_dir);
        }
        MultiByteToWideChar(CP_UTF8, 0, full, -1, g_cache.wpath, 1024);
        g_cache.inited = 1;
        g_cache.has_mtime = 0;
    }

    WIN32_FILE_ATTRIBUTE_DATA fad;
    if (!GetFileAttributesExW(g_cache.wpath, GetFileExInfoStandard, &fad)) return;
    if (!dir_changed && g_cache.has_mtime &&
        fad.ftLastWriteTime.dwLowDateTime == g_cache.mtime.dwLowDateTime &&
        fad.ftLastWriteTime.dwHighDateTime == g_cache.mtime.dwHighDateTime) {
        return; /* 文件未变化 */
    }

    cache_clear_locked();
    FILE *f = _wfopen(g_cache.wpath, L"rb");
    if (!f) return;
    char line[1024];
    while (fgets(line, sizeof(line), f)) {
        size_t len = strlen(line);
        while (len > 0 && (line[len - 1] == '\n' || line[len - 1] == '\r')) line[--len] = 0;
        if (len == 0 || line[0] == '#') continue;
        /* 跳过 UTF-8 BOM */
        char *p = line;
        if ((unsigned char)p[0] == 0xEF && (unsigned char)p[1] == 0xBB && (unsigned char)p[2] == 0xBF) p += 3;
        if (*p == '\0' || *p == '#') continue;

        int locked = 0;
        if (p[0] == '*' && p[1] == '\t') {
            locked = 1;
            p += 2;
        }
        char *tab = strchr(p, '\t');
        if (!tab || tab == p) continue;
        *tab = '\0';
        char *zh = tab + 1;
        if (!zh[0]) continue;
        cache_insert_mem_locked(p, zh, locked);
    }
    fclose(f);
    g_cache.mtime = fad.ftLastWriteTime;
    g_cache.has_mtime = 1;
}

static char *ocr_cache_lookup(const char *en) {
    if (!g_cache.inited || !en || !en[0]) return NULL;
    char key[128];
    normalize_cache_key(en, key, sizeof(key));
    if (!key[0] || strlen(key) > 48) return NULL;

    ensure_cache_cs();
    EnterCriticalSection(&g_cache_cs);
    OcrCacheEntry *e = cache_find_locked(key);
    char *ret = NULL;
    if (e && e->zh && e->zh[0]) {
        lru_unlink(e);
        lru_push_front(e);
        ret = fy_strdup(e->zh);
    }
    LeaveCriticalSection(&g_cache_cs);
    return ret;
}

static int can_cache_pair(const char *en, const char *zh) {
    if (!en || !zh) return 0;
    size_t el = strlen(en), zl = strlen(zh);
    if (el == 0 || el > 48 || zl == 0 || zl > 120) return 0;
    if (strcmp(en, zh) == 0) return 0;
    int has_alpha = 0;
    for (size_t i = 0; i < el; i++) {
        unsigned char c = (unsigned char)en[i];
        if (c == '\r' || c == '\n' || c == '\t') return 0;
        if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')) has_alpha = 1;
    }
    if (!has_alpha) return 0;
    for (size_t i = 0; i < zl; i++) {
        unsigned char c = (unsigned char)zh[i];
        if (c == '\r' || c == '\n' || c == '\t') return 0;
    }
    return 1;
}

static void ocr_cache_put(const char *en, const char *zh) {
    if (!g_cache.inited || !can_cache_pair(en, zh)) return;
    char key[128];
    normalize_cache_key(en, key, sizeof(key));
    if (!key[0]) return;

    ensure_cache_cs();
    EnterCriticalSection(&g_cache_cs);
    OcrCacheEntry *ex = cache_find_locked(key);
    if (ex) {
        if (!ex->locked && strcmp(ex->zh, zh) != 0) {
            free(ex->zh);
            ex->zh = fy_strdup(zh);
            lru_unlink(ex);
            lru_push_front(ex);
            cache_rewrite_file_locked();
        }
        LeaveCriticalSection(&g_cache_cs);
        return;
    }

    cache_insert_mem_locked(en, zh, 0);
    if (g_cache.auto_count > OCR_CACHE_MAX) {
        /* 从尾部淘汰最久未用的非锁定条目 */
        for (OcrCacheEntry *v = g_cache.tail; v; v = v->prev) {
            if (!v->locked) {
                unsigned int idx = fnv1a_hash(v->key) & (OCR_HASH_BUCKETS - 1);
                OcrCacheEntry **pp = &g_cache.buckets[idx];
                while (*pp && *pp != v) pp = &((*pp)->hnext);
                if (*pp == v) *pp = v->hnext;
                lru_unlink(v);
                free(v->key);
                free(v->raw_en);
                free(v->zh);
                free(v);
                g_cache.auto_count--;
                break;
            }
        }
        cache_rewrite_file_locked();
    } else if (g_cache.wpath[0]) {
        cache_ensure_dir_w(g_cache.dir);
        int is_new = (GetFileAttributesW(g_cache.wpath) == INVALID_FILE_ATTRIBUTES);
        FILE *f = _wfopen(g_cache.wpath, L"ab");
        if (f) {
            if (is_new) {
                fputs("# 快译NG 截屏翻译本地记忆库（UTF-8，Tab 分隔：原文<Tab>译文；开头加 *<Tab> 表示人工校对永久保留）\r\n", f);
            }
            fprintf(f, "%s\t%s\r\n", en, zh);
            fclose(f);
            cache_update_mtime_locked();
        }
    }
    LeaveCriticalSection(&g_cache_cs);
}

/* ---------------- 并发翻译兜底工作线程 ---------------- */
typedef struct {
    TextBlock *blks;
    const int *miss_idx;
    int        miss_n;
    volatile LONG next_pos;
    const char *ai_url;
    const char *ai_key;
    const char *ai_model;
    const char *baidu_appid;
    const char *baidu_secret;
    const char *baidu_url;
} OcrBatchJob;

static DWORD WINAPI ocr_worker_proc(LPVOID param) {
    OcrBatchJob *job = (OcrBatchJob *)param;
    const int flags = FY_CHAIN_NO_DICT | FY_CHAIN_PREFER_BAIDU;
    for (;;) {
        LONG pos = InterlockedIncrement(&job->next_pos) - 1;
        if (pos >= job->miss_n) break;
        int bi = job->miss_idx[pos];
        char *r = fy_translate_chain_ex(job->blks[bi].en, FY_EN2ZH,
                                        job->ai_url, job->ai_key, job->ai_model,
                                        job->baidu_appid, job->baidu_secret, job->baidu_url,
                                        flags);
        if (r) {
            job->blks[bi].zh = clean_zh_for_overlay(r, job->blks[bi].n_lines <= 1);
            fy_free(r);
        } else {
            job->blks[bi].zh = fy_strdup(job->blks[bi].en);
        }
    }
    return 0;
}

/* 翻译所有块：先查本地记忆库，未命中块优先批量请求（百度→必应→AI，跳过词典），未对齐则并发兜底 */
static void translate_blocks(TextBlock *blks, int nb,
                             const char *ai_url, const char *ai_key, const char *ai_model,
                             const char *baidu_appid, const char *baidu_secret, const char *baidu_url,
                             const char *cache_dir) {
    if (nb <= 0) return;

    if (cache_dir && cache_dir[0]) {
        ensure_cache_cs();
        EnterCriticalSection(&g_cache_cs);
        cache_load_locked(cache_dir);
        LeaveCriticalSection(&g_cache_cs);
    }

    int *miss = (int *)malloc((size_t)nb * sizeof(int));
    if (!miss) return;
    int m = 0, active = 0, skipped = 0;
    for (int i = 0; i < nb; i++) {
        if (blks[i].skip_translate) {
            skipped++;
            continue;
        }
        active++;
        char *cached = ocr_cache_lookup(blks[i].en);
        if (cached) {
            blks[i].zh = cached;
        } else {
            miss[m++] = i;
        }
    }
    if (active == 0) {
        fy_log_write("OCR", "识别到 %d 块均非英文（中文/数字/符号），已全部跳过", skipped);
        free(miss);
        return;
    }
    if (m == 0) {
        fy_log_write("OCR", "英文 %d 块全部命中本地记忆库（跳过非英文 %d 块），零网络请求", active, skipped);
        free(miss);
        return;
    }
    if (m < active || skipped > 0) {
        fy_log_write("OCR", "英文 %d 块（记忆库命中 %d，待联网 %d），跳过非英文 %d 块",
                     active, active - m, m, skipped);
    }

    const int flags = FY_CHAIN_NO_DICT | FY_CHAIN_PREFER_BAIDU;

    if (m > 1) {
        size_t total_len = 0;
        for (int k = 0; k < m; k++) total_len += strlen(blks[miss[k]].en) + 1;
        char *joined = (char *)malloc(total_len + 1);
        if (joined) {
            char *p = joined;
            for (int k = 0; k < m; k++) {
                size_t l = strlen(blks[miss[k]].en);
                memcpy(p, blks[miss[k]].en, l);
                p += l;
                *p++ = (k + 1 < m) ? '\n' : '\0';
            }
            char *res = fy_translate_chain_ex(joined, FY_EN2ZH, ai_url, ai_key, ai_model,
                                              baidu_appid, baidu_secret, baidu_url, flags);
            free(joined);
            if (res) {
                char *body = res;
                if (strncmp(body, "\xEF\xBC\x88", 3) == 0) {
                    char *nl = strchr(body, '\n');
                    if (nl && strstr(body, "已用必应翻译") && strstr(body, "已用必应翻译") < nl) {
                        body = nl + 1;
                    }
                }
                /* 去掉尾部多余空行后统计行数 */
                size_t blen = strlen(body);
                while (blen > 0 && (body[blen - 1] == '\n' || body[blen - 1] == '\r')) {
                    body[--blen] = '\0';
                }
                int lines = body[0] ? 1 : 0;
                for (char *q = body; *q; q++) if (*q == '\n') lines++;
                if (lines == m) {
                    char *cur = body;
                    for (int k = 0; k < m; k++) {
                        char *nl = strchr(cur, '\n');
                        if (nl) *nl = '\0';
                        size_t l = strlen(cur);
                        while (l > 0 && cur[l - 1] == '\r') cur[--l] = '\0';
                        blks[miss[k]].zh = fy_strdup(cur);
                        ocr_cache_put(blks[miss[k]].en, blks[miss[k]].zh);
                        cur = nl ? (nl + 1) : (cur + l);
                    }
                    fy_free(res);
                    free(miss);
                    return;
                }
                fy_free(res);
            }
        }
    }

    /* 单块或批量行数未对齐：最多 4 线程并发逐块请求 */
    OcrBatchJob job;
    job.blks = blks;
    job.miss_idx = miss;
    job.miss_n = m;
    job.next_pos = 0;
    job.ai_url = ai_url;
    job.ai_key = ai_key;
    job.ai_model = ai_model;
    job.baidu_appid = baidu_appid;
    job.baidu_secret = baidu_secret;
    job.baidu_url = baidu_url;

    int nthreads = m > 4 ? 4 : m;
    if (nthreads <= 1) {
        ocr_worker_proc(&job);
    } else {
        HANDLE ths[4];
        int created = 0;
        for (int t = 0; t < nthreads; t++) {
            HANDLE h = CreateThread(NULL, 0, ocr_worker_proc, &job, 0, NULL);
            if (h) ths[created++] = h;
        }
        if (created == 0) {
            ocr_worker_proc(&job);
        } else {
            WaitForMultipleObjects((DWORD)created, ths, TRUE, INFINITE);
            for (int t = 0; t < created; t++) CloseHandle(ths[t]);
        }
    }

    for (int k = 0; k < m; k++) {
        int bi = miss[k];
        if (blks[bi].zh) ocr_cache_put(blks[bi].en, blks[bi].zh);
    }
    free(miss);
}

/* 采样矩形外围 1-2px 主背景色（4-bit 量化众数桶求均值，抗抗锯齿毛边） */
static COLORREF sample_bg_color(const BYTE *bgra, int w, int h, int bx, int by, int bw, int bh) {
    int counts[4096];
    unsigned long sum_r[4096], sum_g[4096], sum_b[4096];
    memset(counts, 0, sizeof(counts));
    memset(sum_r, 0, sizeof(sum_r));
    memset(sum_g, 0, sizeof(sum_g));
    memset(sum_b, 0, sizeof(sum_b));

    int x0 = bx - 2; if (x0 < 0) x0 = 0;
    int y0 = by - 2; if (y0 < 0) y0 = 0;
    int x1 = bx + bw + 1; if (x1 >= w) x1 = w - 1;
    int y1 = by + bh + 1; if (y1 >= h) y1 = h - 1;

    int best_idx = 0, best_cnt = 0;
    for (int y = y0; y <= y1; y++) {
        for (int x = x0; x <= x1; x++) {
            /* 只采边框 2px 环形区域 */
            if (x > bx + 1 && x < bx + bw - 2 && y > by + 1 && y < by + bh - 2) continue;
            const BYTE *px = bgra + ((size_t)y * (size_t)w + (size_t)x) * 4;
            int b = px[0], g = px[1], r = px[2];
            int idx = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
            counts[idx]++;
            sum_r[idx] += (unsigned long)r;
            sum_g[idx] += (unsigned long)g;
            sum_b[idx] += (unsigned long)b;
            if (counts[idx] > best_cnt) {
                best_cnt = counts[idx];
                best_idx = idx;
            }
        }
    }
    if (best_cnt == 0) return RGB(255, 255, 255);
    return RGB((BYTE)(sum_r[best_idx] / best_cnt),
               (BYTE)(sum_g[best_idx] / best_cnt),
               (BYTE)(sum_b[best_idx] / best_cnt));
}

/* 选择与背景高对比度的文字色（优先采样原文字主色） */
static COLORREF sample_fg_color(const BYTE *bgra, int w, int h, int bx, int by, int bw, int bh, COLORREF bg) {
    int bg_r = GetRValue(bg), bg_g = GetGValue(bg), bg_b = GetBValue(bg);
    int bg_y = (299 * bg_r + 587 * bg_g + 114 * bg_b) / 1000;

    int x0 = bx < 0 ? 0 : bx;
    int y0 = by < 0 ? 0 : by;
    int x1 = bx + bw; if (x1 > w) x1 = w;
    int y1 = by + bh; if (y1 > h) y1 = h;

    unsigned long sr = 0, sg = 0, sb = 0, cnt = 0;
    for (int y = y0; y < y1; y++) {
        for (int x = x0; x < x1; x++) {
            const BYTE *px = bgra + ((size_t)y * (size_t)w + (size_t)x) * 4;
            int b = px[0], g = px[1], r = px[2];
            int diff = abs(r - bg_r) + abs(g - bg_g) + abs(b - bg_b);
            if (diff > 110) {
                sr += (unsigned long)r; sg += (unsigned long)g; sb += (unsigned long)b;
                cnt++;
            }
        }
    }
    if (cnt >= 8) {
        int fr = (int)(sr / cnt), fg = (int)(sg / cnt), fb = (int)(sb / cnt);
        int fy = (299 * fr + 587 * fg + 114 * fb) / 1000;
        if (abs(fy - bg_y) >= 75) return RGB(fr, fg, fb);
    }
    return (bg_y > 135) ? RGB(22, 22, 22) : RGB(245, 245, 245);
}

/* ---- 译文精准折行与字号自适应（按字形实高排版，避免 DrawText 留白压小字号） ---- */
#define OCR_MAX_WRAP_LINES 64

typedef struct {
    const WCHAR *ptr;
    int len;
    int width;
} WrapLine;

static int is_latin_token_w(WCHAR c) {
    return (c >= L'A' && c <= L'Z') || (c >= L'a' && c <= L'z') ||
           (c >= L'0' && c <= L'9') || c == L'_' || c == L'.' || c == L'-';
}

/* 按像素宽度精确折行：汉字任意断行，英文/数字词尽量不从中间劈开 */
static int wrap_wide_text(HDC hdc, const WCHAR *wz, int max_w, int allow_wrap,
                          WrapLine *out_lines, int max_lines, int *out_max_w) {
    *out_max_w = 0;
    if (!wz || !wz[0] || max_w <= 0 || max_lines <= 0) return 0;

    if (!allow_wrap) {
        int len = (int)wcslen(wz);
        SIZE sz = {0, 0};
        GetTextExtentPoint32W(hdc, wz, len, &sz);
        out_lines[0].ptr = wz;
        out_lines[0].len = len;
        out_lines[0].width = sz.cx;
        *out_max_w = sz.cx;
        return 1;
    }

    int nl = 0;
    const WCHAR *p = wz;
    while (*p) {
        if (*p == L'\r') { p++; continue; }
        if (*p == L'\n') { p++; continue; }
        while (*p == L' ' || *p == L'\t') p++;
        if (!*p || *p == L'\r' || *p == L'\n') continue;

        if (nl >= max_lines) return max_lines + 1;

        const WCHAR *line_start = p;
        int line_len = 0;
        int line_w = 0;

        while (*p && *p != L'\r' && *p != L'\n') {
            /* 取下一个 token：连续空白、连续英文数字串、或单个汉字/标点 */
            const WCHAR *tok_start = p;
            int tok_len = 1;
            if (*p == L' ' || *p == L'\t') {
                while (p[tok_len] == L' ' || p[tok_len] == L'\t') tok_len++;
            } else if (is_latin_token_w(*p)) {
                while (is_latin_token_w(p[tok_len])) tok_len++;
            }

            int cand_len = (int)((tok_start + tok_len) - line_start);
            SIZE sz = {0, 0};
            GetTextExtentPoint32W(hdc, line_start, cand_len, &sz);
            if (sz.cx <= max_w) {
                line_len = cand_len;
                line_w = sz.cx;
                p = tok_start + tok_len;
            } else if (line_len > 0) {
                /* 本行已有内容，当前 token 留到下一行 */
                break;
            } else {
                /* 单个长 token 超出整行宽度：按字符截断，至少放 1 个字符 */
                int fit = 1;
                SIZE csz = {0, 0};
                GetTextExtentPoint32W(hdc, line_start, 1, &csz);
                line_w = csz.cx;
                for (int k = 2; k <= tok_len; k++) {
                    GetTextExtentPoint32W(hdc, line_start, k, &csz);
                    if (csz.cx > max_w) break;
                    fit = k;
                    line_w = csz.cx;
                }
                line_len = fit;
                p = line_start + fit;
                break;
            }
        }

        /* 去掉行尾空白 */
        while (line_len > 0 && (line_start[line_len - 1] == L' ' || line_start[line_len - 1] == L'\t')) {
            line_len--;
        }
        if (line_len > 0) {
            SIZE sz = {0, 0};
            GetTextExtentPoint32W(hdc, line_start, line_len, &sz);
            line_w = sz.cx;
            out_lines[nl].ptr = line_start;
            out_lines[nl].len = line_len;
            out_lines[nl].width = line_w;
            if (line_w > *out_max_w) *out_max_w = line_w;
            nl++;
        }
    }
    return nl;
}

static HFONT create_overlay_font(int fh, int n_width) {
    return CreateFontW(-fh, n_width, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                       0, 0, CLEARTYPE_QUALITY, 0, L"Microsoft YaHei UI");
}

/* 计算字号 fh 在指定行距下的视觉总高度（汉字实高约 0.90*fh，多行步进 1.15*fh） */
static int calc_visual_total_h(int fh, int n_lines, int *out_vis_h, int *out_step) {
    int vis_h = (int)ceilf((float)fh * 0.90f);
    if (vis_h < 5) vis_h = 5;
    int step = (int)ceilf((float)fh * 1.15f);
    if (step < vis_h + 1) step = vis_h + 1;
    if (out_vis_h) *out_vis_h = vis_h;
    if (out_step) *out_step = step;
    return (n_lines <= 1) ? vis_h : ((n_lines - 1) * step + vis_h);
}

/* 判断给定字号 fh 是否能完整放入 avail_w x avail_h */
static int test_font_fit(HDC hdc, const WCHAR *wz, int avail_w, int avail_h,
                         int fh, int allow_wrap) {
    HFONT f = create_overlay_font(fh, 0);
    if (!f) return 0;
    HFONT old_f = (HFONT)SelectObject(hdc, f);
    WrapLine tmp[OCR_MAX_WRAP_LINES];
    int max_lw = 0;
    int nl = wrap_wide_text(hdc, wz, avail_w, allow_wrap, tmp, OCR_MAX_WRAP_LINES, &max_lw);
    SelectObject(hdc, old_f);
    DeleteObject(f);
    if (nl <= 0 || nl > OCR_MAX_WRAP_LINES) return 0;
    if (max_lw > avail_w) return 0;
    int total_h = calc_visual_total_h(fh, nl, NULL, NULL);
    return (total_h <= avail_h);
}

/* 在 [min_fh, max_fh] 内二分查找能完整放入的最大字号，找不到返回 0 */
static int find_best_fh(HDC hdc, const WCHAR *wz, int avail_w, int avail_h,
                        int min_fh, int max_fh, int allow_wrap) {
    if (max_fh < min_fh) return 0;
    int lo = min_fh, hi = max_fh, best = 0;
    while (lo <= hi) {
        int mid = lo + (hi - lo) / 2;
        if (test_font_fit(hdc, wz, avail_w, avail_h, mid, allow_wrap)) {
            best = mid;
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    return best;
}

/* 计算块 i 上下可安全借用的空白高度（不压到任何相邻文本块，含跳过的中文块） */
static void compute_safe_vpad(const TextBlock *blks, int nb, int idx, int img_h,
                              int orig_h, int *out_top, int *out_bot) {
    const TextBlock *b = &blks[idx];
    int gap_top = b->y;
    int gap_bot = img_h - (b->y + b->h);
    if (gap_top < 0) gap_top = 0;
    if (gap_bot < 0) gap_bot = 0;

    for (int j = 0; j < nb; j++) {
        if (j == idx) continue;
        const TextBlock *o = &blks[j];
        /* 仅考虑水平方向有重叠的相邻块 */
        if (o->x + o->w <= b->x - 2 || o->x >= b->x + b->w + 2) continue;
        if (o->y + o->h <= b->y) {
            int g = b->y - (o->y + o->h);
            if (g < gap_top) gap_top = g;
        } else if (o->y >= b->y + b->h) {
            int g = o->y - (b->y + b->h);
            if (g < gap_bot) gap_bot = g;
        }
    }

    int pad_cap = (int)ceilf((float)orig_h * 0.28f);
    if (pad_cap < 2) pad_cap = 2;
    int pt = (gap_top > 2) ? ((gap_top - 1) / 2) : (gap_top > 0 ? 1 : 0);
    int pb = (gap_bot > 2) ? ((gap_bot - 1) / 2) : (gap_bot > 0 ? 1 : 0);
    if (pt > pad_cap) pt = pad_cap;
    if (pb > pad_cap) pb = pad_cap;
    *out_top = pt;
    *out_bot = pb;
}

static void render_blocks(const void *bgra_in, void *bgra_out, int w, int h,
                          const TextBlock *blks, int nb) {
    memcpy(bgra_out, bgra_in, (size_t)w * (size_t)h * 4);
    if (nb <= 0) return;

    HDC hdc_scr = GetDC(NULL);
    HDC hdc = CreateCompatibleDC(hdc_scr);
    BITMAPINFO bmi = {0};
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = w;
    bmi.bmiHeader.biHeight = -h;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    void *bits = NULL;
    HBITMAP hbm = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &bits, NULL, 0);
    if (!hbm || !bits) {
        if (hbm) DeleteObject(hbm);
        DeleteDC(hdc); ReleaseDC(NULL, hdc_scr);
        return;
    }
    memcpy(bits, bgra_in, (size_t)w * (size_t)h * 4);
    HBITMAP old_bm = (HBITMAP)SelectObject(hdc, hbm);
    SetBkMode(hdc, TRANSPARENT);

    for (int i = 0; i < nb; i++) {
        const TextBlock *b = &blks[i];
        /* 中文/非英文块原图保留，不擦除不重画 */
        if (b->skip_translate || !b->zh || !b->zh[0]) continue;

        int wlen = MultiByteToWideChar(CP_UTF8, 0, b->zh, -1, NULL, 0);
        if (wlen <= 1) continue;
        WCHAR *wz = (WCHAR *)malloc((size_t)wlen * sizeof(WCHAR));
        if (!wz) continue;
        MultiByteToWideChar(CP_UTF8, 0, b->zh, -1, wz, wlen);

        int orig_h = (b->n_lines > 1 ? b->line_h : b->h);
        if (orig_h < 8) orig_h = 8;

        int pad_top = 0, pad_bot = 0;
        compute_safe_vpad(blks, nb, i, h, orig_h, &pad_top, &pad_bot);

        /* 宽度严格限制在原英文长度内，竖直方向可在安全空隙内适度扩展 */
        RECT safe_rc;
        safe_rc.left   = b->x;             if (safe_rc.left < 0) safe_rc.left = 0;
        safe_rc.right  = b->x + b->w + 2;  if (safe_rc.right > w) safe_rc.right = w;
        safe_rc.top    = b->y - pad_top;   if (safe_rc.top < 0) safe_rc.top = 0;
        safe_rc.bottom = b->y + b->h + pad_bot; if (safe_rc.bottom > h) safe_rc.bottom = h;

        int avail_w = safe_rc.right - safe_rc.left;
        int avail_h = safe_rc.bottom - safe_rc.top;
        if (avail_w <= 2 || avail_h <= 2) {
            free(wz);
            continue;
        }

        /* 视觉等大基准字号 ≈ orig_h * 1.05，允许有空余时最多放大到 1.15 倍 */
        int base_fh = (int)floorf((float)orig_h * 1.05f + 0.5f);
        int max_fh  = (int)floorf((float)orig_h * 1.05f * 1.15f + 0.5f);
        if (max_fh < 10) max_fh = 10;
        if (max_fh > 72) max_fh = 72;
        int min_fh = (orig_h < 10) ? 7 : 8;
        if (min_fh > max_fh) min_fh = max_fh;

        int best_fh = 0;
        int use_wrap = (b->n_lines > 1);

        if (!use_wrap) {
            int fh_single = find_best_fh(hdc, wz, avail_w, avail_h, min_fh, max_fh, 0);
            int fh_wrap = 0;
            /* 单行字号被迫压到明显小于原文时，尝试折行看能否换来更大字号 */
            if (fh_single < (int)((float)base_fh * 0.85f) && avail_h >= min_fh * 2) {
                fh_wrap = find_best_fh(hdc, wz, avail_w, avail_h, min_fh, max_fh, 1);
            }
            if (fh_wrap > fh_single) {
                best_fh = fh_wrap;
                use_wrap = 1;
            } else {
                best_fh = fh_single;
                use_wrap = 0;
            }
        } else {
            best_fh = find_best_fh(hdc, wz, avail_w, avail_h, min_fh, max_fh, 1);
        }

        int n_width = 0;
        int need_ellipsis = 0;
        if (best_fh == 0) {
            /* 兜底：min_fh 仍放不下，压窄字宽（最低 0.70 倍）并在末行加省略号，严格不超宽 */
            best_fh = min_fh;
            HFONT ftmp = create_overlay_font(best_fh, 0);
            if (ftmp) {
                HFONT old_f = (HFONT)SelectObject(hdc, ftmp);
                SIZE sz = {0, 0};
                GetTextExtentPoint32W(hdc, wz, wlen - 1, &sz);
                TEXTMETRICW tm;
                if (sz.cx > avail_w && GetTextMetricsW(hdc, &tm) && tm.tmAveCharWidth > 0) {
                    float ratio = (float)avail_w / (float)sz.cx;
                    if (ratio < 0.70f) ratio = 0.70f;
                    int nw = (int)((float)tm.tmAveCharWidth * ratio);
                    if (nw >= 2 && nw < tm.tmAveCharWidth) n_width = nw;
                }
                SelectObject(hdc, old_f);
                DeleteObject(ftmp);
            }
            need_ellipsis = 1;
        }

        HFONT hf = create_overlay_font(best_fh, n_width);
        if (!hf) {
            free(wz);
            continue;
        }

        HFONT old_f = (HFONT)SelectObject(hdc, hf);
        WrapLine lines[OCR_MAX_WRAP_LINES];
        int max_lw = 0;
        int nl = wrap_wide_text(hdc, wz, avail_w, use_wrap, lines, OCR_MAX_WRAP_LINES, &max_lw);
        if (nl > OCR_MAX_WRAP_LINES) nl = OCR_MAX_WRAP_LINES;

        int vis_h = 0, line_step = 0;
        int total_h = calc_visual_total_h(best_fh, nl > 0 ? nl : 1, &vis_h, &line_step);
        /* 若兜底模式下行数仍超出 avail_h，截断到可见行数 */
        if (nl > 1 && total_h > avail_h) {
            int max_fit_lines = 1 + (avail_h > vis_h ? (avail_h - vis_h) / line_step : 0);
            if (max_fit_lines < 1) max_fit_lines = 1;
            if (nl > max_fit_lines) {
                nl = max_fit_lines;
                need_ellipsis = 1;
                total_h = calc_visual_total_h(best_fh, nl, &vis_h, &line_step);
            }
        }

        COLORREF bg = sample_bg_color((const BYTE *)bgra_in, w, h, b->x, b->y, b->w, b->h);
        COLORREF fg = sample_fg_color((const BYTE *)bgra_in, w, h, b->x, b->y, b->w, b->h, bg);

        /* 背景擦除框：完整盖住原英文，若放大或折行后文字更高则在 safe_rc 内同步扩展（留 2px 呼吸边不切字头） */
        RECT fill_rc;
        fill_rc.left   = b->x - 2; if (fill_rc.left < 0) fill_rc.left = 0;
        fill_rc.right  = b->x + b->w + 2; if (fill_rc.right > w) fill_rc.right = w;
        fill_rc.top    = b->y - 1; if (fill_rc.top < 0) fill_rc.top = 0;
        fill_rc.bottom = b->y + b->h + 1; if (fill_rc.bottom > h) fill_rc.bottom = h;
        int fill_h = fill_rc.bottom - fill_rc.top;
        int need_h = total_h + 4;
        if (need_h > fill_h) {
            int center_y = (fill_rc.top + fill_rc.bottom) / 2;
            int nt = center_y - need_h / 2;
            int nb_y = nt + need_h;
            if (nt < safe_rc.top) { nt = safe_rc.top; nb_y = nt + need_h; }
            if (nb_y > safe_rc.bottom) { nb_y = safe_rc.bottom; nt = nb_y - need_h; }
            if (nt < safe_rc.top) nt = safe_rc.top;
            if (nt < fill_rc.top) fill_rc.top = nt;
            if (nb_y > fill_rc.bottom) fill_rc.bottom = nb_y;
            fill_h = fill_rc.bottom - fill_rc.top;
        }

        HBRUSH br = CreateSolidBrush(bg);
        FillRect(hdc, &fill_rc, br);
        DeleteObject(br);

        SetTextColor(hdc, fg);
        TEXTMETRICW tm;
        int ink_offset = 0;
        if (GetTextMetricsW(hdc, &tm)) {
            /* 去掉约 2/3 的 tmInternalLeading，既让汉字墨迹居中又保留顶部防裁切余量 */
            ink_offset = (tm.tmInternalLeading * 2) / 3;
            if (ink_offset < 0) ink_offset = 0;
        }

        int ink_top = fill_rc.top + (fill_h - total_h) / 2;
        RECT clip_rc = { safe_rc.left, fill_rc.top, safe_rc.right, fill_rc.bottom };

        for (int r = 0; r < nl; r++) {
            int draw_y = ink_top + r * line_step - ink_offset;
            int is_last = (r == nl - 1);
            if (is_last && (need_ellipsis || lines[r].width > avail_w)) {
                /* 末行超宽时用省略号截断到 avail_w 以内 */
                int ell_len = lines[r].len;
                WCHAR *ebuf = (WCHAR *)malloc((size_t)(ell_len + 2) * sizeof(WCHAR));
                if (ebuf) {
                    int k = ell_len;
                    while (k > 0) {
                        memcpy(ebuf, lines[r].ptr, (size_t)k * sizeof(WCHAR));
                        ebuf[k] = 0x2026; /* … */
                        ebuf[k + 1] = 0;
                        SIZE esz = {0, 0};
                        GetTextExtentPoint32W(hdc, ebuf, k + 1, &esz);
                        if (esz.cx <= avail_w) break;
                        k--;
                    }
                    if (k > 0) {
                        ExtTextOutW(hdc, safe_rc.left, draw_y, ETO_CLIPPED, &clip_rc, ebuf, k + 1, NULL);
                    } else {
                        ExtTextOutW(hdc, safe_rc.left, draw_y, ETO_CLIPPED, &clip_rc, lines[r].ptr, lines[r].len, NULL);
                    }
                    free(ebuf);
                }
            } else {
                ExtTextOutW(hdc, safe_rc.left, draw_y, ETO_CLIPPED, &clip_rc, lines[r].ptr, lines[r].len, NULL);
            }
        }

        SelectObject(hdc, old_f);
        DeleteObject(hf);
        free(wz);
    }

    GdiFlush();
    memcpy(bgra_out, bits, (size_t)w * (size_t)h * 4);
    SelectObject(hdc, old_bm);
    DeleteObject(hbm);
    DeleteDC(hdc);
    ReleaseDC(NULL, hdc_scr);
}

static char *join_block_texts(const TextBlock *blks, int nb, int is_zh) {
    if (nb <= 0) return NULL;
    size_t cap = 0;
    for (int i = 0; i < nb; i++) {
        if (blks[i].skip_translate) continue;
        const char *s = is_zh ? blks[i].zh : blks[i].en;
        cap += (s ? strlen(s) : 0) + 2;
    }
    char *out = (char *)malloc(cap + 1);
    if (!out) return NULL;
    char *p = out;
    int first = 1;
    for (int i = 0; i < nb; i++) {
        if (blks[i].skip_translate) continue;
        const char *s = is_zh ? blks[i].zh : blks[i].en;
        if (s && s[0]) {
            if (!first) *p++ = '\n';
            first = 0;
            size_t l = strlen(s);
            memcpy(p, s, l);
            p += l;
        }
    }
    *p = '\0';
    return out;
}

FySnipResult *fy_snip_translate(const void *bgra_in, int width, int height,
                                const char *ai_url, const char *ai_key,
                                const char *ai_model, const char *baidu_appid,
                                const char *baidu_secret, const char *baidu_url,
                                const char *cache_dir) {
    FySnipResult *res = (FySnipResult *)calloc(1, sizeof(FySnipResult));
    if (!res) return NULL;
    res->width = width;
    res->height = height;
    if (!bgra_in || width <= 0 || height <= 0) {
        res->err = fy_strdup("无效图像尺寸");
        return res;
    }

    size_t img_bytes = (size_t)width * (size_t)height * 4;
    res->bgra_out = malloc(img_bytes);
    if (!res->bgra_out) {
        res->err = fy_strdup("内存不足");
        return res;
    }
    memcpy(res->bgra_out, bgra_in, img_bytes);

    OcrLineBox *lines = NULL;
    int n_lines = 0;
    char *ocr_err = NULL;
    if (run_winrt_ocr(bgra_in, width, height, &lines, &n_lines, &ocr_err) != 0) {
        res->err = ocr_err ? ocr_err : fy_strdup("OCR 识别失败");
        return res;
    }
    if (n_lines == 0) {
        res->err = fy_strdup("未识别到文本");
        return res;
    }

    int nb = 0;
    TextBlock *blks = group_lines(lines, n_lines, &nb);
    for (int i = 0; i < n_lines; i++) free(lines[i].text);
    free(lines);

    int need_cnt = 0;
    for (int i = 0; i < nb; i++) {
        if (!blks[i].skip_translate) need_cnt++;
    }
    if (need_cnt == 0) {
        for (int i = 0; i < nb; i++) free(blks[i].en);
        free(blks);
        res->err = fy_strdup("未检测到需要翻译的英文文本");
        return res;
    }

    translate_blocks(blks, nb, ai_url, ai_key, ai_model, baidu_appid, baidu_secret, baidu_url, cache_dir);
    render_blocks(bgra_in, res->bgra_out, width, height, blks, nb);

    res->ok = 1;
    res->en_text = join_block_texts(blks, nb, 0);
    res->zh_text = join_block_texts(blks, nb, 1);

    for (int i = 0; i < nb; i++) {
        free(blks[i].en);
        free(blks[i].zh);
    }
    free(blks);
    return res;
}

void fy_snip_result_free(FySnipResult *r) {
    if (!r) return;
    free(r->bgra_out);
    free(r->zh_text);
    free(r->en_text);
    free(r->err);
    free(r);
}
