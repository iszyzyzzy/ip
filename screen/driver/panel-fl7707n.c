// SPDX-License-Identifier: GPL-2.0
#include <linux/delay.h>
#include <linux/gpio/consumer.h>
#include <linux/io.h>
#include <linux/iopoll.h>
#include <linux/module.h>
#include <linux/of.h>
#include <linux/platform_device.h>
#include <video/mipi_display.h>
#include <drm/drm_connector.h>
#include <drm/drm_mipi_dsi.h>
#include <drm/drm_modes.h>
#include <drm/drm_panel.h>

struct fl7707n {
	struct drm_panel panel;
	struct mipi_dsi_device *dsi;
	struct gpio_desc *reset;
	void __iomem *rp1_regs;
};

static unsigned int meter_reset_ms;
module_param(meter_reset_ms, uint, 0644);
MODULE_PARM_DESC(meter_reset_ms, "保持 LP-11，按低/高/低阶段延长复位供万用表测量（最大 30 秒/阶段）");

static unsigned int brightness_bytes = 2;
module_param(brightness_bytes, uint, 0644);
MODULE_PARM_DESC(brightness_bytes, "51 亮度命令的参数数目：1 或 2，默认按厂家手册发 2 个");

static bool vendor_generic;
module_param(vendor_generic, bool, 0644);
MODULE_PARM_DESC(vendor_generic, "用 Generic 包发送同一厂家扩展命令表，供包类型对照");

static bool backlight_only;
module_param(backlight_only, bool, 0644);
MODULE_PARM_DESC(backlight_only, "保持 Display Off，仅按手册公开的 C7/C8 参数测试背光 PWM");

static bool video_before_on = true;
module_param(video_before_on, bool, 0644);
MODULE_PARM_DESC(video_before_on, "先启动视频并等待 120 ms，再发送 Display On，供同步电源时序对照");

static bool skip_ef;
module_param(skip_ef, bool, 0644);
MODULE_PARM_DESC(skip_ef, "诊断时跳过厂家 EF 命令，其含义尚未确认，默认保留");

static bool internal_sync;
module_param(internal_sync, bool, 0644);
MODULE_PARM_DESC(internal_sync, "诊断时将 B8 电源同步源改为内部 Hsync，默认保留厂家外部同步");

static bool brightness_probe;
module_param(brightness_probe, bool, 0644);
MODULE_PARM_DESC(brightness_probe, "在 Command Mode 临时启用 BL，对照亮度长短包写入，默认关闭");

static bool jd9365_probe;
module_param(jd9365_probe, bool, 0444);
MODULE_PARM_DESC(jd9365_probe, "仅用 JD9365 分页及只读寄存器识别，结束保持复位，不发送 FL 初始化表");

static bool jd9365_start;
module_param(jd9365_start, bool, 0444);
MODULE_PARM_DESC(jd9365_start, "配合 jd9365_probe，仅用 JD 解锁、两通道及标准 DCS 测试现有 OTP 配置");

static bool jd9365_tcon_720;
module_param(jd9365_tcon_720, bool, 0644);
MODULE_PARM_DESC(jd9365_tcon_720, "JD实验：仅将页1的40/41调整为720列、720行，不修改模拟电源或GIP");

static bool jd9365_power_nt;
module_param(jd9365_power_nt, bool, 0644);
MODULE_PARM_DESC(jd9365_power_nt, "JD实验：将页1的55低四位设为2，选择外部NT电源IC模式，保留电压/倍率参数");

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

static void fl7707n_host_status(struct fl7707n *ctx, const char *stage)
{
	void __iomem *r = ctx->rp1_regs;
	u32 err0, err1;

	if (!r)
		return;
	/* 错误状态读取会清除锁存位，分阶段采样才能定位到本次请求。 */
	err0 = readl(r + 0xbc);
	err1 = readl(r + 0xc0);
	dev_info(&ctx->dsi->dev,
		 "%s: LPCLK=%#x PHY=%#x PKT=%#x RXCFG=%#x RDWAIT=%#x IF=%#x ERR0=%#x ERR1=%#x%s\n",
		 stage, readl(r + 0x94), readl(r + 0xb0), readl(r + 0x74),
		 readl(r + 0x2c), readl(r + 0xf4), readl(r + 0xa4), err0, err1,
		 err1 & BIT(1) ? " LP接收超时" : "");
}

