// Models Skia's Dawn backend calling through an inline C++ wrapper (webgpu_cpp.h).
#include <cstdint>
extern "C" {
typedef struct WGPUDeviceImpl* WGPUDevice;
typedef struct WGPUBufferImpl* WGPUBuffer;
typedef struct { uint64_t size; } WGPUBufferDescriptor;
WGPUBuffer wgpuDeviceCreateBuffer(WGPUDevice device, const WGPUBufferDescriptor* descriptor);
uint64_t wgpuBufferGetSize(WGPUBuffer buffer);
void wgpuBufferRelease(WGPUBuffer buffer);
}
namespace wgpu {
struct Buffer {
    WGPUBuffer h;
    ~Buffer() { wgpuBufferRelease(h); }
    uint64_t GetSize() const { return wgpuBufferGetSize(h); }
};
struct Device {
    WGPUDevice h;
    Buffer CreateBuffer(uint64_t size) const {
        WGPUBufferDescriptor d{size};
        return Buffer{wgpuDeviceCreateBuffer(h, &d)};
    }
};
}
extern "C" uint64_t C_Backend_Run(WGPUDevice device) {
    wgpu::Device d{device};
    return d.CreateBuffer(42).GetSize();
}
