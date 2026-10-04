#include <metal_stdlib>
using namespace metal;

// Scalar arrays match Rust's layout without Metal float3 alignment padding.
struct EntranceUniforms {
    uint width, height, diagonal;
    float threshold, edge;
    float bounds[4];
    float inverse[3][3];
    float center[2];
    float opacity;
};

uchar4 sample_card(device const uchar4 *source, constant EntranceUniforms &u, float2 position) {
    int2 origin = int2(floor(position));
    float2 t = position - float2(origin);
    float alpha = 0.0;
    float3 rgb = 0.0;
    for (int dy = 0; dy < 2; dy++) {
        for (int dx = 0; dx < 2; dx++) {
            int2 cell = origin + int2(dx, dy);
            if (cell.x < 0 || cell.y < 0 || cell.x >= int(u.width) || cell.y >= int(u.height)) continue;
            float weight = (dx == 0 ? 1.0 - t.x : t.x) * (dy == 0 ? 1.0 - t.y : t.y);
            float4 pixel = float4(source[cell.y * u.width + cell.x]);
            float a = pixel.a * weight;
            alpha += a;
            rgb += pixel.rgb * a;
        }
    }
    if (alpha < 0.5) return uchar4(0);
    return uchar4(uchar3(round(rgb / alpha)), uchar(round(alpha)));
}

kernel void image_entrance(device uchar4 *output [[buffer(0)]],
                           device const uchar4 *source [[buffer(1)]],
                           constant EntranceUniforms &u [[buffer(2)]],
                           uint2 id [[thread_position_in_grid]]) {
    if (id.x >= u.width || id.y >= u.height) return;
    uint index = id.y * u.width + id.x;
    if (u.diagonal) {
        uchar4 pixel = source[index];
        float local = (float(id.x) + 0.5 - u.bounds[0]) / u.bounds[2]
                    + (float(id.y) + 0.5 - u.bounds[1]) / u.bounds[3];
        pixel.a = uchar(round(float(pixel.a) * clamp((u.threshold - local) / u.edge + 0.5, 0.0, 1.0)));
        output[index] = pixel;
        return;
    }
    float2 xy = float2(id) + 0.5 - float2(u.center[0], u.center[1]);
    float w = u.inverse[2][0] * xy.x + u.inverse[2][1] * xy.y + 1.0;
    uchar4 pixel = uchar4(0);
    if (w > 0.01) {
        float2 position = float2(u.inverse[0][0] * xy.x + u.inverse[0][1] * xy.y,
                                 u.inverse[1][0] * xy.x + u.inverse[1][1] * xy.y) / w
                        + float2(u.center[0], u.center[1]) - 0.5;
        pixel = sample_card(source, u, position);
        pixel.a = uchar(round(float(pixel.a) * u.opacity));
    }
    output[index] = pixel;
}