static int fl7707n_identify_jd9365(struct fl7707n *ctx)
{
	static const u8 unlock[][2] = {
		{ 0xe0, 0x00 }, { 0xe1, 0x93 }, { 0xe2, 0x65 }, { 0xe3, 0xf8 },
	};
	static const u8 pages[] = { 0, 1, 0, 1, 0 };
	static const u8 regs[] = { 0xe0, 0x8d, 0x8e, 0x8f, 0x80 };
	unsigned int i, j;
	int ret;

	/* 不触及 OTP、模拟电源或 GIP；验证分页相关性比模块可定义的 04 ID 更有区分度。 */
	for (i = 0; i < ARRAY_SIZE(unlock); i++) {
		ret = mipi_dsi_dcs_write_buffer(ctx->dsi, unlock[i], sizeof(unlock[i]));
		if (ret < 0)
			return ret;
	}
	for (i = 0; i < ARRAY_SIZE(pages); i++) {
		ret = mipi_dsi_dcs_write(ctx->dsi, 0xe0, &pages[i], 1);
		if (ret < 0)
			return ret;
		msleep(5);
		for (j = 0; j < ARRAY_SIZE(regs); j++) {
			u8 value = 0;

			mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
			ret = mipi_dsi_dcs_read(ctx->dsi, regs[j], &value, 1);
			dev_info(&ctx->dsi->dev, "JD分页识别：page=%u reg=%02x ret=%d value=%02x\n",
				 pages[i], regs[j], ret, value);
			if (ret != 1 ||
			    (regs[j] == 0xe0 && value != pages[i]) ||
			    (pages[i] == 1 && regs[j] == 0x8d && value != 0x93) ||
			    (pages[i] == 1 && regs[j] == 0x8e && value != 0x65))
				return -ENODEV;
		}
	}
	ret = mipi_dsi_dcs_write(ctx->dsi, 0xe0, (u8[]) { 3 }, 1);
	if (ret < 0)
		return ret;
	for (i = 0x98; i <= 0x9c; i++) {
		u8 value = 0;

		ret = mipi_dsi_dcs_read(ctx->dsi, i, &value, 1);
		dev_info(&ctx->dsi->dev, "JD默认PWM：page=3 reg=%02x ret=%d value=%02x\n",
			 i, ret, value);
	}
	return mipi_dsi_dcs_write(ctx->dsi, 0xe0, (u8[]) { 0 }, 1);
}

static int fl7707n_jd9365_timing_status(struct fl7707n *ctx, const char *stage)
{
	u8 tcon[6] = { 0 }, power[8] = { 0 }, ctrl[3] = { 0 };
	static const u8 ctrl_regs[] = { 0x24, 0x37, 0x54 };
	unsigned int i;
	int ret;

	ret = mipi_dsi_dcs_write(ctx->dsi, 0xe0, (u8[]) { 1 }, 1);
	if (ret < 0)
		return ret;
	for (i = 0; i < sizeof(tcon); i++) {
		ret = mipi_dsi_dcs_read(ctx->dsi, 0x40 + i, &tcon[i], 1);
		if (ret != 1)
			return ret < 0 ? ret : -EIO;
	}
	for (i = 0; i < sizeof(power); i++) {
		ret = mipi_dsi_dcs_read(ctx->dsi, 0x55 + i, &power[i], 1);
		if (ret != 1)
			return ret < 0 ? ret : -EIO;
	}
	for (i = 0; i < ARRAY_SIZE(ctrl_regs); i++) {
		ret = mipi_dsi_dcs_read(ctx->dsi, ctrl_regs[i], &ctrl[i], 1);
		if (ret != 1)
			return ret < 0 ? ret : -EIO;
	}
	dev_info(&ctx->dsi->dev, "JD时序%s：P1 40..45=%*ph，55..5C=%*ph，24/37/54=%*ph，扫描行数=%u\n",
		 stage, (int)sizeof(tcon), tcon, (int)sizeof(power), power,
		 (int)sizeof(ctrl), ctrl, ((tcon[1] << 2) | ((tcon[0] >> 4) & 3)) * 2);
	return mipi_dsi_dcs_write(ctx->dsi, 0xe0, (u8[]) { 0 }, 1);
}

