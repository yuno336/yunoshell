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

void main() {
    vec2 center = vec2(0.5, 0.5);
    vec2 delta = (qt_TexCoord0 - center) * vec2(aspect, 1.0);
    float d = length(delta);
    float maxR = length(vec2(0.5 * aspect, 0.5));

    float p = clamp(progress, 0.0, 1.0);
    float env = sin(p * 3.14159265);
    float radius = p * (maxR + 0.24);

    float distToRing = d - radius;
    float ring = exp(-(distToRing * distToRing) / 0.010);
    float wash = (1.0 - smoothstep(radius - 0.24, radius + 0.04, d)) * 0.055;
    float alpha = clamp((ring * 0.20 + wash) * env, 0.0, 0.28) * qt_Opacity;

    fragColor = vec4(accentColor.rgb * alpha, alpha);
}
