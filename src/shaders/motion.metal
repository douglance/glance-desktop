#include <metal_stdlib>
using namespace metal;

// Scalars/arrays keep the ABI identical to Rust (float3 would introduce padding).
struct LiquidUniforms {
    uint width;
    uint height;
    float orbit_sin;
    float orbit_cos;
    float dark[3];
    float blue[3];
    float cyan[3];
    float mint[3];
    uint effect;
};

uint grain_hash(uint x, uint y) {
    uint n = x * 1973u + y * 9277u + 89173u;
    n = (n ^ (n >> 16u)) * 2246822519u;
    n = (n ^ (n >> 13u)) * 3266489917u;
    return n ^ (n >> 16u);
}

float random_value(uint x, uint y) {
    return float(grain_hash(x, y) & 65535u) / 65535.0;
}

float paper_noise(float2 p) {
    int2 cell = int2(floor(p));
    float2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(random_value(uint(cell.x), uint(cell.y)),
                   random_value(uint(cell.x + 1), uint(cell.y)), f.x),
               mix(random_value(uint(cell.x), uint(cell.y + 1)),
                   random_value(uint(cell.x + 1), uint(cell.y + 1)), f.x), f.y);
}

float3 lava(float2 p, constant LiquidUniforms &u, float3 dark, float3 blue, float3 cyan, float3 mint) {
    float field = 0.0;
    float2 gradient = 0.0;
    float aspect = float(u.width) / float(min(u.width, u.height));
    for (uint i = 0; i < 6; i++) {
        float angle = float(i) * 1.8;
        float s = u.orbit_sin * cos(angle) + u.orbit_cos * sin(angle);
        float c = u.orbit_cos * cos(angle) - u.orbit_sin * sin(angle);
        float2 center = float2((float(i) / 5.0 - 0.5) * aspect * 0.80 + 0.09 * s, 0.36 * c);
        float2 delta = (p - center) * float2(1.0, 0.85);
        float r = 0.16 + 0.030 * sin(angle * 1.3);
        float d = dot(delta, delta) + 0.006;
        field += r * r / d;
        gradient += -2.0 * r * r * delta * float2(1.0, 0.85) / (d * d);
    }
    float fill = smoothstep(0.85, 1.1, field);
    float core = smoothstep(1.0, 2.1, field);
    float shine = 0.4 + 0.6 * clamp(dot(gradient, float2(-0.5, -0.8)) / max(length(gradient), 0.001) + 0.3, 0.0, 1.0);
    float rim = exp(-(field - 1.15) * (field - 1.15) * 18.0) * shine;
    return mix(dark + blue * min(field, 0.8) * 0.07, mix(mix(blue, cyan, core), mint, rim * 0.85), fill);
}

float3 aurora(float2 p, constant LiquidUniforms &u, float3 dark, float3 blue, float3 cyan, float3 mint) {
    float3 color = dark;
    for (uint i = 0; i < 2; i++) {
        float f = float(i);
        float edge = -0.18 + f * 0.32 + 0.13 * sin(p.x * 3.8 + f * 1.8 + 0.6 * u.orbit_sin)
                   + 0.045 * cos(p.x * 9.0 - f + u.orbit_cos);
        float d = p.y - edge;
        float fibers = 0.7 + 0.3 * pow(sin(p.x * 160.0 + sin(p.x * 11.0 + u.orbit_sin)), 2.0);
        float curtain = exp(-max(d, 0.0) * 8.0) * smoothstep(-0.025, 0.03, d) * fibers;
        float glow = 0.4 * exp(-d * d * 65.0) + 0.65 * exp(-d * d * 1250.0) + curtain * 0.4;
        float t = 0.5 + 0.5 * sin(p.x * 2.0 + f + u.orbit_sin);
        color += mix(mix(blue, cyan, t), mint, 0.20) * glow;
    }
    return color;
}