static int fl7707n_start_jd9365(struct fl7707n *ctx)
{
	static const u8 cmds[][2] = {
		{ 0x80, 0x01 }, { 0x3a, 0x77 }, { 0x53, 0x20 },
		{ 0x55, 0x00 }, { 0x51, 0x80 },
	};
	static const u8 regs[] = { 0x80, 0x0a, 0x0c, 0x54, 0x52 };
	unsigned int i;
	int ret;

	ret = fl7707n_jd9365_timing_status(ctx, "复位后");
	if (ret < 0)
		return ret;
	if (jd9365_tcon_720 || jd9365_power_nt) {
		u8 value = 0;

		ret = mipi_dsi_dcs_write(ctx->dsi, 0xe0, (u8[]) { 1 }, 1);
		if (ret < 0)
			return ret;
		if (jd9365_tcon_720) {
			ret = mipi_dsi_dcs_read(ctx->dsi, 0x40, &value, 1);
			if (ret != 1)
				return ret < 0 ? ret : -EIO;
			/* LN=360对应720行；保留IP750相关位，仅设置LN低两位和RSO。 */
			value = (value & ~0x37) | 0x04;
			ret = mipi_dsi_dcs_write(ctx->dsi, 0x40, &value, 1);
			if (ret < 0)
				return ret;
			ret = mipi_dsi_dcs_write(ctx->dsi, 0x41, (u8[]) { 0x5a }, 1);
			if (ret < 0)
				return ret;
		}
		if (jd9365_power_nt) {
			ret = mipi_dsi_dcs_read(ctx->dsi, 0x55, &value, 1);
			if (ret != 1)
				return ret < 0 ? ret : -EIO;
			value = (value & 0xf0) | 2;
			ret = mipi_dsi_dcs_write(ctx->dsi, 0x55, &value, 1);
			if (ret < 0)
				return ret;
		}
		ret = mipi_dsi_dcs_write(ctx->dsi, 0xe0, (u8[]) { 0 }, 1);
		if (ret < 0)
			return ret;
	}

	/* 已验证 JD 分页后仅设置接口和标准控制，保留面板自身的电源/GIP/OTP 参数。 */
	for (i = 0; i < ARRAY_SIZE(cmds); i++) {
		ret = mipi_dsi_dcs_write_buffer(ctx->dsi, cmds[i], sizeof(cmds[i]));
		if (ret < 0)
			return ret;
	}
	ret = mipi_dsi_dcs_exit_sleep_mode(ctx->dsi);
	if (ret < 0)
		return ret;
	msleep(250);
	ret = fl7707n_jd9365_timing_status(ctx, "Sleep Out后");
	if (ret < 0)
		return ret;
	for (i = 0; i < ARRAY_SIZE(regs); i++) {
		u8 value = 0;

		ret = mipi_dsi_dcs_read(ctx->dsi, regs[i], &value, 1);
		dev_info(&ctx->dsi->dev, "JD最小启动：reg=%02x ret=%d value=%02x\n",
			 regs[i], ret, value);
	}
	return 0;
}

