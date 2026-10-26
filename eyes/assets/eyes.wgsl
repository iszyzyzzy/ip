// 外部运行时着色器：纯贴图合成，F5 重新加载本文件、eye.json 和全部贴图。
// 眼型、配色全部由贴图决定；这里只负责摆放、视线移动、眨眼和镜像。
struct Params {
    viewport: vec4f, left: vec4f, right: vec4f, shape: vec4f,
    iris: vec4f, background: vec4f, extra: vec4f,
    layers_a: vec4f, layers_b: vec4f, jelly_left: vec4f, jelly_right: vec4f,
    contour: vec4f, detail: vec4f,
    // 0..6 各图层 offset.xy / scale / gaze；7 = gaze_range.xy, blink_pivot, canvas；8 = upper_lid_y, lower_lid_y
    placement: array<vec4f, 9>,
}
@group(0) @binding(0) var<uniform> u: Params;
@group(0) @binding(2) var layers: texture_2d_array<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
    var v: Vertex;
    v.position = vec4f(p[index], 0., 1.);
    v.uv = p[index] * vec2f(0.5, -0.5) + 0.5;
    return v;
}
fn layer(uv: vec2f, index: i32) -> vec4f {
    let texel = textureSampleLevel(layers, linear_sampler, uv, index, 0.);
    // 贴图外完全透明，避免边缘像素被拉伸到整张脸上。
    let inside = all(uv >= vec2f(0.)) && all(uv <= vec2f(1.));
    let masks = array<vec4f, 2>(u.layers_a, u.layers_b);
    return vec4f(texel.rgb, texel.a * select(0., 1., inside) * masks[index / 4][index % 4]);
}
fn over(base: vec3f, paint: vec4f) -> vec3f { return mix(base, paint.rgb, paint.a); }
// 把眼睛局部坐标换成第 index 层的贴图 UV。
// p 已按左右眼镜像（x 指向外眼角）；mirror=false 的图层按屏幕方向采样，
// 这样两只眼的高光、虹膜光照都在同一侧。
fn place(p: vec2f, index: i32, gaze: vec2f, side: f32, mirror: bool, extra_scale: f32) -> vec2f {
    let l = u.placement[index];
    let size = 2. * u.placement[7].w;
    var d = (p - l.xy * size - gaze * l.w) / (size * l.z * extra_scale);
    if !mirror { d.x *= side; }
    return d + 0.5;
}
fn eye(pos: vec2f, side: f32, data: vec4f, base: vec3f) -> vec3f {
    var p = pos - vec2f(side * u.shape.x, 0.);
    let jelly = select(u.jelly_right, u.jelly_left, side > 0.);
    p -= jelly.xy;
    p.x -= p.y * jelly.w;
    p /= vec2f(1. + jelly.z, 1. - jelly.z);
    p.x *= side;
    p.y += p.x * u.extra.z;
    let motion = u.placement[7];
    let lids = u.placement[8];
    let opening = clamp(data.z, 0., 1.4);
    let gaze = vec2f(data.x * side, data.y) * motion.xy;
    var color = base;
    // 光晕沿用原来的可调参数，所有图层都关掉时不显示。
    let any_layer = min(1., dot(u.layers_a, vec4f(1.)) + dot(u.layers_b, vec4f(1.)));
    let halo = exp(-dot(p / vec2f(0.25, 0.23), p / vec2f(0.25, 0.23)) * 2.2);
    color += u.iris.rgb * halo * u.extra.x * 0.10 * any_layer;

    // 眨眼：眼内各层以 blink_pivot 为轴纵向压扁；眼白的 alpha 就是眼眶形状，
    // 虹膜等内层都被它裁切，所以眼型完全由 sclera.png 决定。
    let pivot = motion.z;
    let q = vec2f(p.x, pivot + (p.y - pivot) / max(opening, 0.02));
    let visible = smoothstep(0.02, 0.08, opening);
    let sclera = layer(place(q, 3, vec2f(0.), side, true, 1.), 3);
    // 从当前背景开始合成，避免关闭眼白后仍把它的 RGB 当成底色。
    var inner = over(color, sclera);
    // 虹膜大小、瞳孔大小两个滑块按默认值 0.2 / 0.4 等比缩放对应图层。
    let iris_scale = u.shape.y / 0.2;
    let iris = layer(place(q, 0, gaze, side, false, iris_scale), 0);
    inner = over(inner, vec4f(iris.rgb * u.iris.rgb, iris.a));
    let pupil = layer(place(q, 1, gaze, side, false, iris_scale * u.extra.y / 0.4), 1);
    inner = over(inner, pupil);
    let highlight = layer(place(q, 2, gaze, side, false, iris_scale), 2);
    inner = over(inner, highlight);
    // 眼白层隐藏时仍需要眼眶裁切，直接读原始 alpha。
    let socket_uv = place(q, 3, vec2f(0.), side, true, 1.);
    let socket = textureSampleLevel(layers, linear_sampler, socket_uv, 3, 0.).a;
    let in_canvas = select(0., 1., all(socket_uv >= vec2f(0.)) && all(socket_uv <= vec2f(1.)));
    color = over(color, vec4f(inner, socket * in_canvas * visible));

    // 上下眼线从各自位置平移到 pivot 合成闭眼线；睁大（opening>1）时反向外扩。
    let close = 1. - opening;
    let lower_p = p - vec2f(0., (pivot - lids.y) * close);
    let upper_p = p - vec2f(0., (pivot - lids.x) * close);
    let lower = layer(place(lower_p, 5, vec2f(0.), side, true, 1.), 5);
    color = over(color, vec4f(lower.rgb, lower.a * smoothstep(0.02, 0.3, opening)));
    color = over(color, layer(place(upper_p, 4, vec2f(0.), side, true, 1.), 4));
    let brow_p = p + vec2f(0., data.w * 0.05);
    color = over(color, layer(place(brow_p, 6, vec2f(0.), side, true, 1.), 6));
    return color;
}
@fragment fn fs_main(v: Vertex) -> @location(0) vec4f {
    let aspect = u.viewport.x / max(u.viewport.y, 1.);
    // 单眼输出有自己的视口和中心，不依赖双眼窗口的眼距裁切。
    let single = abs(u.extra.w) > 0.5;
    let fit = select(min(1., aspect / 2.5), min(1.5, aspect / 0.68), single) * u.viewport.w;
    var p = (v.uv - 0.5) * vec2f(aspect, 1.) / max(fit, 0.1);
    let roll = u.shape.w;
    p = mat2x2f(cos(roll), -sin(roll), sin(roll), cos(roll)) * p;
    var color = u.background.rgb;
    if single {
        let side = select(-1., 1., u.extra.w > 0.);
        color = eye(p, side, select(u.right, u.left, side > 0.), color);
    } else {
        color = eye(p, -1., u.right, color);
        color = eye(p, 1., u.left, color);
    }
    return vec4f(color, 1.);
}
