"""生成纯贴图眼睛的占位图层，供替换成手绘稿。

所有图层共用同一张 512×512 画布，画布中心即眼睛中心；
右侧为外眼角（眼白、眼线、眉毛由渲染器对另一只眼镜像），
虹膜、瞳孔、高光按屏幕方向绘制，两只眼不镜像。
画布与屏幕的比例、眼线所在高度见 eye.json 的 motion 段，改了那里要同步这里的常量。

用法：python scripts/make_placeholder_eyes.py assets/layers
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SIZE = 512
SS = 4  # 超采样后缩小，得到抗锯齿边缘
CANVAS = 0.32  # 与 motion.canvas 一致：画布半边长占屏幕高度的比例
UPPER_Y = -0.14  # 与 motion.upper_lid_y 一致
LOWER_Y = 0.13  # 与 motion.lower_lid_y 一致


def px(v):
    """屏幕单位转画布像素（相对中心）。"""
    return v / (2 * CANVAS) * SIZE


def canvas():
    return Image.new("RGBA", (SIZE * SS, SIZE * SS), (0, 0, 0, 0))


def finish(img, blur=0):
    img = img.resize((SIZE, SIZE), Image.LANCZOS)
    return img.filter(ImageFilter.GaussianBlur(blur)) if blur else img


def s(points):
    c = SIZE / 2
    return [((c + x) * SS, (c + y) * SS) for x, y in points]


def upper_curve(n=48):
    """上眼睑：内眼角(-x)起，最高点偏外侧，外眼角折一下。"""
    top, w = px(UPPER_Y), px(0.2)
    keys = [(-1, 0.2), (-0.75, 0.75), (-0.3, 0.97), (0.25, 1.0), (0.65, 0.9), (0.88, 0.62), (1, 0.25)]
    pts = []
    for i in range(n + 1):
        x = -1 + 2 * i / n
        for (x0, y0), (x1, y1) in zip(keys, keys[1:]):
            if x0 <= x <= x1:
                t = (x - x0) / (x1 - x0)
                t = t * t * (3 - 2 * t)  # 段内平滑，段间保留折角
                pts.append((x * w, top * (y0 + (y1 - y0) * t)))
                break
    return pts


def lower_curve(n=48):
    """下眼睑：圆润，外侧略低。"""
    low, w = px(LOWER_Y), px(0.2)
    pts = []
    for i in range(n + 1):
        x = 1 - 2 * i / n
        k = max(0.0, 1 - abs(x) ** 2.2) ** 0.5
        pts.append((x * w, low * k * (1 + 0.06 * x)))
    return pts


def sclera():
    img = canvas()
    d = ImageDraw.Draw(img)
    d.polygon(s(upper_curve() + lower_curve()), fill=(250, 250, 255, 255))
    # 上缘投影：占位用一条淡紫色带
    band = s(upper_curve() + [(x, y + px(0.05)) for x, y in reversed(upper_curve())])
    shade = canvas()
    ImageDraw.Draw(shade).polygon(band, fill=(190, 190, 225, 255))
    shade = shade.filter(ImageFilter.GaussianBlur(6 * SS))
    img = Image.composite(Image.alpha_composite(img, shade), img, img.split()[3])
    return finish(img)


def iris():
    img = canvas()
    r = px(0.125) * SS
    c = SIZE / 2 * SS
    d = ImageDraw.Draw(img)
    # 上深下浅的同心圆渐变
    steps = 40
    for i in range(steps):
        t = i / steps
        col = tuple(int(a + (b - a) * t) for a, b in zip((30, 40, 120), (110, 170, 240)))
        dy = r * 0.35 * t
        rr = r * (1 - 0.55 * t)
        d.ellipse((c - rr * 0.92, c - rr + dy, c + rr * 0.92, c + rr + dy), fill=col + (255,))
    d.ellipse((c - r * 0.92, c - r, c + r * 0.92, c + r), outline=(20, 25, 80, 255), width=int(4 * SS))
    return finish(img)


def pupil():
    img = canvas()
    c = SIZE / 2 * SS
    w, h = px(0.035) * SS, px(0.055) * SS
    ImageDraw.Draw(img).ellipse((c - w, c - h - 6 * SS, c + w, c + h - 6 * SS), fill=(15, 18, 60, 210))
    return finish(img)


def highlights():
    img = canvas()
    c = SIZE / 2 * SS
    d = ImageDraw.Draw(img)
    big = (px(-0.045) * SS, px(-0.045) * SS, px(0.03) * SS, px(0.037) * SS)
    d.ellipse((c + big[0] - big[2], c + big[1] - big[3], c + big[0] + big[2], c + big[1] + big[3]), fill=(255, 255, 255, 245))
    sx, sy, sr = px(0.045) * SS, px(0.05) * SS, px(0.012) * SS
    d.ellipse((c + sx - sr, c + sy - sr, c + sx + sr, c + sy + sr), fill=(255, 255, 255, 235))
    return finish(img)


def upper_lid():
    curve = upper_curve()
    # 内细外粗；外眼角再向外上方延出一点
    w = px(0.2)
    thick = [px(0.012) + px(0.028) * max(0.0, (x / w + 0.8) / 1.8) for x, _ in curve]
    outer = [(x, y - t) for (x, y), t in zip(curve, thick)]
    tip = (curve[-1][0] + px(0.03), curve[-1][1] - px(0.035))
    img = canvas()
    ImageDraw.Draw(img).polygon(s(outer + [tip] + list(reversed(curve)) ), fill=(38, 28, 58, 255))
    return finish(img)


def lower_lid():
    img = canvas()
    pts = [p for p in lower_curve() if p[0] > px(0.03)]
    ImageDraw.Draw(img).line(s(pts), fill=(140, 95, 115, 150), width=int(3 * SS), joint="curve")
    return finish(img)


def brow():
    img = canvas()
    y, w = px(-0.26), px(0.17)
    pts = [(x * w, y - px(0.02) * (1 - x * x)) for x in [i / 20 - 1 for i in range(41)]]
    ImageDraw.Draw(img).line(s(pts), fill=(70, 55, 90, 230), width=int(6 * SS), joint="curve")
    return finish(img)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "assets/layers")
    out.mkdir(parents=True, exist_ok=True)
    for name, fn in [("iris", iris), ("pupil", pupil), ("highlights", highlights), ("sclera", sclera),
                     ("upper_lid", upper_lid), ("lower_lid", lower_lid), ("brow", brow)]:
        fn().save(out / f"{name}.png")
        print(f"已生成 {out / name}.png")


if __name__ == "__main__":
    main()