static int fl7707n_prepare(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);
	struct device *dev = &ctx->dsi->dev;
	unsigned int i;
	ssize_t ret;
	u8 status = 0;
	u8 id[3] = { 0 };
	unsigned int meter_ms = min(meter_reset_ms, 30000u);
	u32 saved_lpclk = 0, phy;

	fl7707n_host_status(ctx, "复位前主机");
	if (meter_ms && ctx->rp1_regs) {
		/* 仅测量模式强制撤销 HS 时钟请求；普通启动依靠非连续时钟配置。 */
		saved_lpclk = readl(ctx->rp1_regs + 0x94);
		writel(0, ctx->rp1_regs + 0x94);
		ret = readl_poll_timeout(ctx->rp1_regs + 0xb0, phy,
					(phy & 0x94) == 0x94, 1000, 100000);
		dev_info(dev, "万用表测量：CLK/D0/D1 Stop=%s，PHY=%#x\n",
			 ret ? "未全部进入" : "已进入", phy);
	}

	/* 转接板常供电；CAM_GPIO 经 1.8 V 缓冲只控制复位。 */
	gpiod_set_value_cansleep(ctx->reset, 1);
	if (meter_ms) {
		dev_info(dev, "万用表测量：RST 低 %u ms\n", meter_ms);
		msleep(meter_ms);
		gpiod_set_value_cansleep(ctx->reset, 0);
		dev_info(dev, "万用表测量：RST 高 %u ms\n", meter_ms);
		msleep(meter_ms);
		gpiod_set_value_cansleep(ctx->reset, 1);
		dev_info(dev, "万用表测量：RST 再低 %u ms\n", meter_ms);
		msleep(meter_ms);
	}
	msleep(20);
	gpiod_set_value_cansleep(ctx->reset, 0);
	msleep(120);
	fl7707n_host_status(ctx, "复位释放后主机");
	/* Sleep In 也允许读 0A，先区分通信本身失败还是厂家表执行后失效。 */
	ret = mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
	if (ret < 0)
		dev_warn(dev, "初始化前设置回包长度失败: %zd\n", ret);
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x0a, &status, 1);
	dev_info(dev, "初始化前电源状态回读: ret=%zd value=0x%02x\n", ret, status);
	fl7707n_host_status(ctx, "初始化前读0A后");
	status = 0;
	if (jd9365_probe) {
		ret = fl7707n_identify_jd9365(ctx);
		if (ret >= 0 && jd9365_start)
			ret = fl7707n_start_jd9365(ctx);
		if (ret < 0 || !jd9365_start)
			gpiod_set_value_cansleep(ctx->reset, 1);
		dev_info(dev, "JD识别/启动结束：ret=%zd start=%d，跳过 FL 初始化\n", ret, jd9365_start);
		if (meter_ms && ctx->rp1_regs)
			writel(saved_lpclk, ctx->rp1_regs + 0x94);
		return ret < 0 ? ret : 0;
	}
	for (i = 0; i < ARRAY_SIZE(fl7707n_init); i++) {
		const struct fl7707n_cmd *cmd = &fl7707n_init[i];
		u8 data[64];
		size_t len = cmd->len;

		if ((backlight_only || video_before_on) &&
		    cmd->data[0] == MIPI_DCS_SET_DISPLAY_ON)
			continue;
		if (skip_ef && cmd->data[0] == 0xef)
			continue;

		memcpy(data, cmd->data, cmd->len);
		/* 仅切换同步源，保持电源类型、倍率、分频和软启动参数不变。 */
		if (internal_sync && data[0] == 0xb8)
			data[4] &= ~(BIT(1) | BIT(0));
		/* 扩展寄存器长包留在 Command Mode；53 的 BCTRL/BL 仍保持复位关闭值。 */
		if (data[0] == 0xc7)
			data[1] |= BIT(7);
		/* SY7200A 推荐 PWM 不低于 20 kHz，厂家表的约 1 kHz 不适用。 */
		if (data[0] == 0xc8) {
			data[1] = 0x00;
			data[2] = 0x03;
		}
		/* 仅在独立 PWM 实验中对照公开的两参数格式，保留普通模式厂家原表。 */
		if (backlight_only && (data[0] == 0xc7 || data[0] == 0xc8))
			len = 3;
		if (vendor_generic && data[0] >= 0xb0)
			ret = mipi_dsi_generic_write(ctx->dsi, data, len);
		else
			ret = mipi_dsi_dcs_write_buffer(ctx->dsi, data, len);
		if (ret < 0) {
			dev_err(dev, "初始化命令 0x%02x 发送失败: %zd\n", cmd->data[0], ret);
			goto prepare_failed;
		}
		if (data[0] == 0xba || data[0] == 0xc7) {
			u8 value = 0;

			/* 在扩展命令仍解锁时验证关键设置，避免只在 EF 之后检查。 */
			mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
			if (vendor_generic)
				ret = mipi_dsi_generic_read(ctx->dsi, data, 1, &value, 1);
			else
				ret = mipi_dsi_dcs_read(ctx->dsi, data[0], &value, 1);
			dev_info(dev, "扩展寄存器 %02x: 写入首参=%02x 回读ret=%zd value=%02x\n",
				 data[0], data[1], ret, value);
		}
		/* 厂家表仅等待 50 ms；现场时序要求 Display On 后超过 120 ms。 */
		if (data[0] == MIPI_DCS_SET_DISPLAY_ON)
			msleep(150);
		else if (cmd->delay_ms)
			msleep(cmd->delay_ms);
	}
	if (backlight_only) {
		/* Sleep Out 会触发 OTP 装载；醒来后重新解锁并重写完整 IO/PWM 配置作对照。 */
		ret = mipi_dsi_dcs_write(ctx->dsi, 0xb9, (u8[]) { 0xf1, 0x12, 0x87 }, 3);
		if (ret < 0)
			goto prepare_failed;
		for (i = 0; i < ARRAY_SIZE(fl7707n_init); i++) {
			const struct fl7707n_cmd *cmd = &fl7707n_init[i];
			u8 data[64], value = 0;

			if (cmd->data[0] != 0xc7 && cmd->data[0] != 0xc8)
				continue;
			memcpy(data, cmd->data, cmd->len);
			if (data[0] == 0xc7)
				data[1] |= BIT(7);
			else {
				data[1] = 0x00;
				data[2] = 0x03;
			}
			ret = mipi_dsi_dcs_write_buffer(ctx->dsi, data, cmd->len);
			if (ret < 0)
				goto prepare_failed;
			mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
			ret = mipi_dsi_dcs_read(ctx->dsi, data[0], &value, 1);
			dev_info(dev, "Sleep Out 后重写 %02x: 回读ret=%zd value=%02x\n",
				 data[0], ret, value);
		}
		/* PWM 独立于内容自适应调光，避免黑色像素内容干扰背光实验。 */
		ret = mipi_dsi_dcs_write(ctx->dsi, 0x55, (u8[]) { 0x00 }, 1);
		if (ret < 0)
			goto prepare_failed;
		ret = mipi_dsi_dcs_set_display_off(ctx->dsi);
		if (ret < 0)
			goto prepare_failed;
		dev_info(dev, "独立背光测试：保留 Sleep Out，保持 Display Off，不发送 Display On\n");
	}
	/* 先允许亮度寄存器生效，BL 仍关闭；视频稳定后才打开 BL。 */
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x20 }, 1);
	if (ret < 0)
		goto prepare_failed;
	if (brightness_probe) {
		/* 在纯 Command Mode 对照长短包，避免视频消隐窗口干扰诊断。 */
		ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x24 }, 1);
		if (ret < 0)
			goto prepare_failed;
		for (i = 0; i < 3; i++) {
			u8 value = 0;
			u8 payload[] = { 0x51, i == 1 ? 0x6a : 0x35, 0x35 };

			ret = mipi_dsi_dcs_write_buffer(ctx->dsi, payload, i == 1 ? 2 : 3);
			if (ret < 0)
				goto prepare_failed;
			msleep(5);
			mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
			ret = mipi_dsi_dcs_read(ctx->dsi, 0x52, &value, 1);
			dev_info(dev, "Command Mode 51验证：参数数=%u 写入=%02x 回读ret=%zd value=%02x\n",
				 i == 1 ? 1 : 2, payload[1], ret, value);
		}
		ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x20 }, 1);
		if (ret < 0)
			goto prepare_failed;
	}
	/* 亮度长包也在 Command Mode 写入，Video Mode 仅发送短包开关背光。 */
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x51, (u8[]) { 0x80, 0x80 },
				 brightness_bytes == 1 ? 1 : 2);
	if (ret < 0)
		goto prepare_failed;
	/* 写入成功只证明主机发送完成，回读用于区分面板实际应答。 */
	ret = mipi_dsi_set_maximum_return_packet_size(ctx->dsi, sizeof(id));
	if (ret < 0)
		dev_warn(dev, "设置回读包长度失败: %zd\n", ret);
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x04, id, sizeof(id));
	dev_info(dev, "面板 ID 回读: ret=%zd value=%02x %02x %02x\n", ret, id[0], id[1], id[2]);
	fl7707n_host_status(ctx, "初始化后读ID后");
	mipi_dsi_set_maximum_return_packet_size(ctx->dsi, 1);
	ret = mipi_dsi_dcs_read(ctx->dsi, 0x0a, &status, 1);
	dev_info(dev, "初始化完成（%s），电源状态回读: ret=%zd value=0x%02x\n",
		 backlight_only ? "Display Off 独立 PWM 测试" :
		 video_before_on ? "等待视频阶段发送 Display On" : "Display On 后等待 150 ms",
		 ret, status);
	fl7707n_host_status(ctx, "初始化后读0A后");
	if (meter_ms && ctx->rp1_regs)
		writel(saved_lpclk, ctx->rp1_regs + 0x94);
	return 0;