float3 contours(float2 p, constant LiquidUniforms &u, float3 dark, float3 blue, float3 cyan, float3 mint) {
    float2 q = p * 4.0;
    float t = atan2(u.orbit_sin, u.orbit_cos);
    // Locally deforming waves, with integer frequencies for a seamless loop.
    float warp = q.x * 0.7 - q.y * 0.9 - 2.0 * t;
    float bend = q.x * 0.65 + q.y * 0.45 - t;
    float a = q.x + 0.85 * sin(q.y * 0.8 + t) + 0.30 * cos(warp);
    float b = q.y * 0.85 - q.x * 0.25 + 0.65 * cos(bend);
    float c = q.x * 1.6 + q.y * 1.2 + t;
    float weight = 0.30 + 0.06 * u.orbit_sin;
    float field = 0.52 * sin(a) + weight * cos(b) + 0.12 * sin(c);
    // Travel through three major levels. The atan2 branch jump is exactly
    // three major / nine minor levels, leaving the isolines continuous.
    float level = field * 9.0 + 3.0 * t / 6.28318530718;
    float d = abs(level - round(level));
    float minor = abs(level * 3.0 - round(level * 3.0));
    // Analytic field derivatives supply pixel coverage in a compute shader,
    // where fragment fwidth() is unavailable. This keeps steep lines continuous.
    float dx = 0.52 * cos(a) * (1.0 - 0.21 * sin(warp))
             - weight * sin(b) * (-0.25 - 0.4225 * sin(bend)) + 0.192 * cos(c);
    float dy = 0.52 * cos(a) * (0.68 * cos(q.y * 0.8 + t) + 0.27 * sin(warp))
             - weight * sin(b) * (0.85 - 0.2925 * sin(bend)) + 0.144 * cos(c);
    float width = clamp(36.0 * (abs(dx) + abs(dy)) / float(min(u.width, u.height)), 0.004, 0.14);
    float line = 1.0 - smoothstep(width * 0.45, width * 1.35, d);
    float fine = 1.0 - smoothstep(width * 0.60, width * 2.40, minor);
    float3 base = mix(dark, blue, 0.15 + 0.15 * field);
    base = mix(base, cyan, fine * 0.16);
    return mix(base, mix(cyan, mint, 0.35), line * 0.85);
}

float2 prism_vertex(float x, float y, float t) {
    return float2(x + 0.20 * sin(t + x * 1.7 + y * 0.9),
                  y + 0.20 * cos(2.0 * t + x * 0.8 - y * 1.4));
}

float3 prism(float2 p, constant LiquidUniforms &u, float3 dark, float3 blue, float3 cyan, float3 mint) {
    float t = atan2(u.orbit_sin, u.orbit_cos);
    float2 grid = float2(p.x * 0.8 + p.y * 0.6, -p.x * 0.6 + p.y * 0.8) * 3.0;
    int2 cell = int2(floor(grid));
    // Each neighbor uses the same moving vertices: straight edges, no gaps.
    for (int y = cell.y - 1; y <= cell.y + 1; y++) {
        for (int x = cell.x - 1; x <= cell.x + 1; x++) {
            float2 va = prism_vertex(float(x), float(y), t);
            float2 vb = prism_vertex(float(x) + 1.0, float(y), t);
            float2 vc = prism_vertex(float(x), float(y) + 1.0, t);
            float2 vd = prism_vertex(float(x) + 1.0, float(y) + 1.0, t);
            for (uint side = 0; side < 2; side++) {
                float2 a = side == 0u ? va : vd;
                float2 b = side == 0u ? vb : vc;
                float2 c = side == 0u ? vc : vb;
                float2 e = b - a, f = c - a, v = grid - a;
                float det = e.x * f.y - e.y * f.x;
                float s = (v.x * f.y - v.y * f.x) / det;
                float r = (e.x * v.y - e.y * v.x) / det;
                float3 weights = float3(1.0 - s - r, s, r);
                if (any(weights < 0.0)) continue;
                uint hash = grain_hash(uint(x), uint(y)) ^ (side == 0u ? 0x5bd1e995u : 0u);
                float phase = float(hash & 65535u) / 65535.0 * 6.28318530718;
                float3 face = hash % 3u == 0u ? blue : (hash % 3u == 1u ? cyan : mint);
                float light = 0.55 + 0.30 * sin(t + phase) + 0.15 * s;
                float glint = pow(0.5 + 0.5 * cos(2.0 * t - phase + s * 1.4 - r), 12.0);
                float3 color = mix(mix(dark, face, light), mint, glint * 0.45);
                float3 distances = weights * abs(det) / float3(length(e - f), length(f), length(e));
                float edge = min(min(distances.x, distances.y), distances.z);
                float seam = 1.0 - smoothstep(0.005, 0.005 + 2.0 / float(min(u.width, u.height)), edge);
                // Match adjacent faces exactly at their shared edge.
                return mix(color, mint, seam);
            }
        }
    }
    return dark;
}

