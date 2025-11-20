#include "bindings.h"

#include "include/gpu/graphite/Context.h"
#include "include/gpu/graphite/Recorder.h"
#include "include/gpu/graphite/Recording.h"
#include "include/gpu/graphite/Surface.h"
#include "include/gpu/graphite/BackendTexture.h"
#include "include/gpu/graphite/TextureInfo.h"
#include "include/gpu/graphite/ContextOptions.h"

#ifdef SK_METAL
#include "include/gpu/graphite/mtl/MtlBackendContext.h"
#include "include/gpu/graphite/mtl/MtlGraphiteTypes.h"
#include "include/gpu/graphite/mtl/MtlGraphiteUtils.h"
#endif

#ifdef SK_VULKAN
#include "include/gpu/graphite/vk/VulkanGraphiteContext.h"
#include "include/gpu/graphite/vk/VulkanGraphiteTypes.h"
#include "include/gpu/graphite/vk/VulkanGraphiteUtils.h"
#endif

extern "C" void C_TextureInfo_Construct(skgpu::graphite::TextureInfo* ti) {
    new (ti) skgpu::graphite::TextureInfo();
}

extern "C" void C_TextureInfo_Destruct(skgpu::graphite::TextureInfo* ti) {
    ti->~TextureInfo();
}

extern "C" void C_BackendTexture_Construct(skgpu::graphite::BackendTexture* bt) {
    new (bt) skgpu::graphite::BackendTexture();
}

extern "C" void C_BackendTexture_Destruct(skgpu::graphite::BackendTexture* bt) {
    bt->~BackendTexture();
}

extern "C" void C_ContextOptions_Construct(skgpu::graphite::ContextOptions* co) {
    new (co) skgpu::graphite::ContextOptions();
}

extern "C" void C_ContextOptions_Destruct(skgpu::graphite::ContextOptions* co) {
    co->~ContextOptions();
}

extern "C" void C_Context_Destruct(skgpu::graphite::Context* self) {
    delete self;
}

extern "C" void C_Recorder_Destruct(skgpu::graphite::Recorder* self) {
    delete self;
}

extern "C" void C_Recording_Destruct(skgpu::graphite::Recording* self) {
    delete self;
}

#ifdef SK_METAL
extern "C" void C_MtlBackendContext_Construct(
    skgpu::graphite::MtlBackendContext* context,
    const void* device,
    const void* queue) {
    new (context) skgpu::graphite::MtlBackendContext();
    context->fDevice.reset(CFRetain((CFTypeRef)device));
    context->fQueue.reset(CFRetain((CFTypeRef)queue));
}

extern "C" void C_MtlBackendContext_Destruct(skgpu::graphite::MtlBackendContext* context) {
    context->~MtlBackendContext();
}

extern "C" skgpu::graphite::Context* C_Context_MakeMetal(
    const skgpu::graphite::MtlBackendContext* backendContext,
    const skgpu::graphite::ContextOptions* options) {
    return skgpu::graphite::ContextFactory::MakeMetal(*backendContext, *options).release();
}
#endif

#ifdef SK_VULKAN
#include "include/gpu/vk/VulkanTypes.h"
#include "include/gpu/graphite/vk/VulkanGraphiteTypes.h"

extern "C" skgpu::graphite::Context* C_Context_MakeVulkan(
    const skgpu::VulkanBackendContext* backendContext,
    const skgpu::graphite::ContextOptions* options) {
    return skgpu::graphite::ContextFactory::MakeVulkan(*backendContext, *options).release();
}

extern "C" skgpu::graphite::VulkanTextureInfo* C_VulkanTextureInfo_Make(
    uint32_t sampleCount,
    skgpu::Mipmapped mipmapped,
    VkImageCreateFlags flags,
    VkFormat format,
    VkImageTiling imageTiling,
    VkImageUsageFlags imageUsageFlags,
    VkSharingMode sharingMode,
    VkImageAspectFlags aspectMask,
    const skgpu::VulkanYcbcrConversionInfo* ycbcrConversionInfo) {
    return new skgpu::graphite::VulkanTextureInfo(
        sampleCount, mipmapped, flags, format, imageTiling, imageUsageFlags,
        sharingMode, aspectMask, *ycbcrConversionInfo);
}

