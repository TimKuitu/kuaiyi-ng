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
} OcrLineBox;

typedef struct {
    int x, y, w, h;
    int line_h;
    int n_lines;
    char *en;
    char *zh;
} TextBlock;

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

/* 调用 Windows.Media.Ocr 识别英文行及外接矩形 */
static int run_winrt_ocr(const void *bgra, int w, int h,
                         OcrLineBox **out_lines, int *out_count, char **out_err) {
    *out_lines = NULL;
    *out_count = 0;

    HMODULE combase = LoadLibraryA("combase.dll");
    if (!combase) {
        *out_err = fy_strdup("当前系统不支持 Windows OCR（缺少 combase.dll）");
        return -1;
    }
    PFN_RoInitialize pRoInit = (PFN_RoInitialize)GetProcAddress(combase, "RoInitialize");
    PFN_RoUninitialize pRoUninit = (PFN_RoUninitialize)GetProcAddress(combase, "RoUninitialize");
    PFN_RoGetActivationFactory pRoFactory = (PFN_RoGetActivationFactory)GetProcAddress(combase, "RoGetActivationFactory");
    PFN_WindowsCreateStringReference pWinStrRef = (PFN_WindowsCreateStringReference)GetProcAddress(combase, "WindowsCreateStringReference");
    PFN_WindowsGetStringRawBuffer pWinStrBuf = (PFN_WindowsGetStringRawBuffer)GetProcAddress(combase, "WindowsGetStringRawBuffer");
    PFN_WindowsDeleteString pWinStrDel = (PFN_WindowsDeleteString)GetProcAddress(combase, "WindowsDeleteString");
    if (!pRoInit || !pRoFactory || !pWinStrRef || !pWinStrBuf || !pWinStrDel) {
        *out_err = fy_strdup("当前系统版本过低，不支持 Windows OCR（需 Win10/11）");
        return -1;
    }

    HRESULT hr_init = pRoInit(1); /* RO_INIT_MULTITHREADED */
    int need_uninit = (hr_init == S_OK || hr_init == S_FALSE);

    void *langFactory = NULL, *lang = NULL, *ocrStatics = NULL, *ocrEngine = NULL;
    void *bufFactory = NULL, *buf = NULL, *byteAccess = NULL, *sbStatics = NULL;
    void *sb = NULL, *asyncOp = NULL, *asyncInfo = NULL, *ocrRes = NULL, *lines = NULL;
    void *scaled_buf = NULL;

    FY_HSTRING_HEADER hh;
    FY_HSTRING hs = NULL;

    /* 1. 创建 OcrEngine（优先 en-US，缺失时回退到用户语言） */
    pWinStrRef(L"Windows.Media.Ocr.OcrEngine", 27, &hh, &hs);
    if (FAILED(pRoFactory(hs, &FY_IID_IOcrEngineStatics, &ocrStatics)) || !ocrStatics) {
        *out_err = fy_strdup("无法初始化 Windows OCR 引擎");
        goto cleanup;
    }

    pWinStrRef(L"Windows.Globalization.Language", 28, &hh, &hs);
    if (SUCCEEDED(pRoFactory(hs, &FY_IID_ILanguageFactory, &langFactory)) && langFactory) {
        pWinStrRef(L"en-US", 5, &hh, &hs);
        if (SUCCEEDED((*(FY_ILanguageFactoryVtbl **)langFactory)->CreateLanguage(langFactory, hs, &lang)) && lang) {
            (*(FY_IOcrEngineStaticsVtbl **)ocrStatics)->TryCreateFromLanguage(ocrStatics, lang, &ocrEngine);
        }
    }
    if (!ocrEngine) {
        (*(FY_IOcrEngineStaticsVtbl **)ocrStatics)->TryCreateFromUserProfileLanguages(ocrStatics, &ocrEngine);
    }
    if (!ocrEngine) {
        *out_err = fy_strdup("系统未安装英文 OCR 语言包，请在 Windows 设置中安装英语识别");
        goto cleanup;
    }

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
    pWinStrRef(L"Windows.Storage.Streams.Buffer", 30, &hh, &hs);
    if (FAILED(pRoFactory(hs, &FY_IID_IBufferFactory, &bufFactory)) || !bufFactory) {
        *out_err = fy_strdup("无法创建 OCR 图像缓冲");
        goto cleanup;
    }
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

    pWinStrRef(L"Windows.Graphics.Imaging.SoftwareBitmap", 39, &hh, &hs);
    if (FAILED(pRoFactory(hs, &FY_IID_ISoftwareBitmapStatics, &sbStatics)) || !sbStatics) {
        *out_err = fy_strdup("无法初始化 SoftwareBitmap");
        goto cleanup;
    }
    if (FAILED((*(FY_ISoftwareBitmapStaticsVtbl **)sbStatics)->CreateCopyFromBuffer(sbStatics, buf, 87, ocr_w, ocr_h, &sb)) || !sb) {
        *out_err = fy_strdup("构造 OCR 图像失败");
        goto cleanup;
    }

    /* 4. 异步识别并轮询完成 */
    if (FAILED((*(FY_IOcrEngineVtbl **)ocrEngine)->RecognizeAsync(ocrEngine, sb, &asyncOp)) || !asyncOp) {
        *out_err = fy_strdup("OCR 识别启动失败");
        goto cleanup;
    }
    if (FAILED((*(FY_IInspectableVtbl **)asyncOp)->QueryInterface(asyncOp, &FY_IID_IAsyncInfo, &asyncInfo)) || !asyncInfo) {
        *out_err = fy_strdup("OCR 状态查询失败");
        goto cleanup;
    }
    int status = 0;
    for (int tries = 0; tries < 2000; tries++) {
        (*(FY_IAsyncInfoVtbl **)asyncInfo)->get_Status(asyncInfo, &status);
        if (status != 0) break;
        Sleep(5);
    }
    if (status != 1) {
        *out_err = fy_strdup("OCR 识别超时或失败");
        goto cleanup;
    }
    if (FAILED((*(FY_IAsyncOperationVtbl **)asyncOp)->GetResults(asyncOp, &ocrRes)) || !ocrRes) {
        *out_err = fy_strdup("获取 OCR 结果失败");
        goto cleanup;
    }

    /* 5. 提取每行文本及外接矩形 */
    if (FAILED((*(FY_IOcrResultVtbl **)ocrRes)->get_Lines(ocrRes, &lines)) || !lines) {
        *out_err = fy_strdup("读取 OCR 行列表失败");
        goto cleanup;
    }
    UINT32 nLines = 0;
    (*(FY_IVectorViewVtbl **)lines)->get_Size(lines, &nLines);
    if (nLines == 0) {
        goto cleanup_ok;
    }

    OcrLineBox *arr = (OcrLineBox *)calloc(nLines, sizeof(OcrLineBox));
    int valid = 0;
    for (UINT32 i = 0; i < nLines; i++) {
        void *line = NULL;
        if (FAILED((*(FY_IVectorViewVtbl **)lines)->GetAt(lines, i, &line)) || !line) continue;

        FY_HSTRING lt = NULL;
        (*(FY_IOcrLineVtbl **)line)->get_Text(line, &lt);
        UINT32 wlen = 0;
        PCWSTR ws = lt ? pWinStrBuf(lt, &wlen) : NULL;
        char *utf8 = wide_to_utf8(ws, (int)wlen);
        if (lt) pWinStrDel(lt);

        void *words = NULL;
        float min_x = 1e9f, min_y = 1e9f, max_x = -1e9f, max_y = -1e9f;
        if (SUCCEEDED((*(FY_IOcrLineVtbl **)line)->get_Words(line, &words)) && words) {
            UINT32 nWords = 0;
            (*(FY_IVectorViewVtbl **)words)->get_Size(words, &nWords);
            for (UINT32 j = 0; j < nWords; j++) {
                void *word = NULL;
                if (SUCCEEDED((*(FY_IVectorViewVtbl **)words)->GetAt(words, j, &word)) && word) {
                    FY_WinRect wr;
                    if (SUCCEEDED((*(FY_IOcrWordVtbl **)word)->get_BoundingRect(word, &wr))) {
                        if (wr.X < min_x) min_x = wr.X;
                        if (wr.Y < min_y) min_y = wr.Y;
                        if (wr.X + wr.Width > max_x) max_x = wr.X + wr.Width;
                        if (wr.Y + wr.Height > max_y) max_y = wr.Y + wr.Height;
                    }
                    SAFE_REL(word);
                }
            }
            SAFE_REL(words);
        }
        SAFE_REL(line);

        if (utf8 && utf8[0] && max_x > min_x && max_y > min_y) {
            arr[valid].x = min_x * inv_scale;
            arr[valid].y = min_y * inv_scale;
            arr[valid].w = (max_x - min_x) * inv_scale;
            arr[valid].h = (max_y - min_y) * inv_scale;
            arr[valid].text = utf8;
            valid++;
        } else {
            free(utf8);
        }
    }
    if (valid > 1) qsort(arr, (size_t)valid, sizeof(OcrLineBox), cmp_line_box);
    *out_lines = arr;
    *out_count = valid;

cleanup_ok:
    SAFE_REL(lines);
    SAFE_REL(ocrRes);
    SAFE_REL(asyncInfo);
    SAFE_REL(asyncOp);
    SAFE_REL(sb);
    SAFE_REL(sbStatics);
    SAFE_REL(byteAccess);
    SAFE_REL(buf);
    SAFE_REL(bufFactory);
    SAFE_REL(ocrEngine);
    SAFE_REL(ocrStatics);
    SAFE_REL(lang);
    SAFE_REL(langFactory);
    free(scaled_buf);
    if (need_uninit && pRoUninit) pRoUninit();
    return 0;

cleanup:
    SAFE_REL(lines);
    SAFE_REL(ocrRes);
    SAFE_REL(asyncInfo);
    SAFE_REL(asyncOp);
    SAFE_REL(sb);
    SAFE_REL(sbStatics);
    SAFE_REL(byteAccess);
    SAFE_REL(buf);
    SAFE_REL(bufFactory);
    SAFE_REL(ocrEngine);
    SAFE_REL(ocrStatics);
    SAFE_REL(lang);
    SAFE_REL(langFactory);
    free(scaled_buf);
    if (need_uninit && pRoUninit) pRoUninit();
    return -1;
}