prepare_failed:
	gpiod_set_value_cansleep(ctx->reset, 1);
	if (meter_ms && ctx->rp1_regs)
		writel(saved_lpclk, ctx->rp1_regs + 0x94);
	return ret;
}

static int fl7707n_enable(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);
	int ret;
	unsigned int i;
	static const u8 commands[] = { 0x0a, 0x54, 0x52, 0xc7, 0x05, 0x0e, 0x0f };

	if (jd9365_probe && !jd9365_start)
		return 0;

	/* 先给外部 Hsync 同步电源留出稳定时间，期间不插入 BTA 回读。 */
	msleep(video_before_on || jd9365_probe ? 120 : 50);
	if ((video_before_on || jd9365_probe) && !backlight_only) {
		ret = mipi_dsi_dcs_set_display_on(ctx->dsi);
		if (ret < 0)
			goto backlight_failed;
		msleep(150);
	}
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x24 }, 1);
	if (ret < 0)
		goto backlight_failed;
	dev_info(&ctx->dsi->dev, "视频后背光命令已发送：video_before_on=%d skip_ef=%d backlight_only=%d internal_sync=%d\n",
		 video_before_on, skip_ef, backlight_only, internal_sync);
	msleep(50);
	fl7707n_host_status(ctx, "背光短包后");
	/* 回读标准亮度和控制值，区分命令已发出与寄存器确实发生变化。 */
	for (i = 0; i < ARRAY_SIZE(commands); i++) {
		u8 value[2] = { 0 };
		size_t len = commands[i] == 0x52 || commands[i] == 0xc7 ? 2 : 1;

		mipi_dsi_set_maximum_return_packet_size(ctx->dsi, len);
		ret = mipi_dsi_dcs_read(ctx->dsi, commands[i], value, len);
		dev_info(&ctx->dsi->dev, "视频后回读 %02x: ret=%d data=%02x %02x（51 参数数=%u）\n",
			 commands[i], ret, value[0], value[1], brightness_bytes);
		fl7707n_host_status(ctx, "视频后读寄存器");
	}
	return 0;

