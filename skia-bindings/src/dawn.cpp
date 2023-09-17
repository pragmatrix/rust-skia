#if !defined(SK_DAWN)
    #define SK_DAWN
#endif

#include "include/gpu/GrDirectContext.h"
#include "webgpu/webgpu_cpp.h"

// HACK
static wgpu::Device *device = nullptr;

extern "C" GrDirectContext* C_GrDirectContext_MakeDawn(
    const GrContextOptions* options) {
    if (!device) {
        device = new wgpu::Device();
    }
    if (options) {
        return GrDirectContext::MakeDawn(*device, *options).release();
    }
    return GrDirectContext::MakeDawn(*device).release();
}
