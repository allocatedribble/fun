#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct FunDlssContext FunDlssContext;

typedef enum FunDlssMode {
    FUN_DLSS_MODE_QUALITY = 0,
    FUN_DLSS_MODE_BALANCED = 1,
    FUN_DLSS_MODE_PERFORMANCE = 2,
    FUN_DLSS_MODE_ULTRA_PERFORMANCE = 3
} FunDlssMode;

typedef enum FunDlssResult {
    FUN_DLSS_RESULT_OK = 0,
    FUN_DLSS_RESULT_UNSUPPORTED = 1,
    FUN_DLSS_RESULT_INVALID_ARGUMENT = 2,
    FUN_DLSS_RESULT_INVALID_DEVICE = 3,
    FUN_DLSS_RESULT_INVALID_COMMAND_LIST = 4,
    FUN_DLSS_RESULT_INVALID_RESOURCE = 5,
    FUN_DLSS_RESULT_INVALID_DIMENSIONS = 6,
    FUN_DLSS_RESULT_SDK_INIT_FAILED = 7,
    FUN_DLSS_RESULT_SDK_EVALUATE_FAILED = 8,
    FUN_DLSS_RESULT_MISSING_DLL = 9,
    FUN_DLSS_RESULT_DRIVER_UNSUPPORTED = 10
} FunDlssResult;

typedef struct FunDlssSupport {
    int sr_supported;
    int rr_supported;
    int needs_updated_driver;
    int reserved;
} FunDlssSupport;

typedef struct FunDlssCreateDesc {
    const char* app_id;
    const char* app_name;
    const char* sdk_path;
    void* d3d12_device;
    void* d3d12_queue;
    int enable_debug;
} FunDlssCreateDesc;

typedef struct FunDlssResizeDesc {
    unsigned input_width;
    unsigned input_height;
    unsigned output_width;
    unsigned output_height;
    FunDlssMode mode;
    int hdr;
} FunDlssResizeDesc;

typedef struct FunDlssEvaluateDesc {
    void* d3d12_cmdlist;

    void* input_color;
    void* output_color;
    void* depth;
    void* motion_vectors;
    void* exposure;

    unsigned input_width;
    unsigned input_height;
    unsigned output_width;
    unsigned output_height;

    float jitter_x;
    float jitter_y;
    float sharpness;

    float camera_near;
    float camera_far;

    int reset_history;
    int hdr;
    int motion_vectors_are_low_res;
    int inverted_depth;
    int reserved;
} FunDlssEvaluateDesc;

FunDlssContext* fun_dlss_create(const FunDlssCreateDesc* desc);
void fun_dlss_destroy(FunDlssContext* ctx);

int fun_dlss_query_support(FunDlssContext* ctx, FunDlssSupport* out_support);
int fun_dlss_resize(FunDlssContext* ctx, const FunDlssResizeDesc* desc);
int fun_dlss_evaluate(FunDlssContext* ctx, const FunDlssEvaluateDesc* desc);

const char* fun_dlss_last_error(FunDlssContext* ctx);

#ifdef __cplusplus
}
#endif
