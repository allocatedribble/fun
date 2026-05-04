#include "fun_dlss_bridge.h"

#include <memory>
#include <string>

struct FunDlssContext {
    std::string last_error;
    bool valid_device = false;
    bool runtime_present = false;
};

namespace {

void set_support_unsupported(FunDlssSupport* out_support) {
    if (out_support == nullptr) {
        return;
    }
    out_support->sr_supported = 0;
    out_support->rr_supported = 0;
    out_support->needs_updated_driver = 0;
    out_support->reserved = 0;
}

bool valid_resize_desc(const FunDlssResizeDesc* desc) {
    return desc != nullptr && desc->input_width > 0 && desc->input_height > 0 &&
           desc->output_width > 0 && desc->output_height > 0 &&
           desc->input_width <= desc->output_width &&
           desc->input_height <= desc->output_height;
}

bool valid_evaluate_desc(const FunDlssEvaluateDesc* desc) {
    return desc != nullptr && desc->d3d12_cmdlist != nullptr &&
           desc->input_color != nullptr && desc->output_color != nullptr &&
           desc->depth != nullptr && desc->motion_vectors != nullptr &&
           desc->input_width > 0 && desc->input_height > 0 &&
           desc->output_width > 0 && desc->output_height > 0;
}

} // namespace

FunDlssContext* fun_dlss_create(const FunDlssCreateDesc* desc) {
    auto ctx = std::make_unique<FunDlssContext>();
    if (desc == nullptr) {
        ctx->last_error = "create descriptor is null";
        return ctx.release();
    }
    ctx->valid_device = desc->d3d12_device != nullptr && desc->d3d12_queue != nullptr;
    ctx->runtime_present = desc->sdk_path != nullptr && desc->sdk_path[0] != '\0';
    if (!ctx->valid_device) {
        ctx->last_error = "D3D12 device or queue is null";
    } else if (!ctx->runtime_present) {
        ctx->last_error = "NVIDIA DLSS runtime DLL was not discovered";
    } else {
        ctx->last_error =
            "NVIDIA Streamline/NGX SDK integration is not linked in this bridge";
    }
    return ctx.release();
}

void fun_dlss_destroy(FunDlssContext* ctx) {
    delete ctx;
}

int fun_dlss_query_support(FunDlssContext* ctx, FunDlssSupport* out_support) {
    set_support_unsupported(out_support);
    if (ctx == nullptr || out_support == nullptr) {
        return FUN_DLSS_RESULT_INVALID_ARGUMENT;
    }
    if (!ctx->valid_device) {
        return FUN_DLSS_RESULT_INVALID_DEVICE;
    }
    if (!ctx->runtime_present) {
        return FUN_DLSS_RESULT_MISSING_DLL;
    }
    return FUN_DLSS_RESULT_UNSUPPORTED;
}

int fun_dlss_resize(FunDlssContext* ctx, const FunDlssResizeDesc* desc) {
    if (ctx == nullptr || desc == nullptr) {
        return FUN_DLSS_RESULT_INVALID_ARGUMENT;
    }
    if (!ctx->valid_device) {
        return FUN_DLSS_RESULT_INVALID_DEVICE;
    }
    if (!valid_resize_desc(desc)) {
        ctx->last_error = "invalid DLSS resize dimensions";
        return FUN_DLSS_RESULT_INVALID_DIMENSIONS;
    }
    ctx->last_error =
        "DLSS resize requested before Streamline/NGX integration is linked";
    return FUN_DLSS_RESULT_UNSUPPORTED;
}

int fun_dlss_evaluate(FunDlssContext* ctx, const FunDlssEvaluateDesc* desc) {
    if (ctx == nullptr || desc == nullptr) {
        return FUN_DLSS_RESULT_INVALID_ARGUMENT;
    }
    if (!ctx->valid_device) {
        return FUN_DLSS_RESULT_INVALID_DEVICE;
    }
    if (!valid_evaluate_desc(desc)) {
        ctx->last_error = "invalid DLSS evaluate descriptor";
        return FUN_DLSS_RESULT_INVALID_RESOURCE;
    }
    ctx->last_error =
        "DLSS evaluate requested before Streamline/NGX integration is linked";
    return FUN_DLSS_RESULT_UNSUPPORTED;
}

const char* fun_dlss_last_error(FunDlssContext* ctx) {
    if (ctx == nullptr) {
        return "DLSS context is null";
    }
    return ctx->last_error.c_str();
}
