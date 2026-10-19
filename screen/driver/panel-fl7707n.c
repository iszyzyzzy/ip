// SPDX-License-Identifier: GPL-2.0
#include <linux/delay.h>
#include <linux/gpio/consumer.h>
#include <linux/module.h>
#include <linux/of.h>
#include <drm/drm_connector.h>
#include <drm/drm_mipi_dsi.h>
#include <drm/drm_modes.h>
#include <drm/drm_panel.h>

struct fl7707n {
	struct drm_panel panel;
	struct mipi_dsi_device *dsi;
	struct gpio_desc *reset;
};

struct fl7707n_cmd {
	u8 len;
	u8 data[64];
	u16 delay_ms;
};

/* 厂家两通道初始化表单独生成，避免手工抄写 GIP 和 Gamma 参数。 */
#include "fl7707n-init.h"

static const struct drm_display_mode fl7707n_mode = {
	.clock = 48989,
	.hdisplay = 720,
	.hsync_start = 720 + 120,
	.hsync_end = 720 + 120 + 120,
	.htotal = 720 + 120 + 120 + 120,
	.vdisplay = 720,
	.vsync_start = 720 + 16,
	.vsync_end = 720 + 16 + 4,
	.vtotal = 720 + 16 + 4 + 16,
	.width_mm = 72,
	.height_mm = 72,
};

static struct fl7707n *to_fl7707n(struct drm_panel *panel)
{
	return container_of(panel, struct fl7707n, panel);
}

static int fl7707n_prepare(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);
	struct device *dev = &ctx->dsi->dev;
	unsigned int i;
	ssize_t ret;
	u8 status = 0;

	/* 转接板常供电；CAM_GPIO 经 1.8 V 缓冲只控制复位。 */
	gpiod_set_value_cansleep(ctx->reset, 1);
	msleep(20);
	gpiod_set_value_cansleep(ctx->reset, 0);
	msleep(120);
	for (i = 0; i < ARRAY_SIZE(fl7707n_init); i++) {
		const struct fl7707n_cmd *cmd = &fl7707n_init[i];

		ret = mipi_dsi_dcs_write_buffer(ctx->dsi, cmd->data, cmd->len);
		if (ret < 0) {
			dev_err(dev, "初始化命令 0x%02x 发送失败: %zd\n", cmd->data[0], ret);
			gpiod_set_value_cansleep(ctx->reset, 1);
			return ret;
		}
		if (cmd->delay_ms)
			msleep(cmd->delay_ms);
	}
	/* 写入成功只证明主机发送完成，回读用于区分面板实际应答。 */
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x0a, &status, 1);
	dev_info(dev, "初始化表已发送，电源状态回读: ret=%zd value=0x%02x\n", ret, status);
	return 0;
}

static int fl7707n_unprepare(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);

	mipi_dsi_dcs_set_display_off(ctx->dsi);
	msleep(50);
	mipi_dsi_dcs_enter_sleep_mode(ctx->dsi);
	msleep(120);
	gpiod_set_value_cansleep(ctx->reset, 1);
	return 0;
}

static int fl7707n_get_modes(struct drm_panel *panel,
			    struct drm_connector *connector)
{
	struct drm_display_mode *mode;

	mode = drm_mode_duplicate(connector->dev, &fl7707n_mode);
	if (!mode)
		return -ENOMEM;
	drm_mode_set_name(mode);
	mode->type = DRM_MODE_TYPE_DRIVER | DRM_MODE_TYPE_PREFERRED;
	drm_mode_probed_add(connector, mode);
	connector->display_info.width_mm = fl7707n_mode.width_mm;
	connector->display_info.height_mm = fl7707n_mode.height_mm;
	connector->display_info.bpc = 8;
	return 1;
}

static const struct drm_panel_funcs fl7707n_funcs = {
	.prepare = fl7707n_prepare,
	.unprepare = fl7707n_unprepare,
	.get_modes = fl7707n_get_modes,
};

static int fl7707n_probe(struct mipi_dsi_device *dsi)
{
	struct fl7707n *ctx;
	int ret;

	ctx = devm_kzalloc(&dsi->dev, sizeof(*ctx), GFP_KERNEL);
	if (!ctx)
		return -ENOMEM;
	ctx->dsi = dsi;
	ctx->reset = devm_gpiod_get(&dsi->dev, "reset", GPIOD_OUT_HIGH);
	if (IS_ERR(ctx->reset))
		return dev_err_probe(&dsi->dev, PTR_ERR(ctx->reset), "无法获取复位 GPIO\n");
	mipi_dsi_set_drvdata(dsi, ctx);
	dsi->lanes = 2;
	dsi->format = MIPI_DSI_FMT_RGB888;
	dsi->mode_flags = MIPI_DSI_MODE_VIDEO | MIPI_DSI_MODE_VIDEO_SYNC_PULSE |
			  MIPI_DSI_MODE_LPM;
	drm_panel_init(&ctx->panel, &dsi->dev, &fl7707n_funcs, DRM_MODE_CONNECTOR_DSI);
	/* 发送初始化命令前需要 DSI 主机进入 LP-11。 */
	ctx->panel.prepare_prev_first = true;
	drm_panel_add(&ctx->panel);
	ret = mipi_dsi_attach(dsi);
	if (ret)
		drm_panel_remove(&ctx->panel);
	else
		dev_info(&dsi->dev, "FL7707N 720x720 双通道面板已注册\n");
	return ret;
}

static void fl7707n_remove(struct mipi_dsi_device *dsi)
{
	struct fl7707n *ctx = mipi_dsi_get_drvdata(dsi);

	mipi_dsi_detach(dsi);
	drm_panel_remove(&ctx->panel);
}

static const struct of_device_id fl7707n_of_match[] = {
	{ .compatible = "huadi,hd40031c30-fl7707n" },
	{ }
};
MODULE_DEVICE_TABLE(of, fl7707n_of_match);

static struct mipi_dsi_driver fl7707n_driver = {
	.probe = fl7707n_probe,
	.remove = fl7707n_remove,
	.driver = {
		.name = "panel-fl7707n",
		.of_match_table = fl7707n_of_match,
	},
};
module_mipi_dsi_driver(fl7707n_driver);

MODULE_DESCRIPTION("HD40031C30 FL7707N 720x720 DSI panel");
MODULE_LICENSE("GPL");
