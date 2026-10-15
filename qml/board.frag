// The theme picker's board (ThemePicker.qml): the inside of the sheet's frame, cleared of windows. The
// paper is the compositor's own (compositor/src/sheet.rs: the gradient, the three blooms of glow, the
// 5 mm dots, the dither), worked out over the whole output so that it meets the margins without a seam;
// with a wallpaper, the picture blurred instead. Outside the frame's interior, and over the title block
// when no window covers it, the board is clear, so the sheet's own margins and title block show.
// Compiled to board.frag.qsb with qsb (the Makefile).
#version 440

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 paper;
    vec4 paperLight;
    vec4 paperShade;
    vec4 glow;
    vec4 ink;
    vec4 shape;      // angle (radians), midpoint, blend, shading
    vec4 strength;   // glow strength, glow size, pattern strength, physical pixels per millimetre
    vec4 physical;   // the output's size in physical pixels; z: 1 with a wallpaper
    vec4 interior;   // the frame's interior in physical pixels: left, top, right, bottom
    vec4 hole;       // the title block, left, top, right, bottom (empty: none)
};
layout(binding = 1) uniform sampler2D picture;   // the wallpaper, blurred, over the interior

float bloom(vec2 uv, vec2 c, float r, float k) {
    vec2 d = vec2((uv.x - c.x) * physical.x / physical.y, uv.y - c.y);
    float rr = r * strength.y / 2.0;
    return min(1.0, k * strength.x) * exp(-dot(d, d) / (2.0 * rr * rr));
}

void main() {
    vec2 uv = qt_TexCoord0;
    vec2 px = floor(uv * physical.xy);
    bool inside = px.x >= interior.x && px.x < interior.z && px.y >= interior.y && px.y < interior.w;
    bool overBlock = px.x >= hole.x && px.x < hole.z && px.y >= hole.y && px.y < hole.w;
    if (!inside || overBlock) {
        fragColor = vec4(0.0);
        return;
    }
    if (physical.z > 0.5) {
        vec2 at = (px + 0.5 - interior.xy) / (interior.zw - interior.xy);
        fragColor = vec4(texture(picture, at).rgb, 1.0) * qt_Opacity;
        return;
    }
    float ca = cos(shape.x), sa = sin(shape.x);
    float t = (ca * uv.x + sa * uv.y) / (ca + sa);
    float p = clamp(0.5 + (t - shape.y) / shape.z, 0.0, 1.0);
    p = p * p * (3.0 - 2.0 * p);
    vec3 light = paper.rgb + (paperLight.rgb - paper.rgb) * shape.w;
    vec3 shade = paper.rgb + (paperShade.rgb - paper.rgb) * shape.w;
    vec3 c = p < 0.5 ? mix(light, paper.rgb, p / 0.5) : mix(paper.rgb, shade, (p - 0.5) / 0.5);
    c += (glow.rgb - c) * bloom(uv, vec2(0.16, 0.20), 0.55, 0.50);
    c += (glow.rgb - c) * bloom(uv, vec2(0.80, 0.30), 0.40, 0.26);
    c += (glow.rgb - c) * bloom(uv, vec2(0.42, 0.88), 0.48, 0.30);
    // the dots every 5 mm, counted from the bottom-left corner
    float stp = floor(5.0 * strength.w);
    float m = (mod(px.x, stp) < 1.0 && mod(physical.y - 1.0 - px.y, stp) < 1.0) ? 0.6 : 0.0;
    c += (ink.rgb - c) * m * strength.z;
    float n = fract(sin(dot(px, vec2(12.9898, 78.233))) * 43758.5453) - 0.5;
    c += n / 255.0;
    fragColor = vec4(c, 1.0) * qt_Opacity;
}