/* 判断两行是否属于同一连续段落（左对齐接近、行高相近、行距紧凑） */
static int should_merge_lines(const TextBlock *blk, const OcrLineBox *ln) {
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
        } else {
            TextBlock *b = &blks[nb++];
            b->x = (int)floorf(lines[i].x);
            b->y = (int)floorf(lines[i].y);
            b->w = (int)ceilf(lines[i].w);
            b->h = (int)ceilf(lines[i].h);
            b->line_h = b->h > 0 ? b->h : 14;
            b->n_lines = 1;
            b->en = fy_strdup(lines[i].text);
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

/* 批量翻译所有块；若返回行数与块数一致则一次映射，否则逐块翻译 */
static void translate_blocks(TextBlock *blks, int nb,
                             const char *ai_url, const char *ai_key, const char *ai_model,
                             const char *baidu_appid, const char *baidu_secret, const char *baidu_url) {
    if (nb <= 0) return;

    if (nb > 1) {
        size_t total_len = 0;
        for (int i = 0; i < nb; i++) total_len += strlen(blks[i].en) + 1;
        char *joined = (char *)malloc(total_len + 1);
        if (joined) {
            char *p = joined;
            for (int i = 0; i < nb; i++) {
                size_t l = strlen(blks[i].en);
                memcpy(p, blks[i].en, l);
                p += l;
                *p++ = (i + 1 < nb) ? '\n' : '\0';
            }
            char *res = fy_translate_chain(joined, FY_EN2ZH, ai_url, ai_key, ai_model,
                                           baidu_appid, baidu_secret, baidu_url);
            free(joined);
            if (res) {
                char *body = res;
                if (strncmp(body, "\xEF\xBC\x88", 3) == 0) {
                    char *nl = strchr(body, '\n');
                    if (nl && strstr(body, "已用必应翻译") && strstr(body, "已用必应翻译") < nl) {
                        body = nl + 1;
                    }
                }
                int lines = 1;
                for (char *q = body; *q; q++) if (*q == '\n') lines++;
                if (lines == nb) {
                    char *cur = body;
                    for (int i = 0; i < nb; i++) {
                        char *nl = strchr(cur, '\n');
                        if (nl) *nl = '\0';
                        size_t l = strlen(cur);
                        if (l > 0 && cur[l - 1] == '\r') cur[l - 1] = '\0';
                        blks[i].zh = fy_strdup(cur);
                        cur = nl ? (nl + 1) : (cur + l);
                    }
                    fy_free(res);
                    return;
                }
                fy_free(res);
            }
        }
    }

    /* 单块或行数未对齐：逐块翻译并清理词典附加行 */
    for (int i = 0; i < nb; i++) {
        char *r = fy_translate_chain(blks[i].en, FY_EN2ZH, ai_url, ai_key, ai_model,
                                     baidu_appid, baidu_secret, baidu_url);
        if (r) {
            blks[i].zh = clean_zh_for_overlay(r, blks[i].n_lines <= 1);
            fy_free(r);
        } else {
            blks[i].zh = fy_strdup(blks[i].en);
        }
    }
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
        if (!b->zh || !b->zh[0]) continue;

        COLORREF bg = sample_bg_color((const BYTE *)bgra_in, w, h, b->x, b->y, b->w, b->h);
        COLORREF fg = sample_fg_color((const BYTE *)bgra_in, w, h, b->x, b->y, b->w, b->h, bg);

        RECT fill_rc;
        fill_rc.left   = b->x - 2; if (fill_rc.left < 0) fill_rc.left = 0;
        fill_rc.top    = b->y - 1; if (fill_rc.top < 0) fill_rc.top = 0;
        fill_rc.right  = b->x + b->w + 2; if (fill_rc.right > w) fill_rc.right = w;
        fill_rc.bottom = b->y + b->h + 1; if (fill_rc.bottom > h) fill_rc.bottom = h;
        HBRUSH br = CreateSolidBrush(bg);
        FillRect(hdc, &fill_rc, br);
        DeleteObject(br);

        int wlen = MultiByteToWideChar(CP_UTF8, 0, b->zh, -1, NULL, 0);
        if (wlen <= 1) continue;
        WCHAR *wz = (WCHAR *)malloc((size_t)wlen * sizeof(WCHAR));
        if (!wz) continue;
        MultiByteToWideChar(CP_UTF8, 0, b->zh, -1, wz, wlen);

        SetTextColor(hdc, fg);
        RECT draw_rc = fill_rc;
        int avail_w = draw_rc.right - draw_rc.left;
        int avail_h = draw_rc.bottom - draw_rc.top;
        int start_fh = (int)((b->n_lines > 1 ? b->line_h : b->h) * 0.92f);
        if (start_fh < 11) start_fh = 11;
        if (start_fh > 64) start_fh = 64;

        HFONT hf = NULL;
        int use_wrap = (b->n_lines > 1);
        if (!use_wrap) {
            for (int fh = start_fh; fh >= 11; fh--) {
                HFONT f = CreateFontW(-fh, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                                      0, 0, CLEARTYPE_QUALITY, 0, L"Microsoft YaHei UI");
                HFONT old_f = (HFONT)SelectObject(hdc, f);
                RECT calc = {0, 0, avail_w, avail_h};
                DrawTextW(hdc, wz, -1, &calc, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
                SelectObject(hdc, old_f);
                if (calc.right <= avail_w + 2 || fh == 11) {
                    hf = f;
                    if (calc.right > avail_w + 4) use_wrap = 1;
                    break;
                }
                DeleteObject(f);
            }
        }
        if (use_wrap) {
            if (hf) { DeleteObject(hf); hf = NULL; }
            for (int fh = start_fh; fh >= 10; fh--) {
                HFONT f = CreateFontW(-fh, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                                      0, 0, CLEARTYPE_QUALITY, 0, L"Microsoft YaHei UI");
                HFONT old_f = (HFONT)SelectObject(hdc, f);
                RECT calc = {0, 0, avail_w, 0};
                DrawTextW(hdc, wz, -1, &calc, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX);
                SelectObject(hdc, old_f);
                if (calc.bottom <= avail_h + 4 || fh == 10) {
                    hf = f;
                    break;
                }
                DeleteObject(f);
            }
        }
        if (hf) {
            HFONT old_f = (HFONT)SelectObject(hdc, hf);
            UINT fmt = use_wrap ? (DT_LEFT | DT_WORDBREAK | DT_NOPREFIX)
                                : (DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            DrawTextW(hdc, wz, -1, &draw_rc, fmt);
            SelectObject(hdc, old_f);
            DeleteObject(hf);
        }
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
        const char *s = is_zh ? blks[i].zh : blks[i].en;
        cap += (s ? strlen(s) : 0) + 2;
    }
    char *out = (char *)malloc(cap + 1);
    if (!out) return NULL;
    char *p = out;
    for (int i = 0; i < nb; i++) {
        const char *s = is_zh ? blks[i].zh : blks[i].en;
        if (s && s[0]) {
            size_t l = strlen(s);
            memcpy(p, s, l);
            p += l;
            if (i + 1 < nb) *p++ = '\n';
        }
    }
    *p = '\0';
    return out;
}

FySnipResult *fy_snip_translate(const void *bgra_in, int width, int height,
                                const char *ai_url, const char *ai_key,
                                const char *ai_model, const char *baidu_appid,
                                const char *baidu_secret, const char *baidu_url) {
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
        res->err = fy_strdup("未识别到英文文本");
        return res;
    }

    int nb = 0;
    TextBlock *blks = group_lines(lines, n_lines, &nb);
    for (int i = 0; i < n_lines; i++) free(lines[i].text);
    free(lines);

    translate_blocks(blks, nb, ai_url, ai_key, ai_model, baidu_appid, baidu_secret, baidu_url);
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
