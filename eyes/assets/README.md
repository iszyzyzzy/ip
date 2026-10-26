# 眼睛素材包：修改后按 F5

当前默认皮肤是**纯贴图 Q 版蓝眼睛**：眼型、配色、高光全部由 `layers/` 下 7 张透明 PNG 决定，着色器只负责摆放、视线移动、眨眼和左右镜像。按拆分参考图绘制厚弧形上眼线、向下收尖的外眼角和纵向圆润虹膜；虹膜下半部为蓝色圆弧叠层，底部暖白反光与上方圆形高光放在独立的高光层。替换 PNG 后按 **F5**，不需要重新编译。

可编辑矢量源稿保存在 `layers-src/`，每张源稿共用 512×512 画布。修改 SVG 后重新导出：

```powershell
cargo run --example export_textures -- assets/layers-src assets/layers
```

这会覆盖七张 PNG。SVG 可任意放大编辑；运行时使用导出的 RGBA PNG，已按单屏 **720×720** 检查睁眼、半闭眼、闭眼和侧视。`scripts/make_placeholder_eyes.py` 仅保留用于调试占位稿，不要向正式 `assets/layers/` 运行它，否则会覆盖这套画稿。

## 画布约定

- 7 张图共用同一张 **512×512 画布**，画布中心 = 眼睛中心。画布在屏幕上的大小由 `motion.canvas` 决定（半边长占屏幕高度的比例，这套皮肤为 0.38）。
- **右侧画外眼角**（以人物左眼、屏幕右侧那只为准）。眼白、上下眼线、眉毛给另一只眼时自动水平镜像。
- 虹膜、瞳孔、高光**按屏幕方向画、不镜像**，所以两只眼的高光都在同一侧。
- 合成顺序：眼白 → 虹膜 → 瞳孔 → 高光（这四层都被**眼白的 alpha** 裁切，眼白轮廓就是眼眶形状）→ 下眼线 → 上眼线 → 眉毛。
- 虹膜、瞳孔、高光可以画在画布中心，大一点没关系，超出眼白的部分会被裁掉。
- RGBA straight alpha，透明处不要铺底色；其他尺寸会缩放成 512×512。

| 图层 | 文件 | 说明 |
| --- | --- | --- |
| 虹膜 | `layers/iris.png` | 随视线移动 |
| 瞳孔 | `layers/pupil.png` | 随视线移动，"Pupil" 滑块按默认 0.4 缩放 |
| 高光 | `layers/highlights.png` | 随视线移动 |
| 眼白 | `layers/sclera.png` | alpha 即眼眶形状，裁切上面三层 |
| 上眼线/睫毛 | `layers/upper_lid.png` | 眨眼时整体下移 |
| 下眼线 | `layers/lower_lid.png` | 眨眼时整体上移，闭眼淡出 |
| 眉毛 | `layers/brow.png` | 随眉毛参数上下移动 |

## 偏移配置：eye.json

`placement` 给每层单独调位置，不用改图：

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `offset` | `[0, 0]` | 以画布宽为单位，0.1 = 51 像素；x 向外眼角、y 向下为正。范围 -1–1 |
| `scale` | 1 | 以画布中心缩放，0.1–4 |
| `gaze` | 虹膜/瞳孔/高光 1，其他 0 | 跟随视线的比例；高光设 0.6 之类会产生视差 |

`motion` 是运动参数：

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `canvas` | 0.38 | 画布半边长占屏幕高度的比例，0.1–1 |
| `gaze_range` | `[0.075, 0.055]` | 视线到头时虹膜在屏幕上的位移（水平、竖直），0–0.3 |
| `blink_pivot` | 0.05 | 闭眼线所在高度（屏幕单位，相对眼睛中心，向下为正） |
| `upper_lid_y` | -0.113 | 上眼线在你的图里所在高度，眨眼时从这里移到 pivot |
| `lower_lid_y` | 0.174 | 下眼线所在高度 |

屏幕单位换算到像素：`像素偏移 = 屏幕值 / (2 × canvas) × 512`。当前 canvas 0.38 时，0.1 屏幕单位约 67 个贴图像素。

眨眼的做法：眼白及里面三层以 `blink_pivot` 为轴纵向压扁，上下眼线平移到 pivot 合成闭眼线。所以画眼线时，让它们在 pivot 高度重合后能拼成一条闭眼线就好。

`defaults` 里仍可调虹膜 tint（白色 = 原色）、背景色、眼角倾斜 `tilt`、光晕；`iris_size` 默认 0.2 时虹膜层按原大小显示，滑块按比例缩放虹膜、瞳孔、高光。`half_width`、`upper_height`、`lower_height`、`roundness`、`iris_aspect`、`brow_height`、`lid_texture_height`、`shadow_strength` 是旧程序化眼型用的参数，仍需保留在 JSON 中且在合法范围内，但纯贴图着色器不使用它们。

`placement` 和 `motion` 只在 F5 / ReloadAssets 时从文件读取，不进入 RPC 和 `eyes-settings.json`。

旧的模型提取贴图仍在 `png/`，SVG 源稿仍在根目录；它们按旧着色器的眼睑条带约定绘制，不能直接用于当前着色器（旧版 `eyes.wgsl` 见 git 历史）。

`eyes.wgsl` 也是运行时外部文件，复杂变形或混合逻辑可直接修改。加载前校验 JSON、图片和 WGSL/GPU 管线；重新加载失败会显示错误，并继续使用之前完整的素材包。

面板 VISIBLE LAYERS 和 RPC `Config.layers` 可以逐层切换显示；关闭虹膜不会连带关闭瞳孔/高光。`Config.art` 可修改上述旧眼型参数（纯贴图着色器不使用）。F5 会恢复素材包眼型和 defaults，但保留模拟目标、果冻和图层配置。新版 WGSL 的 Params uniform 包含图层掩码、左右果冻形变、眼型参数；请与新版 exe 一起更新着色器。

复制整个 assets 目录可以制作另一套皮肤，然后使用 `eyes.exe --assets D:\my-eye-skin` 加载。gRPC `ReloadAssets` 与 F5 使用相同的加载路径和错误回退逻辑。
