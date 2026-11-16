#version 440

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 accentColor;
    float progress;
    float aspect;
};

layout(binding = 1) uniform sampler2D oldTex;
layout(binding = 2) uniform sampler2D newTex;

void main() {
    vec2 center = vec2(0.5, 0.5);
    vec2 delta = (qt_TexCoord0 - center) * vec2(aspect, 1.0);
    float d = length(delta);
    float maxR = length(vec2(0.5 * aspect, 0.5));

    float p = clamp(progress, 0.0, 1.0);
    float angle = atan(delta.y, delta.x);
    float envelope = sin(p * 3.14159265);
    float wave = (sin(angle * 5.0 + p * 6.0) + 0.5 * cos(angle * 9.0 - p * 4.5)) * 0.015 * envelope;

    float feather = 0.16;
    float radius = p * (maxR + feather + 0.06) - 0.04;
    float edge = smoothstep(radius - feather, radius + 0.02, d + wave);
    float mask = 1.0 - edge;

    // Subtle refraction ripple along wavefront + gentle zoom settle on incoming wallpaper
    float band = mask * (1.0 - mask) * 4.0;
    vec2 dir = d > 0.0001 ? normalize(delta) : vec2(0.0);
    vec2 refractOff = dir * band * 0.009 * envelope / vec2(aspect, 1.0);

    float zoom = 1.0 + (1.0 - p) * 0.032;
    vec2 newUv = clamp((qt_TexCoord0 - 0.5) / zoom + 0.5 - refractOff, 0.0, 1.0);
    vec2 oldUv = clamp(qt_TexCoord0 + refractOff * 0.5, 0.0, 1.0);

    vec4 oldCol = texture(oldTex, oldUv);
    vec4 newCol = texture(newTex, newUv);

    vec3 col = mix(oldCol.rgb, newCol.rgb, mask);
    float rimGlow = band * 0.22 * (0.35 + 0.65 * envelope);
    col += accentColor.rgb * rimGlow;

    fragColor = vec4(col, 1.0) * qt_Opacity;
}