extern "C" void C_VulkanTextureInfo_Destruct(skgpu::graphite::VulkanTextureInfo* self) {
    delete self;
}

extern "C" void C_BackendTexture_MakeVulkan(
    skgpu::graphite::BackendTexture* self,
    const SkISize* dimensions,
    const skgpu::graphite::VulkanTextureInfo* info,
    VkImageLayout layout,
    uint32_t queueFamilyIndex,
    VkImage image,
    const skgpu::VulkanAlloc* alloc) {
    *self = skgpu::graphite::BackendTextures::MakeVulkan(
        *dimensions, *info, layout, queueFamilyIndex, image, *alloc);
}
#endif

extern "C" SkSurface* C_Surface_Make(
    skgpu::graphite::Recorder* recorder,
    const SkImageInfo* info,
    skgpu::Mipmapped mipmapped,
    const SkSurfaceProps* props) {
    return SkSurfaces::RenderTarget(
        recorder, *info, mipmapped, props).release();
}

extern "C" bool C_TextureInfo_isValid(const skgpu::graphite::TextureInfo* self) {
    return self->isValid();
}

extern "C" bool C_BackendTexture_isValid(const skgpu::graphite::BackendTexture* self) {
    return self->isValid();
}

extern "C" void C_BackendTexture_info(const skgpu::graphite::BackendTexture* self, skgpu::graphite::TextureInfo* result) {
    new (result) skgpu::graphite::TextureInfo(self->info());
}

extern "C" SkISize C_BackendTexture_dimensions(const skgpu::graphite::BackendTexture* self) {
    return self->dimensions();
}

extern "C" void C_RecorderOptions_Construct(skgpu::graphite::RecorderOptions* ro) {
    new (ro) skgpu::graphite::RecorderOptions();
}

extern "C" void C_RecorderOptions_Destruct(skgpu::graphite::RecorderOptions* ro) {
    ro->~RecorderOptions();
}

extern "C" skgpu::graphite::Recorder* C_Context_makeRecorder(
    skgpu::graphite::Context* self,
    const skgpu::graphite::RecorderOptions* options) {
    if (options) {
        return self->makeRecorder(*options).release();
    } else {
        return self->makeRecorder().release();
    }
}

extern "C" skgpu::graphite::Recording* C_Recorder_snap(skgpu::graphite::Recorder* self) {
    return self->snap().release();
}

extern "C" bool C_Context_insertRecording(skgpu::graphite::Context* self, skgpu::graphite::Recording* recording) {
    skgpu::graphite::InsertRecordingInfo info;
    info.fRecording = recording;
    return self->insertRecording(info);
}

extern "C" void C_Context_submit(skgpu::graphite::Context* self, skgpu::graphite::SyncToCpu syncToCpu) {
    self->submit(syncToCpu);
}

#include "include/gpu/graphite/BackendTexture.h"
#include "include/gpu/graphite/Recording.h"
#include "include/core/SkSurface.h"
#include "include/core/SkColorSpace.h"

#ifdef SK_METAL
#include "include/gpu/graphite/mtl/MtlGraphiteTypes.h"
#include "include/gpu/graphite/mtl/MtlGraphiteTypes_cpp.h"

extern "C" void C_TextureInfo_MakeMetal(skgpu::graphite::TextureInfo* self, const void* texture) {
    *self = skgpu::graphite::TextureInfos::MakeMetal(texture);
}

extern "C" void C_BackendTexture_MakeMetal(skgpu::graphite::BackendTexture* self, const SkISize* dimensions, const void* texture) {
    *self = skgpu::graphite::BackendTextures::MakeMetal(*dimensions, texture);
}
#endif

#include "include/gpu/graphite/Surface.h"

extern "C" SkSurface* C_Surface_MakeGraphiteWrapped(
    skgpu::graphite::Recorder* recorder,
    const skgpu::graphite::BackendTexture* backendTexture,
    SkColorType colorType,
    const SkColorSpace* colorSpace,
    const SkSurfaceProps* surfaceProps) {
    return SkSurfaces::WrapBackendTexture(
        recorder, *backendTexture, colorType, sk_ref_sp(colorSpace), surfaceProps).release();
}

