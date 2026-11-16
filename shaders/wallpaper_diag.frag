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
    float p = clamp(progress, 0.0, 1.0);
    float envelope = sin(p * 3.14159265);

    // Diagonal sweep from top-left (0,0) down-right to bottom-right (1,1)
    vec2 pCoord = vec2(qt_TexCoord0.x * aspect, qt_TexCoord0.y);
    vec2 diagDir = normalize(vec2(aspect, 1.0));
    vec2 perpDir = vec2(-diagDir.y, diagDir.x);

    float maxProj = dot(vec2(aspect, 1.0), diagDir);
    float d = dot(pCoord, diagDir) / maxProj;
    float perp = dot(pCoord - 0.5 * vec2(aspect, 1.0), perpDir);

    float arcBend = -0.055 * (perp * perp);
    float wave = (sin(perp * 7.5 + p * 7.0) + 0.45 * cos(perp * 13.0 - p * 5.0)) * 0.014 * envelope;

    float feather = 0.16;
    float front = p * (1.0 + feather + 0.12) - 0.06;
    float edge = smoothstep(front - feather, front + 0.02, d + arcBend + wave);
    float mask = 1.0 - edge;

    float band = mask * (1.0 - mask) * 4.0;
    vec2 refractOff = diagDir * band * 0.0085 * envelope / vec2(aspect, 1.0);

    float zoom = 1.0 + (1.0 - p) * 0.032;
    vec2 slideOff = diagDir * (-0.018 * (1.0 - p)) / vec2(aspect, 1.0);
    vec2 newUv = clamp((qt_TexCoord0 - 0.5) / zoom + 0.5 + slideOff - refractOff, 0.0, 1.0);
    vec2 oldUv = clamp(qt_TexCoord0 + refractOff * 0.45, 0.0, 1.0);

    vec4 oldCol = texture(oldTex, oldUv);
    vec4 newCol = texture(newTex, newUv);

    vec3 col = mix(oldCol.rgb, newCol.rgb, mask);
    float rimGlow = band * 0.22 * (0.35 + 0.65 * envelope);
    col += accentColor.rgb * rimGlow;

    fragColor = vec4(col, 1.0) * qt_Opacity;
}