float3 painterly(float2 p, constant LiquidUniforms &u, float3 blue, float3 cyan, float3 mint) {
    float3 color = mix(mint, float3(1.0, 0.97, 0.90), 0.65);
    for (uint i = 0; i < 14; i++) {
        float phase = random_value(i, 29) * 6.28318530718;
        float s = u.orbit_sin * cos(phase) + u.orbit_cos * sin(phase);
        float c = u.orbit_cos * cos(phase) - u.orbit_sin * sin(phase);
        float progress = 0.5 + 0.5 * s;
        float a = random_value(i, 17) * 3.14159265 + 0.22 * c;
        float2 center = float2((random_value(i, 21) - 0.5) * 1.7, (random_value(i, 22) - 0.5) * 1.3)
                      + float2(c, s) * 0.06;
        float2 delta = p - center;
        float2 local = float2(delta.x * cos(a) + delta.y * sin(a), -delta.x * sin(a) + delta.y * cos(a));
        float along = local.x;
        local.y -= 0.022 * sin(along * 8.0 + s * 1.8);
        float full_len = 0.18 + random_value(i, 23) * 0.25;
        float len = full_len * (0.18 + 0.82 * progress);
        local.x += full_len - len;
        float width = (0.045 + random_value(i, 24) * 0.08) * (0.85 + 0.15 * c);
        float rough = paper_noise(float2(along, local.y) * 60.0 + float(i)) * 0.18;
        float coverage = (1.0 - smoothstep(0.7, 1.0, abs(local.x) / len + rough))
                       * (1.0 - smoothstep(0.7, 1.0, abs(local.y) / width + rough));
        float fibers = 0.85 + 0.15 * sin(local.y * 260.0 + float(i));
        float3 brush = i % 3u == 0u ? blue : (i % 3u == 1u ? cyan : mint);
        color = mix(color, brush, coverage * fibers * (0.15 + 0.70 * progress));
    }
    return color;
}

kernel void motion_backdrop(
    device uchar4 *output [[buffer(0)]],
    constant LiquidUniforms &u [[buffer(1)]],
    uint2 id [[thread_position_in_grid]]
) {
    if (id.x >= u.width || id.y >= u.height) return;
    float unit = float(min(u.width, u.height));
    float2 p = (float2(id) + 0.5 - float2(u.width, u.height) * 0.5) / unit;

    // Orbit the domain rather than advancing time linearly: every term loops.
    float2 q = p * 3.4 + float2(0.34 * u.orbit_cos, 0.28 * u.orbit_sin);
    float2 warp = float2(
        sin(q.y * 1.65 + 0.65 * u.orbit_sin) + 0.45 * cos(q.x * 1.3 - q.y),
        sin(q.x * 1.45 - 0.55 * u.orbit_cos) + 0.40 * cos(q.y * 1.4 + q.x)
    );
    q += warp * 0.85;
    float field = sin(q.x * 1.75 + sin(q.y * 1.35))
                + 0.60 * cos(q.y * 1.90 - q.x * 0.65)
                + 0.25 * sin(q.x * 0.80 + q.y * 1.20);
    float band = 0.5 + 0.5 * sin(field * 3.8 + q.y * 0.55);

    float3 dark = float3(u.dark[0], u.dark[1], u.dark[2]);
    float3 blue = float3(u.blue[0], u.blue[1], u.blue[2]);
    float3 cyan = float3(u.cyan[0], u.cyan[1], u.cyan[2]);
    float3 mint = float3(u.mint[0], u.mint[1], u.mint[2]);
    float3 color = mix(dark, blue, smoothstep(0.08, 0.56, band));
    color = mix(color, cyan, smoothstep(0.48, 0.82, band));
    color = mix(color, mint, smoothstep(0.78, 0.98, band));
    switch (u.effect) {
        case 1: color = lava(p, u, dark, blue, cyan, mint); break;
        case 2: color = aurora(p, u, dark, blue, cyan, mint); break;
        case 3: color = contours(p, u, dark, blue, cyan, mint); break;
        case 4: color = prism(p, u, dark, blue, cyan, mint); break;
        case 5: color = painterly(p, u, blue, cyan, mint); break;
    }
    // Fine fixed grain avoids flicker or a discontinuity at the loop boundary.
    float grain = float(grain_hash(id.x, id.y) & 65535u) / 65535.0 - 0.5;
    float amount = u.effect == 0 ? 0.105 * (0.25 + 0.75 * band) : (u.effect == 5 ? 0.075 : 0.015);
    color += grain * amount;
    output[id.y * u.width + id.x] = uchar4(uchar3(round(clamp(color, 0.0, 1.0) * 255.0)), 255);
}
