// SPDX-License-Identifier: GPL-2.0
#include <linux/delay.h>
#include <linux/gpio/consumer.h>
#include <linux/module.h>
#include <linux/of.h>
#include <video/mipi_display.h>
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
	.width_mm = 102,
	.height_mm = 102,
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
	u8 id[3] = { 0 };

	/* 转接板常供电；CAM_GPIO 经 1.8 V 缓冲只控制复位。 */
	gpiod_set_value_cansleep(ctx->reset, 1);
	msleep(20);
	gpiod_set_value_cansleep(ctx->reset, 0);
	msleep(120);
	/* Sleep In 也允许读 0A，先区分通信本身失败还是厂家表执行后失效。 */
	ret = mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
	if (ret < 0)
		dev_warn(dev, "初始化前设置回包长度失败: %zd\n", ret);
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x0a, &status, 1);
	dev_info(dev, "初始化前电源状态回读: ret=%zd value=0x%02x\n", ret, status);
	status = 0;
	for (i = 0; i < ARRAY_SIZE(fl7707n_init); i++) {
		const struct fl7707n_cmd *cmd = &fl7707n_init[i];
		u8 data[64];

		memcpy(data, cmd->data, cmd->len);
		/* 视频稳定前关闭 PWM 引脚，避免提前启动板上背光升压。 */
		if (data[0] == 0xc7)
			data[1] &= ~BIT(7);
		/* SY7200A 推荐 PWM 不低于 20 kHz，厂家表的约 1 kHz 不适用。 */
		if (data[0] == 0xc8) {
			data[1] = 0x00;
			data[2] = 0x03;
		}
		ret = mipi_dsi_dcs_write_buffer(ctx->dsi, data, cmd->len);
		if (ret < 0) {
			dev_err(dev, "初始化命令 0x%02x 发送失败: %zd\n", cmd->data[0], ret);
			gpiod_set_value_cansleep(ctx->reset, 1);
			return ret;
		}
		/* 厂家表仅等待 50 ms；现场时序要求 Display On 后超过 120 ms。 */
		if (data[0] == MIPI_DCS_SET_DISPLAY_ON)
			msleep(150);
		else if (cmd->delay_ms)
			msleep(cmd->delay_ms);
	}
	/* 写入成功只证明主机发送完成，回读用于区分面板实际应答。 */
	ret = mipi_dsi_set_maximum_return_packet_size(ctx->dsi, sizeof(id));
	if (ret < 0)
		dev_warn(dev, "设置回读包长度失败: %zd\n", ret);
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x04, id, sizeof(id));
	dev_info(dev, "面板 ID 回读: ret=%zd value=%02x %02x %02x\n", ret, id[0], id[1], id[2]);
	mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x0a, &status, 1);
	dev_info(dev, "初始化表已发送（Display On 后等待 150 ms），电源状态回读: ret=%zd value=0x%02x\n", ret, status);
	return 0;
}

static int fl7707n_set_pwm_output(struct fl7707n *ctx, bool enable)
{
	unsigned int i;
	u8 data[64];
	ssize_t ret;

	/* C7 还含模块专用参数，开关 PWM 时保留厂家表的其余字节。 */
	for (i = 0; i < ARRAY_SIZE(fl7707n_init); i++) {
		const struct fl7707n_cmd *cmd = &fl7707n_init[i];

		if (cmd->data[0] != 0xc7)
			continue;
		memcpy(data, cmd->data, cmd->len);
		if (enable)
			data[1] |= BIT(7);
		else
			data[1] &= ~BIT(7);
		ret = mipi_dsi_dcs_write_buffer(ctx->dsi, data, cmd->len);
		return ret < 0 ? ret : 0;
	}
	return -EINVAL;
}

static int fl7707n_enable(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);
	int ret;

	/* RP1 在 panel enable 前启动视频；留 50 ms，超过图中的 33.2 ms。 */
	msleep(50);
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x51, (u8[]) { 0x80, 0x80 }, 2);
	if (ret < 0)
		goto backlight_failed;
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x24 }, 1);
	if (ret < 0)
		goto backlight_failed;
	ret = fl7707n_set_pwm_output(ctx, true);
	if (ret < 0)
		goto backlight_failed;
	dev_info(&ctx->dsi->dev, "视频启动后等待 50 ms，背光 PWM 开启命令已发送，亮度 0x80\n");
	return 0;

backlight_failed:
	dev_err(&ctx->dsi->dev, "背光控制命令发送失败: %d\n", ret);
	fl7707n_set_pwm_output(ctx, false);
	return ret;
}

static int fl7707n_disable(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);
	int ret;

	/* 先关背光，再由 DRM 停视频、执行 Display Off 和 Sleep In。 */
	ret = fl7707n_set_pwm_output(ctx, false);
	if (ret < 0)
		return ret;
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x00 }, 1);
	return ret < 0 ? ret : 0;
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
	.enable = fl7707n_enable,
	.disable = fl7707n_disable,
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
