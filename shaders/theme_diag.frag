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
    float p = clamp(progress, 0.0, 1.0);
    float env = sin(p * 3.14159265);

    // Diagonal light beam sweeping from top-left down-right to bottom-right
    vec2 pCoord = vec2(qt_TexCoord0.x * aspect, qt_TexCoord0.y);
    vec2 diagDir = normalize(vec2(aspect, 1.0));
    vec2 perpDir = vec2(-diagDir.y, diagDir.x);

    float maxProj = dot(vec2(aspect, 1.0), diagDir);
    float d = dot(pCoord, diagDir) / maxProj;
    float perp = dot(pCoord - 0.5 * vec2(aspect, 1.0), perpDir);

    float arcBend = -0.05 * (perp * perp);
    float front = p * 1.24 - 0.12;
    float distToWave = (d + arcBend) - front;

    float beam = exp(-(distToWave * distToWave) / 0.0085);
    float softTrail = (1.0 - smoothstep(-0.26, 0.04, distToWave)) * 0.055;
    float alpha = clamp((beam * 0.20 + softTrail) * env, 0.0, 0.28) * qt_Opacity;

    fragColor = vec4(accentColor.rgb * alpha, alpha);
}