backlight_failed:
	dev_err(&ctx->dsi->dev, "背光控制命令发送失败: %d\n", ret);
	return ret;
}

static int fl7707n_disable(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);
	int ret;

	if (jd9365_probe && !jd9365_start)
		return 0;

	/* 仅需标准短包先关背光，再由 DRM 停视频、执行 Display Off 和 Sleep In。 */
	ret = mipi_dsi_dcs_write(ctx->dsi, 0x53, (u8[]) { 0x00 }, 1);
	return ret < 0 ? ret : 0;
}

static int fl7707n_unprepare(struct drm_panel *panel)
{
	struct fl7707n *ctx = to_fl7707n(panel);

	if (jd9365_probe && !jd9365_start)
		return 0;

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
			  MIPI_DSI_MODE_LPM | MIPI_DSI_CLOCK_NON_CONTINUOUS |
			  MIPI_DSI_MODE_NO_EOT_PACKET;
	if (of_device_is_compatible(dsi->host->dev->of_node, "raspberrypi,rp1dsi")) {
		struct resource *res = platform_get_resource(to_platform_device(dsi->host->dev),
							 IORESOURCE_MEM, 1);

		if (!res)
			return -ENODEV;
		ctx->rp1_regs = devm_ioremap(&dsi->dev, res->start, resource_size(res));
		if (!ctx->rp1_regs)
			return -ENOMEM;
	}
	drm_panel_init(&ctx->panel, &dsi->dev, &fl7707n_funcs, DRM_MODE_CONNECTOR_DSI);
	/* 此标志只保证主机先初始化；时钟能否回到 LP-11 还取决于时钟模式。 */
	ctx->panel.prepare_prev_first = true;
	drm_panel_add(&ctx->panel);
	ret = mipi_dsi_attach(dsi);
	if (ret)
		drm_panel_remove(&ctx->panel);
	else
		dev_info(&dsi->dev, "%s 720x720 双通道实验面板已注册\n",
			 jd9365_probe ? "JD9365" : "FL7707N");
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
