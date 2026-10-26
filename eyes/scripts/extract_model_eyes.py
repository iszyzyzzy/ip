"""从 Cubism 默认网格和图集提取眼睛图层，运行时不依赖 Cubism。

先运行 inspect_model.cjs 得到 drawables.json；本脚本只读取用户模型。
"""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("model", type=Path)
    parser.add_argument("--meshes", type=Path, default=Path("target/model-extract/drawables.json"))
    parser.add_argument("--output", type=Path, default=Path("assets"))
    args = parser.parse_args()
    model_json = next(args.model.glob("*.model3.json"))
    model = json.loads(model_json.read_text(encoding="utf-8"))
    atlas_path = args.model / model["FileReferences"]["Textures"][0]
    atlas_image = Image.open(atlas_path).convert("RGBA")
    atlas = np.asarray(atlas_image)
    meshes = json.loads(args.meshes.read_text(encoding="utf-8"))
    out = args.output / "png"
    raw = args.output / "model-source"
    out.mkdir(parents=True, exist_ok=True)
    raw.mkdir(parents=True, exist_ok=True)
    records = []

    def crop(index):
        row = meshes[index]
        uv = np.array(row["uv"]).reshape(-1, 2)
        xy = uv * atlas_image.size
        xy[:, 1] = atlas_image.height - xy[:, 1]
        box = np.r_[np.floor(xy.min(0)), np.ceil(xy.max(0))].astype(int)
        paint = atlas_image.crop(tuple(box))
        mask = Image.new("L", paint.size)
        draw = ImageDraw.Draw(mask)
        for tri in np.array(row["indices"]).reshape(-1, 3):
            draw.polygon([tuple(v) for v in xy[tri] - box[:2]], fill=255)
        a = np.array(paint)
        a[..., 3] = ((a[..., 3].astype(float) * np.array(mask)) / 255).astype(np.uint8)
        paint = Image.fromarray(a)
        paint.save(raw / f'{row["id"]}.png')
        records.append({"drawable": row["id"], "index": index, "atlas_box": box.tolist()})
        return paint

    def raster(indices, bounds):
        # 按默认网格位置对齐瞳孔和高光，避免只按裁切矩形粘贴造成漂移。
        result = np.zeros((512, 512, 4), dtype=np.float32)
        lo, hi = bounds
        for index in sorted(indices, key=lambda i: meshes[i]["order"]):
            row = meshes[index]
            v = np.array(row["vertices"]).reshape(-1, 2)
            xy = (v - lo) / (hi - lo) * 490 + 11
            xy[:, 1] = 512 - xy[:, 1]
            uv = np.array(row["uv"]).reshape(-1, 2)
            uv[:, 1] = 1 - uv[:, 1]
            uv *= atlas_image.size
            for tri in np.array(row["indices"]).reshape(-1, 3):
                p = xy[tri]
                start = np.maximum(np.floor(p.min(0)).astype(int), 0)
                end = np.minimum(np.ceil(p.max(0)).astype(int), 512)
                if np.any(end <= start):
                    continue
                yy, xx = np.mgrid[start[1]:end[1], start[0]:end[0]]
                a, b, c = p
                denom = (b[1]-c[1])*(a[0]-c[0]) + (c[0]-b[0])*(a[1]-c[1])
                if abs(denom) < 1e-10:
                    continue
                w0 = ((b[1]-c[1])*(xx+.5-c[0]) + (c[0]-b[0])*(yy+.5-c[1])) / denom
                w1 = ((c[1]-a[1])*(xx+.5-c[0]) + (a[0]-c[0])*(yy+.5-c[1])) / denom
                w2 = 1-w0-w1
                inside = (w0 >= -1e-6) & (w1 >= -1e-6) & (w2 >= -1e-6)
                sample = w0[..., None]*uv[tri[0]] + w1[..., None]*uv[tri[1]] + w2[..., None]*uv[tri[2]]
                sx = np.clip(sample[..., 0], 0, atlas_image.width-2)
                sy = np.clip(sample[..., 1], 0, atlas_image.height-2)
                ix, iy = np.floor(sx).astype(int), np.floor(sy).astype(int)
                fx, fy = (sx-ix)[..., None], (sy-iy)[..., None]
                paint = ((atlas[iy, ix]*(1-fx)+atlas[iy, ix+1]*fx)*(1-fy)
                         +(atlas[iy+1, ix]*(1-fx)+atlas[iy+1, ix+1]*fx)*fy).astype(np.float32) / 255
                alpha = paint[..., 3:4] * inside[..., None] * row["opacity"]
                dst = result[start[1]:end[1], start[0]:end[0]]
                dst[..., :3] = paint[..., :3]*alpha + dst[..., :3]*(1-alpha)
                dst[..., 3:4] = alpha + dst[..., 3:4]*(1-alpha)
        alpha = result[..., 3:4]
        result[..., :3] = np.divide(result[..., :3], alpha, out=np.zeros_like(result[..., :3]), where=alpha > 0)
        return Image.fromarray(np.clip(result*255, 0, 255).astype(np.uint8))

    selected = [0, 10, 11, 12, 13, 15, 18, 19, 86]
    source = {i: crop(i) for i in selected}
    iris_v = np.array(meshes[13]["vertices"]).reshape(-1, 2)
    bounds = (iris_v.min(0), iris_v.max(0))
    for name, indices in [("iris", [13]), ("pupil", [12]), ("highlights", [10, 11, 86])]:
        raster(indices, bounds).save(out / f"{name}.png")

    # 眼白在模型中是尖角形，取内部渐变色并由眼屏的 Q 版轮廓裁切。
    white = np.array(source[19])
    strip = white[:, white.shape[1]//2:white.shape[1]//2+1].copy()
    strip[..., 3] = 255
    Image.fromarray(strip).resize((512, 512), Image.Resampling.LANCZOS).save(out / "sclera.png")

    def ribbon(index, name, scale):
        image = source[index]
        image = image.resize((460, max(1, round(image.height*460/image.width))), Image.Resampling.LANCZOS)
        a = np.asarray(image)
        dst = Image.new("RGBA", (512, 512))
        for x in range(image.width):
            weights = a[:, x, 3].astype(float)
            if weights.sum() < 1:
                continue
            center = int(np.dot(np.arange(image.height), weights)/weights.sum())
            column = image.crop((x, 0, x+1, image.height))
            column = column.resize((1, max(1, round(image.height*scale))), Image.Resampling.LANCZOS)
            dst.paste(column, (x+26, 256-round(center*scale)))
        dst.save(out / f"{name}.png")

    ribbon(15, "upper_lid", 1.5)
    ribbon(18, "lower_lid", .6)
    brow = Image.new("RGBA", (512, 512))
    b = source[0].resize((460, 130), Image.Resampling.LANCZOS)
    brow.alpha_composite(b, (26, 190))
    brow.save(out / "brow.png")
    provenance = {
        "model": model_json.name,
        "atlas": model["FileReferences"]["Textures"][0],
        "atlas_sha256": hashlib.sha256(atlas_path.read_bytes()).hexdigest(),
        "drawables": records,
        "adaptation": "虹膜/瞳孔/高光按默认模型网格对齐；眼线展平为条带；眼白保留源渐变并由 Q 版眼形裁切；左右眼源网格共用图集。",
    }
    (args.output / "model-source.json").write_text(json.dumps(provenance, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print("已提取眼睛素材：", out)


if __name__ == "__main__":
    main()
