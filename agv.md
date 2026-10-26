<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769598360856-cb3e72b8-e245-4866-99e3-5f31bf5bd91a.png" width="674" alt="" title="" crop="0,0,1,1" id="u07af4a29" class="ne-image">

**<font style="color:rgb(0, 0, 0);">RANGER AIR </font>**

**AgileX Robotics Team**

**用户手册   V1.0.0 2026.01**

## **文档版本**
| 编号 | 版本 | 日期 | 修改人 | 对应固件版本 | 备注 |
| :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | V1.0.0 | 2026/1/25 | 何士玉 | | 初稿 |


---

在机器人第一次通电前，任何个人或者机构在使用设备之前必须阅读并理解这些信息。有任何相关使用的疑问都可以联系我们support@agilex.ai，必须遵守并执行本手册其他章节中的所有组装说明和指南。应特别注意与警告标志相关的文本。

---

**目录**

[文档版本1](#4ever-bi-lm26y6s1oxq1s9l6bi)

[重要安全信息1](#4ever-bi-ldseu337idp67qbuxlo)

[有效性和责任1](#4ever-bi-ldseu3376q5r6te0rkj)

[环境1](#4ever-bi-ldseu337qz279lapcfr)

[检查1](#4ever-bi-ldseu3373mth0jltn8d)

[注意事项1](#4ever-bi-ldseu337o7y9gxmjmf)

[操作注意事项1](#4ever-bi-llqfdsdvzkkg57ht419)

[电池注意事项1](#4ever-bi-lfhocxdfsymy5isv8i)

[使用环境注意事项1](#4ever-bi-lfhol8gledy73p4dlsh)

[安全注意事项1](#4ever-bi-lfhorhlqz3wkm84qok7)

[1.  RANGER AIR  简介introduction1](#4ever-bi-ldsezktygidaqlv0kvw)

[1.1 产品列表1](#4ever-bi-ldsh7tp3fbg3144csi)

[1.2性能参数1](#4ever-bi-ldsezkty3v5kt6pxzw4)

[1.3开发所需1](#4ever-bi-ldsezktzexrmgh9vcdt)

[2 .基本介绍1](#4ever-bi-ldsezktzrm8zczzrczi)

[2.1状态指示1](#4ever-bi-ldsezktzskgo2oarocl)

[2.2.电气接口说明1](#4ever-bi-ldsezktz0t7jai0nwat)

[2.3.遥控说明1](#4ever-bi-ldsezktzv0pe2zkt7e)

[3  使用和开发1](#4ever-bi-ldsezktzrozl6a85cn)

[3.1 使用与操作1](#4ever-bi-ldsezktzef5hd08jxi)

[3.2 CAN接口协议1](#4ever-bi-ldsezktzor3bynmycqo)

[3.3 RANGER AIR  ROS使用说明1](#4ever-bi-lfhtv5ki7ccbji1qacs)

[3.5  车体坐标系说明1](#4ever-bi-lfhtv5ngjzexxsye0r)

[3.6 固件升级1](#4ever-bi-ldsktiay039hsw8weldk)

[4、产品尺寸1](#4ever-bi-ldsktib4jd6vrexncm)

# **重要安全信息**
<font style="color:rgb(0, 0, 0);">本手册中的信息不包含设计、安装和操作一个完整的机器人应用，也不包含所有可能对这一完整的系统的安全造成影响的周边设备。该完整的系统的设计和使用需符合该机器人安装所在国的标准和规范中确立的安全要求。RANGER AIR  的集成商和终端客户有责任确保遵循相关国家的切实可行的法律法规，确保完整的机器人应用实例中不存在任何重大危险。这包括但不限于以下内容： </font>

## **<font style="color:#404040;">有效性和责任</font>**
+ <font style="color:rgb(0, 0, 0);">对完整的机器人系统做一个风险评估</font>
+ <font style="color:rgb(0, 0, 0);">将风险评估定义的其他机械的附加安全设备连接在一起</font>
+ <font style="color:rgb(0, 0, 0);">确认整个机器人系统的外围设备包括软件和硬件系统的设计和安装准确无误</font>
+ <font style="color:rgb(0, 0, 0);">本机器人不具备一个完整的自主移动机器人具备的自动防撞、防跌落、生物接近预警等相关安全功能但不局限于上述描述，相关功能需要集成商和终端客户遵循相关规定和切实可行的法律法规进行安全评估，确保开发完成的机器人在实际应用中不存在任何重大危险和安全隐患</font>
+ <font style="color:rgb(0, 0, 0);">收集技术文件中的所有的文档：包括风险评估和本手册</font>
+ <font style="color:rgb(0, 0, 0);">在操作和使用设备之前已经知晓可能存在的安全风险</font>

## **环境**
+ <font style="color:rgb(0, 0, 0);">首次使用，请先仔细阅读本手册，了解基本操作内容与操作规范</font>
+ <font style="color:rgb(0, 0, 0);">遥控操作，选择相对空旷区域使用，车上本身是不带任何自动避障传感器</font>
+ <font style="color:rgb(0, 0, 0);">在-10 ℃~40℃的环境温度中使用</font>
+ <font style="color:rgb(0, 0, 0);">如果车辆非单独定制IP防护等级，车辆防水、防尘能力为IP22</font>

## **检查**
+ 确保各设备的电量充足
+ 确保车辆无明显异常
+ 检查遥控器的电池电量是否充足

# **注意事项**
## **操作注意事项**
+ 保证操作时周围区域相对空旷
+ 在视距内遥控控制
+ RANGER AIR  最大的载重为 80KG，在使用时，确保有效载荷不超过 80KG
+ RANGER AIR  安装外部扩展时，确认扩展的质心位置，确保在旋转中心
+ 当设备低电量报警时请及时充电
+ 当设备出现异常时，请立即停止使用，避免造成二次伤害
+ 当设备出现异常时，请联系相关技术人员，请勿擅自处理
+ 请根据设备的IP防护等级在满足防护等级要求的环境中使用
+ 请勿直接推车 
+ 尾部扩展电源电流不超过 25A,总功率不超过 600w

## **电池注意事项 **
+ RANGER AIR产品出厂时电池并不是满电状态的，具体电池电压和电量可以通过 RANGER AIR遥控器上 vol 和 batt 显示
+ 请不要在电池使用殆尽后再进行充电，在 RANGERMINI 遥控低电量低于15%的情况下请及时充电
+ 静态存放条件：存储的最佳温度为-10℃~40℃，电池在不使用的情况下存放，必须是1个月左右充放电一次，然后使电池处于满电压状态进行存放，请勿将电池放入火中，或对电池加热，请勿在高温下存储电池
+ 充电：必须使用配套的锂电池专用充电器进行充电，请勿在0℃以下给电池充电，请勿使用非原厂标配的电池、电源、充电器

## **使用环境注意事项**
+ RANGER AIR工作温度为-10℃~40℃，请勿在温度低于 -10℃、高于40℃环境中使用
+ 请勿在存在腐蚀性、易燃性气体的环境或者靠近可燃性物质的环境中使用
+ 请不要在加热器或者大型卷线电阻等发热体周围使用
+ RANGER AIR 防水防尘等级为IP22
+ 建议使用环境海拔高度不超过1000M
+ 建议使用环境昼夜温差不超过25℃

## **安全注意事项**
+ 使用过程有疑问，请按照相关说明手册进行或者咨询相关技术人员
+ 使用设备操作前，注意现场情况，避免误操作导致人员安全问题发生
+ 遇到紧急情况，通过拍停急停按钮，断电设备
+ 请勿未经技术支持和允许，私自改装内部设备结构
+ 当设备出现异常时，请立即停止使用，避免造成二次伤害
+ 当设备出现异常时，请联系相关技术人员，请勿擅自处理。

## <font style="color:rgb(0, 0, 0);">RANGER AIR  简介introduction</font>
<font style="color:rgb(0, 0, 0);">RANGER AIR是一款可编程全向型UGV（UNMANNED GROUND VEHICLE），它是一款采用阿克曼转向设计的底盘。相对于四轮差速底盘，RANGER AIR在普通水泥、柏油路上优势明显，不仅运动速度和载重能力能够更高，还减小了对结构和轮胎的磨损，而且更加稳定和安全。相较于前阿克曼模型的底盘，RANGER AIR不仅缩小了转弯半径，还可以0角度转向。RANGER AIR集差速底盘与阿克曼底盘的优点于一身，适用于多种复杂地形。同时可以搭载立体相机、激光雷达、GPS、IMU、机械手等设备，被运用到无人巡检、安防、科研、勘探物流等领域。</font>

### **<font style="color:rgb(0, 0, 0);">1.1 产品列表</font>**
| 名称 | 数量 |
| :---: | :---: |
| <font style="color:rgb(0, 0, 0);">RANGER AIR  机器人</font> | ×1 |
| <font style="color:rgb(0, 0, 0);">电池充电器(AC 220V) </font> | ×1 |
| <font style="color:rgb(0, 0, 0);">航空插头公头（4Pin）</font> | ×1 |
| <font style="color:rgb(0, 0, 0);">FS遥控器(选配)</font> | ×1 |
| <font style="color:rgb(0, 0, 0);">USB转CAN通讯模块</font> | ×1 |


### **1.2性能参数**
| 参数类型  | 项目 | 指标 |
| :---: | :---: | :---: |
| 机械参数 | 长×宽×高（mm） | <font style="color:rgb(0, 0, 0);">552×500×250</font> |
| | 轴距（mm） | 388 |
| | <font style="color:rgb(0, 0, 0);">前 / 后轮距（mm）</font> | 338 |
| | <font style="color:rgb(0, 0, 0);">整备重量（Kg）</font> | <font style="color:rgb(0, 0, 0);">50-55kg</font> |
| | <font style="color:rgb(0, 0, 0);">电池类型</font> | <font style="color:rgb(0, 0, 0);">磷酸铁锂</font> |
| | <font style="color:rgb(0, 0, 0);">电池参数</font> | <font style="color:rgb(0, 0, 0);">24V30AH</font> |
| | <font style="color:rgb(0, 0, 0);">动力驱动电机</font> | <font style="color:rgb(0, 0, 0);">150W×4</font> |
| | <font style="color:rgb(0, 0, 0);">转向驱动电机 </font> | <font style="color:rgb(0, 0, 0);">50W×4</font> |
| | <font style="color:rgb(0, 0, 0);">驻车形式</font> | <font style="color:rgb(0, 0, 0);">电子刹车</font> |
| | <font style="color:rgb(0, 0, 0);">转向形式</font> | <font style="color:rgb(0, 0, 0);">四轮四转</font> |
| | <font style="color:rgb(0, 0, 0);">悬挂形式</font> | <font style="color:rgb(0, 0, 0);">后摇摆臂悬挂</font> |
| | 转向电机减速比 | 1:51 |
| | 转向电机编码器 | 双编 |
| | 驱动电机减速比 | 1:4.428 |
| | 驱动电机传感器 | 编码器 |
| 性能参数<br/> | 防护等级 | IP22 |
| | 最高速度（km/h) | 5.4 |
| | 最大爬坡能力（°） | <font style="color:rgb(0, 0, 0);">8 °</font> |
| | <font style="color:rgb(0, 0, 0);">离地间隙（mm</font>） | 39 |
| | <font style="color:rgb(0, 0, 0);">最大续航时间（h）</font> | 6.5（空载）、2.5（满载） |
| | <font style="color:rgb(0, 0, 0);">最大行程(km)</font> | 45(空载）、25（满载） |
| | <font style="color:rgb(0, 0, 0);">充电时间（h)</font> | 3 |
| | <font style="color:rgb(0, 0, 0);">工作温度（℃） </font> | <font style="color:rgb(0, 0, 0);">-10~40</font> |
| | <font style="color:rgb(0, 0, 0);">对外供电</font> | <font style="color:rgb(0, 0, 0);">25A</font> |
| 控制参数 | 控制模式 | 遥控控制<br/>指令控制模式 |
| | 遥控器 | 2.4G/ 遥控距离50M |
| | 通讯接口 | CAN |


### **<font style="color:rgb(35, 31, 32);">1.3开发所需</font>**
<font style="color:rgb(0, 0, 0);">RANGER AIR  出厂时可选配FS遥控器，用户可以通过遥控器控制四轮四转底盘，完成模式切换、移动和转向控制操作；RANGER AIR 2配备了标准CAN通信接口，用户可以通过CAN接口进行二次开发。</font>

## **<font style="color:rgb(35, 31, 32);">2 .基本介绍 </font>**
<font style="color:rgb(0, 0, 0);">本部分内容将会对RANGER AIR  移动机器人底盘作一个基本的介绍， 便于用户和开发者对于RANGER AIR  底盘有一个基本的认识。 如下图2.1所示，为整个移动机器人底盘的概览视图。</font>

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769482461283-101d38df-fd30-4353-88c6-403d2fbdf81b.png" width="1129" alt="" title="" crop="0,0,1,1" id="u931042a5" class="ne-image">

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769483882378-36aee0a2-4f5e-4fda-9b45-d7019a46cd17.png" width="1053" alt="" title="" crop="0,0,1,1" id="u29bcf5f4" class="ne-image">

图2.1尾部概览视图

<font style="color:rgb(0, 0, 0);">RANGER AIR  整体上采用了模块化和智能化的设计理念，在动力模块上采用了实心胎与摇摆臂的复合设计，再加上动力强劲的轮毂电机，使得RANGER AIR  机器人底盘具有很强的通过性和地面适应性，可在不同的地面上灵活运动。轮毂电机的加持省去了复杂的传动设计结构，让RANGER AIR  更小巧、更飘逸。在RANGER AIR  的尾部和顶部配置了开放的电气接口和通讯接口，方便客户进行二次开发， 在RANGER AIR 顶部预留了固定孔位，方便用户搭载外部设备扩展使用（重心位置参考图 2.2 重心位置图示）。</font>

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769137177153-0b54d692-9694-46fe-926c-f213ad03d79b.png" width="731" alt="" title="" crop="0,0,1,1" id="uc7265bcc" class="ne-image">

图2.2 RANGERMINI 重心位置图示

### **<font style="color:rgb(35, 31, 32);">2.1状态指示</font>**
<font style="color:rgb(0, 0, 0);">用户可以通过底盘的遥控器显示屏以及CAN报文来确定车体的状态。具体参数考表2.1。</font>

| 状态 | 描述 |
| :---: | --- |
| 当前电压 | <font style="color:rgb(0, 0, 0);">当前电池电压和电量信息可通过遥控器显示器 vol 和 batt 显示</font> |
| <font style="color:#404040;">低电压报警</font> | <font style="color:rgb(0, 0, 0);">当电池BMS 的 SOC低于15%，车体会通过全部红灯闪烁进行提示。当检测到电池BMS 的 SOC低于10%时，RANGER AIR  底盘为了防止电池损坏，会主动切断外部扩展供电和驱动器供电，此时底盘将无法进行运动控制和接受外部指令控制。 </font> |
| 详细状态信息 | 通过CAN报文查看 |


表格 2.1 车体状态说明表

灯光指示灯说明：

| 灯光状态 | 状态说明 |
| --- | --- |
| 全部显示红灯长亮 | 出现报错  |
| 全部红灯闪烁 | 底盘电量信息低于15%警告 |
| 全部显示绿灯呼吸灯 | 充电电流大于1A |
| 全部显示黄灯 | 驻车 |
| 全部显示蓝灯呼吸灯 | 待命状态 或 直线运行 |
| 全部不显示 | 灯光关闭 |


运动模式下对应灯效说明如下：

| 运动模式 | 灯光显示 |
| --- | --- |
| 直线运行 | 全部显示蓝灯呼吸灯 |
| 阿克曼模式运行 | 左转时左侧两个灯由内向外流水效果，右侧蓝色呼吸灯。右转时右侧两个灯流水效果。 |
| 自转模式运行 | 左转时四个灯条向左侧流水效果，右转时四个灯条向右侧流水效果。 |
| 斜行模式运行 | 左对角斜行时，左上角和右下角灯条显示黄色，右对角斜行时，右上角和左下角灯条显示黄色。  |
| 横行模式运行（斜行角度90时） | 向左运行时，左侧两个灯灯条显示黄色，向右运行时，右侧两个灯灯条显示黄色 |


### **2.2.电气接口说明**
<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769169408375-9fd58daf-2187-453a-a319-41ee5383f432.png" width="967" alt="" title="" crop="0,0,1,1" id="ud1357bbc" class="ne-image">

图2.2尾部视图

<font style="color:rgb(0, 0, 0);">RANGER AIR  底盘尾部配置有一个航空扩展接口，航空扩展接口配置了一组电源以及一组CAN通讯接口。便于使用者给扩展设备提供电源，以及通讯使用。其具体引脚定义如下图所示。需要注意的是，这里的扩展电源受内部控制，当电池电压低于安全电压会主动切断供电，所以用户需要注意，在达到临界电压前RANGER AIR  底盘平台会通过灯光闪烁通知，用户在使用过程中注意充电。</font>

### **2.3.遥控说明**
<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1745825997014-07184b9f-d8cd-4875-90ef-1f300cb2b132.png?x-oss-process=image%2Fcrop%2Cx_106%2Cy_24%2Cw_586%2Ch_658" width="586" alt="" title="" crop="0.1532,0.0352,1,1" id="vzN40" class="ne-image">

<font style="color:#404040;">图2.4遥控器示意图</font>

如上图，按键的功能定义为：SWB为控制模式选择拨杆，拨至最上方为指令控制模式，拨至中间或下方为遥控控制模式；SWA为灯光控制开关，拨到下方为关闭灯光（需SWB先进入遥控模式）；SWC拨到下方为驻车模式，此时四轮四转向呈X形锁止；SWD为底盘运动模式设置开关：

SWD拨到上为①前后阿克曼和②自旋模式  ①左摇杆控制速度，右摇杆控制转角；②左摇杆不动，右摇杆左右方向控制自旋；

SWD拨到下为斜移模式：左摇杆控制速度，右摇杆控制转角（最大角度90°即为横移）；

**零点校准：**

首先将 SWA 拨杆打到最下面;SWB 打到最上面;SWD 打到最下面;

左摇杆四个方位代表指定对应四个位置的转向电机（例如：右上指右上角转向电机，左上指的是左上角转向电机），右摇杆左右方向调节转角，左滚轮可两个方向位置累加调节SWC为调节灵敏度:DOWN→粗调MID→中间挡 UP→微调，Key1设定当前位置为零点。

任何情况下按下KEY1 = 强制清除底盘所有错误 **注意！！仅在确保安全的特殊情况下可使用**

**注意：进行零点校准时，****<font style="color:#DF2A3F;">请注意不要断开小车的电源</font>****，否则需要拆开小车重新校准才能恢复。**

****

POWER为电源按钮，同时按住即可开机 。

**遥控控制基本操作流程：**

**启动之前需要确保 RANGERMINI 的轮子和底盘平行朝着正前方**，正常启动RANGERMINI  移动机器人底盘后，启动遥控器，将SWB切换为遥控控制模式，即可通过遥控器控制RANGERMINI平台运动。

**遥控器更换电池说明**：

富斯遥控器采用 5 号电池作为能源供应，当遥控器显示界面 Remoter 比较低的时候，说明此时遥控器电池电量过低，此时需要打开遥控器背面电池盖板，更换电池。

## **<font style="color:rgb(35, 31, 32);">3 使用和开发</font>**
本部分主要介绍RANGER AIR  平台的基本操作与使用，介绍如何通过外部CAN口，通过CAN总线协议来对RANGER AIR  进行二次开发。

### **3.1 使用与操作**
**检查**

<font style="color:rgb(0, 0, 0);background-color:#ffffff;">检查RANGER AIR  状态。 检查RANGER AIR  是否有明显异常；如有，请联系售后支持； 初次使用时确认尾部电气面板中急停开关是否被按下，如按下，顺时针旋转则处于释放状态。</font>

**<font style="color:rgb(0, 0, 0);background-color:#ffffff;">特别注意：当急停按钮释放后，遥控模式下需要手动按 key1 清除急停错误，指令模式下需要通过清除错误指令清除急停错误。</font>**

**<font style="color:rgb(0, 0, 0);background-color:#ffffff;">启动和关机：</font>**

<font style="color:rgb(0, 0, 0);background-color:#ffffff;">尾部 按钮为开关按钮，按下之后即打开电源，小车上电。</font>

<font style="color:rgb(0, 0, 0);background-color:#ffffff;"></font>**<font style="color:rgb(0, 0, 0);background-color:#ffffff;">充电：</font>**

<font style="color:rgb(0, 0, 0);background-color:#ffffff;">检查电池电压，正常电压范围为 24-29.4V,如有外圈led红灯表示电池电压过低，请及时充电。本产品默认随车配备一个 10A的充电器，将充电器的插头插入底盘后面充电插口，将充电器连接电源，将充电器上开关打开，即可进入充电状态。</font>

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769579267383-42741ba3-dd34-4215-80a7-0ba3447fe33d.png" width="722" alt="" title="" crop="0,0,1,1" id="uf857f8a1" class="ne-image">

**<font style="color:rgb(0, 0, 0);background-color:#ffffff;">电池更换：</font>**

<font style="color:rgb(0, 0, 0);background-color:#ffffff;">RANGERMINI 自身配备一块24v30ah 的电池，在作业中，当电池电量过低时，我们可以打开右侧面电池面板</font><font style="color:rgb(0, 0, 0);background-color:#ffffff;">快速更换电池。</font>

**<font style="color:rgb(0, 0, 0);background-color:#ffffff;">注意</font>**<font style="color:rgb(0, 0, 0);background-color:#ffffff;">：电池拔出前需要把电池下面固定卡扣往上拨动之后才可以拔出电池。</font>

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769514717282-007f66f8-4374-4414-8f5e-d906da44066d.png" width="884" alt="" title="" crop="0,0,1,1" id="ub3bb6718" class="ne-image">

**CAN线的连接**

<font style="color:rgb(0, 0, 0);">四轮四转底盘随车发货提供了1个航空插头公头，线的定义可参考下图：</font>

<img src="https://cdn.nlark.com/yuque/0/2025/png/29291030/1749720888386-90dfc249-0e37-4919-aee6-8396210d40c9.png?x-oss-process=image%2Fformat%2Cwebp" width="149" alt="" title="" crop="0,0,1,1" id="CDbZ5" class="ne-image">

| <font style="color:rgb(0, 0, 0);">引脚编号</font> | <font style="color:rgb(0, 0, 0);">引脚类型</font> | <font style="color:rgb(0, 0, 0);">功能及定义</font> | <font style="color:rgb(0, 0, 0);">备注</font> |
| :---: | :---: | :---: | :---: |
| <font style="color:rgb(0, 0, 0);">1</font> | <font style="color:rgb(0, 0, 0);">电源</font> | <font style="color:rgb(0, 0, 0);">VCC</font> | <font style="color:rgb(0, 0, 0);">电源正，电压范围</font><br/><font style="color:rgb(0, 0, 0);">22-29.4V,最大电流25A</font> |
| <font style="color:rgb(0, 0, 0);">2</font> | <font style="color:rgb(0, 0, 0);">电源</font> | <font style="color:rgb(0, 0, 0);">GND</font> | <font style="color:rgb(0, 0, 0);">电源负</font> |
| <font style="color:rgb(0, 0, 0);">3</font> | <font style="color:rgb(0, 0, 0);">CAN</font> | <font style="color:rgb(0, 0, 0);">CAN_H</font> | <font style="color:rgb(0, 0, 0);">CAN总线高</font> |
| <font style="color:rgb(0, 0, 0);">4</font> | <font style="color:rgb(0, 0, 0);">CAN</font> | <font style="color:rgb(0, 0, 0);">CAN_L</font> | <font style="color:rgb(0, 0, 0);">CAN总线低</font> |




图3.1 航空插头示意图

**CAN指令控制的实现**

<font style="color:rgb(0, 0, 0);">正常启动RANGER AIR  底盘，此时RANGER AIR  底盘会接收来自CAN接口的指令，同时主机也可以通过CAN总线回馈的实时数据，解析当前底盘的状态，具体协议内容参考CAN通讯协议。（注意：遥控器拥有第一控制权，拥有最高的优先级。默认遥控器不开启底盘上电，底盘处于待机模式。）</font>

### **3.2 CAN接口协议**
<font style="color:rgb(0, 0, 0);">本产品中CAN通信标准采用的是CAN2.0B标准，通讯波特率为500K，报文格式采用MOTOROLA格式。通过外部CAN总线接口可以进行控制模型切换和控制底盘移动的线速度以及转向角；底盘会实时反馈当前的运动状态信息（包括经过整合处理的整机运动信息和各个轮子的详细运动信息）以及系统状态信息（包含自诊断错误码）。</font>

| <font style="color:rgb(0, 0, 0);">指令名称</font> | <font style="color:rgb(0, 0, 0);">系统状态回馈指令</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:rgb(0, 0, 0);">发送节点</font> | <font style="color:rgb(0, 0, 0);">接收节点</font> | <font style="color:rgb(0, 0, 0);">ID</font> | <font style="color:rgb(0, 0, 0);">周期（ms）</font> | <font style="color:rgb(0, 0, 0);">接收超时(ms)</font> |
| <font style="color:rgb(0, 0, 0);">线控底盘</font> | <font style="color:rgb(0, 0, 0);">决策控制单元</font> | <font style="color:rgb(0, 0, 0);">0x211</font> | <font style="color:#404040;">20ms</font> | <font style="color:#222328;">无</font> |
| <font style="color:rgb(0, 0, 0);">数据长度</font> | <font style="color:rgb(0, 0, 0);">0x08</font> | | | |
| <font style="color:rgb(0, 0, 0);">位置</font> | <font style="color:rgb(0, 0, 0);">功能</font> | <font style="color:rgb(0, 0, 0);">数据类型</font> | <font style="color:rgb(0, 0, 0);">说明</font> | |
| <font style="color:rgb(0, 0, 0);">byte [0]</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">当前车体状态</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">0x00 系统正常</font> | |
| | | | <font style="color:rgb(0, 0, 0);">0x02 系统异常</font> | |
| <font style="color:rgb(0, 0, 0);">byte [1]</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">模式控制</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">         0x00 待机模式         </font> | |
| | | | <font style="color:rgb(0, 0, 0);">0x01 CAN指令控制模式</font> | |
| | | | <font style="color:rgb(0, 0, 0);">0x03 遥控模式</font> | |
| <font style="color:rgb(0, 0, 0);">byte [2]</font> | <font style="color:rgb(0, 0, 0);">电池电压高八位</font> | <font style="color:rgb(0, 0, 0);">unsigned int16</font> | <font style="color:rgb(0, 0, 0);">实际电压X 10 (精确到0.1V)</font> | |
| <font style="color:rgb(0, 0, 0);">byte [3]</font> | <font style="color:rgb(0, 0, 0);">电池电压低八位</font> | | | |
| <font style="color:rgb(0, 0, 0);">byte [4]</font> | <font style="color:rgb(0, 0, 0);">故障信息最高位</font> | <font style="color:rgb(0, 0, 0);">unsigned int32</font> | <font style="color:rgb(0, 0, 0);">详见故障信息说明表</font> | |
| <font style="color:rgb(0, 0, 0);">byte [5]</font> | <font style="color:rgb(0, 0, 0);">故障信息次高位</font> | | | |
| <font style="color:rgb(0, 0, 0);">byte [6]</font> | <font style="color:rgb(0, 0, 0);">故障信息次低位</font> | | | |
| <font style="color:rgb(0, 0, 0);">byte [7]</font> | <font style="color:rgb(0, 0, 0);">故障信息最低位</font> | | | |


<font style="color:rgb(0, 0, 0);">表格 1 故障信息说明表</font>

| <font style="color:rgb(0, 0, 0);">故障信息说明</font> | | |
| :---: | --- | --- |
| <font style="color:rgb(0, 0, 0);">字节</font> | <font style="color:rgb(0, 0, 0);">位</font> | <font style="color:rgb(0, 0, 0);">含义</font> |
| <font style="color:rgb(0, 0, 0);">byte [4]</font> | <font style="color:rgb(0, 0, 0);">bit [0]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [1]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [2]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [3]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [4]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [5]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [6]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [7]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| <font style="color:rgb(0, 0, 0);">byte [5]</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">bit [0]</font> | <font style="color:rgb(0, 0, 0);">右前转向零位校准状态（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [1]</font> | <font style="color:rgb(0, 0, 0);">右后转向零位校准状态（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [2]</font> | <font style="color:rgb(0, 0, 0);">左后转向零位校准状态（0:无故障 1：故障） </font> |
| | <font style="color:rgb(0, 0, 0);">bit [3]</font> | <font style="color:#222328;">左前转向零位校准状态（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [4]</font> | <font style="color:rgb(0, 0, 0);">转向校准超时（0:无故障 1：故障） </font> |
| | <font style="color:rgb(0, 0, 0);">bit [5]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [6]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [7]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| <font style="color:rgb(0, 0, 0);">byte [6]</font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">bit [0]</font> | <font style="color:rgb(0, 0, 0);">驱动器状态错误（0：无故障1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [1]</font> | <font style="color:rgb(0, 0, 0);">保留，默认0</font> |
| | <font style="color:rgb(0, 0, 0);">bit [2]</font> | <font style="color:rgb(0, 0, 0);">5号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [3]</font> | <font style="color:rgb(0, 0, 0);">6号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [4]</font> | <font style="color:rgb(0, 0, 0);">7号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [5]</font> | <font style="color:rgb(0, 0, 0);">8号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [6]</font> | <font style="color:rgb(0, 0, 0);">过温保护（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [7]</font> | <font style="color:rgb(0, 0, 0);">过流保护（0:无故障 1：故障）</font> |
| <font style="color:rgb(0, 0, 0);">byte [7] </font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font><br/><font style="color:rgb(0, 0, 0);"></font> | <font style="color:rgb(0, 0, 0);">bit [0]</font> | <font style="color:rgb(0, 0, 0);">电池欠压故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [1]</font> | <font style="color:rgb(0, 0, 0);">过压保护（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [2]</font> | <font style="color:rgb(0, 0, 0);">遥控器失联保护（0： 无故障 1： 故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [3]</font> | <font style="color:rgb(0, 0, 0);">1号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [4]</font> | <font style="color:rgb(0, 0, 0);">2号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [5]</font> | <font style="color:rgb(0, 0, 0);">3号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [6]</font> | <font style="color:rgb(0, 0, 0);">4号电机驱动器通讯故障（0:无故障 1：故障）</font> |
| | <font style="color:rgb(0, 0, 0);">bit [7]</font> | <font style="color:rgb(0, 0, 0);">急停，（0:正常 1：触发急停）</font> |


运动控制回馈帧指令包含了当前车体的运动线速度、转向角度回馈 

协议具体内容如下

| <font style="color:#404040;">指令名称 </font> | <font style="color:#404040;">运动控制回馈指令</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接 收 节 点 </font> | <font style="color:rgb(35, 31, 32);">ID </font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">线控底盘</font> | <font style="color:rgb(35, 31, 32);">决策控制单元</font> | <font style="color:rgb(35, 31, 32);">0x221</font> | <font style="color:rgb(35, 31, 32);">20ms</font> | <font style="color:rgb(35, 31, 32);">无</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x08</font> |  |  | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | <font style="color:rgb(35, 31, 32);">数据类型</font> | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[1]</font> | <font style="color:rgb(35, 31, 32);">移动速度高八位</font><br/><font style="color:rgb(35, 31, 32);">移动速度低八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(35, 31, 32);">实际速度X 1000 (单位0.001m/s)</font> | |
| <font style="color:rgb(35, 31, 32);">byte [2]</font> | <font style="color:rgb(35, 31, 32);">自旋速度高八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(35, 31, 32);">车体旋转角速度，单位0.001rad/s</font> | |
| <font style="color:rgb(35, 31, 32);">byte [3]</font> | <font style="color:rgb(35, 31, 32);">自旋速度低八位</font> | | | |
| <font style="color:rgb(35, 31, 32);">byte [4]</font> | <font style="color:rgb(35, 31, 32);">保留</font> | <font style="color:rgb(35, 31, 32);">-</font> | <font style="color:rgb(35, 31, 32);">0X00</font> | |
| <font style="color:rgb(35, 31, 32);">byte [5]</font> | <font style="color:rgb(35, 31, 32);">保留</font> | <font style="color:rgb(35, 31, 32);">-</font> | <font style="color:rgb(35, 31, 32);">0X00</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[6]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[7]</font> | <font style="color:rgb(35, 31, 32);">转角高八位</font><br/><font style="color:rgb(35, 31, 32);">转角低八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(35, 31, 32);">实际内转角X 1000 (单位</font><font style="color:rgb(35, 31, 32);">0.001rad </font><font style="color:rgb(35, 31, 32);">)</font> | |


运动控制帧包含了线速度控制指令、转角控制指令，其具体协议内容如下：

| <font style="color:#404040;">指令名称	 控制指令</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接收节点</font> | <font style="color:rgb(35, 31, 32);">ID</font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">决策控制单元</font> | <font style="color:rgb(35, 31, 32);">底盘节点</font> | <font style="color:rgb(35, 31, 32);">0x111</font> | <font style="color:rgb(35, 31, 32);">20ms</font> | <font style="color:rgb(35, 31, 32);">500ms</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x08</font> | | | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | 数据类型 | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[1]</font> | <font style="color:rgb(35, 31, 32);">线速度高八位</font><br/><font style="color:rgb(35, 31, 32);">线速度低八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(0, 0, 0);">车体行进速度，单位mm/s(有效值+ -1500，当转向角度 >20°时有效值+ -525；在前后阿克曼和斜移模式下生效)</font> | |
| <font style="color:rgb(35, 31, 32);">byte [2]</font> | <font style="color:rgb(35, 31, 32);">自旋速度高八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(35, 31, 32);">车体旋转角速度，单位0.001rad/s</font><br/><font style="color:rgb(35, 31, 32);">(有效值+-3259，逆时针旋转为正值)</font> | |
| <font style="color:rgb(35, 31, 32);">byte [3]</font> | <font style="color:rgb(35, 31, 32);">自旋速度低八位</font> | | | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[4]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[5]</font> | <font style="color:rgb(35, 31, 32);">保留</font> | <font style="color:rgb(35, 31, 32);">--</font> | <font style="color:rgb(35, 31, 32);">--</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[6]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[7]</font> | <font style="color:rgb(35, 31, 32);">转角高八位</font><br/><font style="color:rgb(35, 31, 32);">转角低八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(35, 31, 32);">转向内转角角度 单位：0.001rad (有效值</font><font style="color:rgb(35, 31, 32);">前后阿克曼模式</font><font style="color:rgb(35, 31, 32);">+ -698，</font><font style="color:rgb(35, 31, 32);">斜移模式+ - 1571</font><font style="color:rgb(35, 31, 32);">)</font><br/><font style="color:rgb(35, 31, 32);">左转方向为正值</font> | |


如图3.2.1，当RANGER AIR  底盘处于前后阿克曼模式时，反馈的转角为（α+β）/2，负值为左转方向，正值为右转方向；反馈的速度为四轮速度平均值（即底盘运动线速度），负值为倒车，正值为前进。若需要查看各个轮子的详细转角和速度信息，参看0X271和0X281反馈帧。

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769519097762-7b110720-2070-4d28-90e8-7048b7faec04.png" width="698" alt="" title="" crop="0,0,1,1" id="u5aec61d1" class="ne-image">

图3.2.1RANGER AIR  阿克曼结构

如图3.2.2，当RANGER AIR  处于斜移模式时，反馈的转角为（α1+α2+α3+α4）/4，负值为右转方向，正值为左转方向；反馈的线速度为四轮速度平均值，负值为倒车，正值为前进。若需要查看各个轮子的详细转角和速度信息，参看0x271和0x281反馈帧。

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769519380518-c4a86459-7200-4dca-8cf4-e81748c73f01.png" width="564" alt="" title="" crop="0,0,1,1" id="u750099ee" class="ne-image">

图3.2.2ranger斜移结构

当底盘处于自旋模式时，转角为定值不可控，此时转角反馈为α1、α2、α3、α4四个实际角度的绝对值的平均值。可通过指令控制底盘自旋速度，<font style="color:rgb(35, 31, 32);">逆时针旋转为正值。</font>

模式设定帧用于设定终端的控制接口，其具体协议内容如下。

| <font style="color:#404040;">指令名称	 控制指令</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接收节点</font> | <font style="color:rgb(35, 31, 32);">ID</font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">决策控制单元</font> | <font style="color:rgb(35, 31, 32);">底盘节点</font> | <font style="color:rgb(35, 31, 32);">0x421</font> | <font style="color:rgb(35, 31, 32);">无</font> | <font style="color:rgb(35, 31, 32);">无</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x01</font> | | | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | 数据类型 | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font> | <font style="color:rgb(35, 31, 32);">控制模式</font> | <font style="color:rgb(35, 31, 32);">unsigned int8</font> | <font style="color:rgb(35, 31, 32);">0x00 待机模式</font><br/><font style="color:rgb(35, 31, 32);">0x01</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">CAN指令模式   </font><br/><font style="color:rgb(35, 31, 32);">上电默认进入待机模式</font> | |


控制模式说明：底盘在开机上电，遥控器未连接的情况下，控制模式默认是待机模式，此时底盘只接收控制模式指令，其他指令不做响应，要使用CAN进行控制需要先切换到CAN指令模式。若打开遥控器，遥控器具有最高权限，可以屏蔽指令的控制，切换控制模式。

状态置位帧用于清除系统错误，其具体协议内容如下。

| <font style="color:#404040;">指令名称</font> | <font style="color:#404040;">控制指令</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接收节点</font> | <font style="color:rgb(35, 31, 32);">ID</font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">决策控制单元</font> | <font style="color:rgb(35, 31, 32);">底盘节点</font> | <font style="color:rgb(35, 31, 32);">0x441</font> | <font style="color:rgb(35, 31, 32);">无</font> | <font style="color:rgb(35, 31, 32);">无</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x01</font> | | | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | 数据类型 | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font> | <font style="color:rgb(35, 31, 32);">错误清除指令</font> | <font style="color:rgb(35, 31, 32);">unsigned int8</font> | <font style="color:rgb(35, 31, 32);">0x00 清除全部非严重故障（包括急停释放后清除）</font><br/><font style="color:rgb(35, 31, 32);">0x01~0x08分别对应清除1~8号电机驱动器通讯故障</font><br/><font style="color:rgb(35, 31, 32);">0x09 清除电池欠压故障，并尝试恢复动力电源</font><br/><font style="color:rgb(35, 31, 32);">0x0a 清除遥控信号丢失故障</font><br/><font style="color:rgb(35, 31, 32);">0x0b~0x0e 分别对应清除5~8号电机转向校准故障</font><br/><font style="color:rgb(35, 31, 32);">0x0f</font><font style="color:rgb(35, 31, 32);"> 清除过流故障</font><br/><font style="color:rgb(35, 31, 32);">0x10 清除过温故障</font> | |


示例数据，以下数据仅供测试使用，**使用前需要先使能指令控制模式**

<font style="color:rgb(0, 0, 0);">1，小车以0.15m/S的速度前进</font>

| <font style="color:#222328;background-color:#ffffff;">byte [0]</font> | <font style="color:#222328;background-color:#ffffff;">byte [1]</font> | <font style="color:#222328;background-color:#ffffff;">byte [2]</font> | <font style="color:#222328;background-color:#ffffff;">byte [3]</font> | <font style="color:#222328;background-color:#ffffff;">byte [4]</font> | <font style="color:#222328;background-color:#ffffff;">byte [5]</font> | <font style="color:#222328;background-color:#ffffff;">byte [6]</font> | <font style="color:#222328;background-color:#ffffff;">byte [7]</font> |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x96</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> |


<font style="color:#222328;background-color:#ffffff;">2. 小车转向10°</font>

| <font style="color:#222328;background-color:#ffffff;">byte [0]</font> | <font style="color:#222328;background-color:#ffffff;">byte [1]</font> | <font style="color:#222328;background-color:#ffffff;">byte [2]</font> | <font style="color:#222328;background-color:#ffffff;">byte [3]</font> | <font style="color:#222328;background-color:#ffffff;">byte [4]</font> | <font style="color:#222328;background-color:#ffffff;">byte [5]</font> | <font style="color:#222328;background-color:#ffffff;">byte [6]</font> | <font style="color:#222328;background-color:#ffffff;">byte [7]</font> |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x00</font> | <font style="color:#222328;background-color:#ffffff;">0x03</font> | <font style="color:#222328;background-color:#ffffff;">0xe8</font> |


除了底盘的状态信息会进行反馈以外，底盘反馈的信息还包括四轮的转角和转速，电机的电流信息、编码器以及温度信息。      

具体协议内容如下：

PS：在底盘中八个电机编号对应为：右前轮1号，右后轮2号，左后轮3号，左前轮4号，右前转向5号，右后转向6号，左后转向7号，左前转向8号.

电机转速电流位置信息反馈

| <font style="color:#404040;">指令名称</font> | <font style="color:#404040;">电机驱动器高速信息反馈帧</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接收节点</font> | <font style="color:rgb(35, 31, 32);">ID</font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">线控底盘</font> | <font style="color:rgb(35, 31, 32);">决策控制单元</font> | <font style="color:rgb(35, 31, 32);">0x251~0x258</font>  | <font style="color:rgb(35, 31, 32);">20ms</font> | <font style="color:rgb(35, 31, 32);">无</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x08</font> | | | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | <font style="color:rgb(35, 31, 32);">数据类型</font> | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[1]</font> | <font style="color:rgb(35, 31, 32);">电机转速高八位</font><br/><font style="color:rgb(35, 31, 32);">电机转速低八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | 电机当前转速 单位RPM | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[2]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[3]</font> | <font style="color:rgb(35, 31, 32);">电机电流高八位</font><br/><font style="color:rgb(35, 31, 32);">电机电流低八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | 电机当前电流 单位0.1A | |
| <font style="color:rgb(35, 31, 32);">byte </font><font style="color:rgb(35, 31, 32);">[4] </font><br/><font style="color:rgb(35, 31, 32);">byte </font><font style="color:rgb(35, 31, 32);">[5]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[6]</font><br/><font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[7]</font> | <font style="color:rgb(35, 31, 32);">位置最高位</font><br/><font style="color:rgb(35, 31, 32);">位置次高位</font><br/><font style="color:rgb(35, 31, 32);">位置次低位</font><br/><font style="color:rgb(35, 31, 32);">位置最低位</font> | <font style="color:rgb(35, 31, 32);">signed int32</font> | <br/>电机当前位置 单位：脉冲数 | |


电机温度电压及状态反馈

| <font style="color:#404040;">指令名称	 电机驱动器低速信息反馈帧</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接 收 节 点 </font> | <font style="color:rgb(35, 31, 32);">ID </font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">线控底盘</font> | <font style="color:rgb(35, 31, 32);">策控制单元</font> | <font style="color:rgb(35, 31, 32);">0x261~0x268</font> | <font style="color:rgb(35, 31, 32);">100ms</font> | <font style="color:rgb(35, 31, 32);">无</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x08</font> |  |  | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | <font style="color:rgb(35, 31, 32);">数据类型</font> | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font> | <font style="color:rgb(35, 31, 32);">驱动器电压高八位</font> | <font style="color:rgb(35, 31, 32);">unsigned int16</font> | <font style="color:rgb(35, 31, 32);">当前驱动器电压 单位0.1V</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[1]</font> | <font style="color:rgb(35, 31, 32);">驱动器电压低八位</font> | | | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[2]</font> | <font style="color:rgb(35, 31, 32);">驱动器温度高八位</font> | <font style="color:rgb(35, 31, 32);">signed int16</font> | <font style="color:rgb(35, 31, 32);">单位1℃</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[3]</font> | <font style="color:rgb(35, 31, 32);">驱动器温度低八位</font> | | | |
| <font style="color:rgb(35, 31, 32);">byte [4]</font> | <font style="color:rgb(35, 31, 32);">电机温度</font> | <font style="color:rgb(35, 31, 32);">signed int8</font> | <font style="color:rgb(35, 31, 32);">单位 1℃</font><br/><font style="color:rgb(35, 31, 32);">（对于RANGM系列为无效值，可忽略）</font> | |
| <font style="color:rgb(35, 31, 32);">byte [5]</font> | <font style="color:rgb(35, 31, 32);">驱动器状态</font> | <font style="color:rgb(35, 31, 32);">unsigned int8</font> | <font style="color:rgb(35, 31, 32);">详见表2</font> | |
| <font style="color:rgb(35, 31, 32);">byte [6]</font> | <font style="color:rgb(35, 31, 32);">保留</font> | <font style="color:rgb(35, 31, 32);">-</font> | <font style="color:rgb(35, 31, 32);">0X00</font> | |
| <font style="color:rgb(35, 31, 32);">byte [7]</font> | <font style="color:rgb(35, 31, 32);">保留</font> | <font style="color:rgb(35, 31, 32);">-</font> | <font style="color:rgb(35, 31, 32);">0X00</font> | |


表格 2 驱动器状态

| 字节 | 位 | 含义 |
| :---: | :---: | :---: |
| byte[5] | bit[0] | <font style="color:rgb(35, 31, 32);">电源电压是否过低</font><font style="color:rgb(35, 31, 32);">（</font><font style="color:rgb(35, 31, 32);">0</font><font style="color:rgb(35, 31, 32);">：正常</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">1</font><font style="color:rgb(35, 31, 32);">：过低</font><font style="color:rgb(35, 31, 32);">）</font> |
| | bit[1] | <font style="color:rgb(35, 31, 32);">电机是否过温</font><font style="color:rgb(35, 31, 32);">（</font><font style="color:rgb(35, 31, 32);">0</font><font style="color:rgb(35, 31, 32);">：正常</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">1</font><font style="color:rgb(35, 31, 32);">：过温</font><font style="color:rgb(35, 31, 32);">）   </font> |
| | bit[2] | <font style="color:rgb(35, 31, 32);">驱动器是否过流</font><font style="color:rgb(35, 31, 32);">（</font><font style="color:rgb(35, 31, 32);">0</font><font style="color:rgb(35, 31, 32);">：正常</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">1</font><font style="color:rgb(35, 31, 32);">：过流</font><font style="color:rgb(35, 31, 32);">）</font> |
| | bit[3] | <font style="color:rgb(35, 31, 32);">驱动器是否过温</font><font style="color:rgb(35, 31, 32);">（</font><font style="color:rgb(35, 31, 32);">0</font><font style="color:rgb(35, 31, 32);">：正常</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">1</font><font style="color:rgb(35, 31, 32);">：过温</font><font style="color:rgb(35, 31, 32);">）</font> |
| | bit[4] | <font style="color:rgb(35, 31, 32);">传感器状态（0：正常 1：异常）</font> |
| | bit[5] | <font style="color:rgb(35, 31, 32);">驱动器错误状态</font><font style="color:rgb(35, 31, 32);">（</font><font style="color:rgb(35, 31, 32);">0</font><font style="color:rgb(35, 31, 32);">：正常</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">1</font><font style="color:rgb(35, 31, 32);">：错误</font><font style="color:rgb(35, 31, 32);">）</font> |
| | bit[6] | <font style="color:rgb(35, 31, 32);">驱动器使能状态</font><font style="color:rgb(35, 31, 32);">（</font><font style="color:rgb(35, 31, 32);">1</font><font style="color:rgb(35, 31, 32);">：使能</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">0</font><font style="color:rgb(35, 31, 32);">：失能</font><font style="color:rgb(35, 31, 32);">）</font> |
| | bit[7] | <font style="color:rgb(35, 31, 32);">保留</font> |


四轮转角反馈

| <font style="color:#404040;">指令名称</font> | <font style="color:#404040;">四轮转角信息反馈帧</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:#404040;">发送节点</font> | <font style="color:#404040;">接 收 节 点 </font> | <font style="color:#404040;">ID</font> | <font style="color:#404040;">周期（ms）</font> | <font style="color:#404040;">接收超时(ms)</font> |
| <font style="color:#404040;"></font><font style="color:#404040;">线控底盘</font> | <font style="color:#404040;">决策控制单元</font> | <font style="color:#404040;">0x271</font> | <font style="color:#404040;">20ms</font> | <font style="color:#404040;">无</font> |
| <font style="color:#404040;">数据长度</font> | <font style="color:#404040;">0x08</font> | <font style="color:#404040;"></font> | <font style="color:#404040;"></font> | |
| <font style="color:#404040;">位置</font> | <font style="color:#404040;">功能</font> | <font style="color:#404040;">数据类型</font> | <font style="color:#404040;">说明</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[0]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[1]</font> | <font style="color:#404040;">5号转向转角高八位</font><br/><font style="color:#404040;">5号转向转角低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转角 单位0.001rad</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[2]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[3]</font> | <font style="color:#404040;">6号转向转角高八位</font><br/><font style="color:#404040;">6号转向转角低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转角 单位0.001rad</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[4]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[5]</font> | <font style="color:#404040;">7号转向转角高八位</font><br/><font style="color:#404040;">7号转向转角低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转角 单位0.001rad</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[6]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[7]</font> | <font style="color:#404040;">8号转向转角高八位</font><br/><font style="color:#404040;">8号转向转角低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转角 单位0.001rad</font> | |


四轮转速反馈

| <font style="color:#404040;">指令名称</font> | <font style="color:#404040;">四轮转速信息反馈帧</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:#404040;">发送节点</font> | <font style="color:#404040;">接 收 节 点</font> | <font style="color:#404040;">ID</font> | <font style="color:#404040;">周期（ms）</font> | <font style="color:#404040;">接收超时(ms)</font> |
| <font style="color:#404040;"></font><font style="color:#404040;">线控底盘</font> | <font style="color:#404040;">决策控制单元</font> | <font style="color:#404040;">0x281 </font> | <font style="color:#404040;">20ms</font> | <font style="color:#404040;">无</font> |
| <font style="color:#404040;">数据长度</font> | <font style="color:#404040;">0x08</font> | <font style="color:#404040;"></font> | <font style="color:#404040;"></font> | |
| <font style="color:#404040;">位置</font> | <font style="color:#404040;">功能</font> | <font style="color:#404040;">数据类型</font> | <font style="color:#404040;">说明</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[0]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[1]</font> | <font style="color:#404040;">1号轮转速高八位</font><br/><font style="color:#404040;">1号轮转速低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转速 单位mm/s</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[2]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[3]</font> | <font style="color:#404040;">2号轮转速高八位</font><br/><font style="color:#404040;">2号轮转速低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转速 单位mm/s</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[4]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[5]</font> | <font style="color:#404040;">3号轮转速高八位</font><br/><font style="color:#404040;">3号轮转速低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转速 单位mm/s</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[6]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[7]</font> | <font style="color:#404040;">4号轮转速高八位</font><br/><font style="color:#404040;">4号轮转速低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">当前转速 单位mm/s</font> | |


运动模型切换指令用于切换底盘运动模型，其具体协议内容如下

| <font style="color:#404040;">指令名称</font> | <font style="color:#404040;">当前运动模式回馈指令</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:#404040;">发送节点</font> | <font style="color:#404040;">接 收 节 点 </font> | <font style="color:#404040;">ID </font> | <font style="color:#404040;">周期（ms）</font> | <font style="color:#404040;">接收超时(ms)</font> |
| <font style="color:#404040;"></font><font style="color:#404040;">线控底盘</font> | <font style="color:#404040;">决策控制单元</font> | <font style="color:#404040;">0x291</font> | <font style="color:#404040;">20ms</font> | <font style="color:#404040;">无</font> |
| <font style="color:#404040;">数据长度</font> | <font style="color:#404040;">0x03</font> | <font style="color:#404040;"></font> | <font style="color:#404040;"></font> | |
| <font style="color:#404040;">位置</font> | <font style="color:#404040;">功能</font> | <font style="color:#404040;">数据类型</font> | <font style="color:#404040;">说明</font> | |
| <font style="color:#404040;">byte [0]</font> | <font style="color:#404040;">当前底盘运动模式</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">0x00前后阿克曼模式</font><br/><font style="color:#404040;">0x01斜移模式</font><br/><font style="color:#404040;"> 0x02自旋模式</font><br/><font style="color:rgb(35, 31, 32);">0x03驻车模式</font> | |
| <font style="color:#404040;">byte [1]</font> | <font style="color:#404040;">是否处于运动模型切换过程</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">0x00 切换完成</font><br/><font style="color:#404040;">0x01 运动模型切换中</font><br/><font style="color:#404040;">模型切换过程不响应速度控制指令</font> | |
| byte<font style="color:#404040;"> [2]</font> | 当前驱动模式反馈 | <font style="color:#404040;">unsigned int8</font> | 0×00 电流驱动模式<br/>0×01 电压驱动模式 | |


运动模型切换指令用于切换底盘运动模型，其具体协议内容如下

| <font style="color:#404040;">指令名称	 控制指令</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:rgb(35, 31, 32);">发送节点</font> | <font style="color:rgb(35, 31, 32);">接收节点</font> | <font style="color:rgb(35, 31, 32);">ID</font> | <font style="color:rgb(35, 31, 32);">周期（ms）</font> | <font style="color:rgb(35, 31, 32);">接收超时(ms)</font> |
| <font style="color:rgb(35, 31, 32);">决策控制单元</font> | <font style="color:rgb(35, 31, 32);">底盘节点</font> | <font style="color:rgb(35, 31, 32);">0x141</font> | <font style="color:rgb(35, 31, 32);">无</font> | <font style="color:rgb(35, 31, 32);">无</font> |
| <font style="color:rgb(35, 31, 32);">数据长度</font> | <font style="color:rgb(35, 31, 32);">0x01</font> | | | |
| <font style="color:rgb(35, 31, 32);">位置</font> | <font style="color:rgb(35, 31, 32);">功能</font> | 数据类型 | <font style="color:rgb(35, 31, 32);">说明</font> | |
| <font style="color:rgb(35, 31, 32);">byte</font><font style="color:rgb(35, 31, 32);"> </font><font style="color:rgb(35, 31, 32);">[0]</font> | <font style="color:rgb(35, 31, 32);">运动模式</font> | <font style="color:rgb(35, 31, 32);">unsigned int8</font> | <font style="color:rgb(35, 31, 32);">0x00</font><font style="color:rgb(35, 31, 32);">前后阿克曼模式（</font><font style="color:rgb(35, 31, 32);">上电默认</font><font style="color:rgb(35, 31, 32);">）</font><br/><font style="color:rgb(35, 31, 32);">0x01斜移模式</font><br/><font style="color:rgb(35, 31, 32);">0x02自旋模式</font><br/><font style="color:rgb(35, 31, 32);">0x03驻车模式</font> | |


灯光控制指令如下

| <font style="color:rgb(0, 0, 0);">指令名称</font> | <font style="color:rgb(0, 0, 0);">灯光控制帧</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:rgb(0, 0, 0);">发 送 节 点</font> | <font style="color:rgb(0, 0, 0);">接收节点 </font> | <font style="color:rgb(0, 0, 0);">ID</font> | <font style="color:rgb(0, 0, 0);">周期（ms）</font> | <font style="color:rgb(0, 0, 0);">接收超时(ms)</font> |
| <font style="color:rgb(0, 0, 0);">决策控制单元</font> | <font style="color:rgb(0, 0, 0);">线控底盘</font> | <font style="color:rgb(0, 0, 0);">0x121</font> | <font style="color:rgb(0, 0, 0);">20ms</font> | <font style="color:rgb(0, 0, 0);">500ms</font> |
| <font style="color:rgb(0, 0, 0);">数据长度</font> | <font style="color:rgb(0, 0, 0);">0x08</font> | | | |
| <font style="color:rgb(0, 0, 0);">位置</font> | <font style="color:rgb(0, 0, 0);">功能</font> | <font style="color:rgb(0, 0, 0);">数据类型</font> | <font style="color:rgb(0, 0, 0);">说明</font> | |
| <font style="color:rgb(0, 0, 0);">byte [0]</font> | <font style="color:rgb(0, 0, 0);">灯光控制使能标志</font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0x00 控制指令无效</font><br/><font style="color:rgb(0, 0, 0);">0x01灯光控制使能</font> | |
| <font style="color:rgb(0, 0, 0);">byte [1]</font> | <font style="color:rgb(0, 0, 0);">灯光模式</font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0x00 常关</font><br/><font style="color:rgb(0, 0, 0);">0x01 常开</font> | |
| <font style="color:rgb(0, 0, 0);">byte [2]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [3]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [4]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [5]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [6]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [7]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |


灯光控制指令反馈帧如下

| <font style="color:rgb(0, 0, 0);">指令名称</font> | <font style="color:rgb(0, 0, 0);">灯光控制反馈帧</font> | | | |
| :---: | :---: | --- | --- | --- |
| <font style="color:rgb(0, 0, 0);">发送节点</font> | <font style="color:rgb(0, 0, 0);">接收节点</font> | <font style="color:rgb(0, 0, 0);">ID</font> | <font style="color:rgb(0, 0, 0);">周期（ms）</font> | <font style="color:rgb(0, 0, 0);">接收超时(ms)</font> |
| <font style="color:rgb(0, 0, 0);">线控底盘 </font> | <font style="color:rgb(0, 0, 0);">决策控制单元 </font> | <font style="color:rgb(0, 0, 0);">0x231</font> | <font style="color:rgb(0, 0, 0);">20ms</font> | <font style="color:rgb(0, 0, 0);">无</font> |
| <font style="color:rgb(0, 0, 0);">数据长度</font> | <font style="color:rgb(0, 0, 0);">0x08</font> | | | |
| <font style="color:rgb(0, 0, 0);">位置</font> | <font style="color:rgb(0, 0, 0);">功能</font> | <font style="color:rgb(0, 0, 0);">数据类型</font> | <font style="color:rgb(0, 0, 0);">说明</font> | |
| <font style="color:rgb(0, 0, 0);">byte [0]</font> | <font style="color:rgb(0, 0, 0);">当前灯光控制使能标志</font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0x00 控制指令无效</font><br/><font style="color:rgb(0, 0, 0);">0x01灯光控制使能</font> | |
| <font style="color:rgb(0, 0, 0);">byte [1]</font> | <font style="color:rgb(0, 0, 0);">当前灯光模式</font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0x00 常关</font><br/><font style="color:rgb(0, 0, 0);">0x01 常开</font> | |
| <font style="color:rgb(0, 0, 0);">byte [2]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [3]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [4]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [5]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [6]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | <font style="color:rgb(0, 0, 0);">_ _</font> | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:rgb(0, 0, 0);">byte [7]</font> | <font style="color:rgb(0, 0, 0);">计数校验 (count)</font> | <font style="color:rgb(0, 0, 0);">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0~255循环计数，每发送一条指令计数加一次</font> | |


<font style="color:rgb(0, 0, 0);">里程计信息反馈帧如下</font>

<font style="color:rgb(0, 0, 0);">前轮</font>

| 指令名称 | 前轮里程反馈 | | | |
| :---: | :---: | --- | --- | --- |
| 发送节点 | 接收节点 | id | 周期（ms） | 接收超时（ms） |
| 线控底盘 | 决策控制单元 | 0×311 | 20ms | 无 |
| 数据长度 | 0×08 | | | |
| 字节 | 描述 | 数据类型 | 说明 | |
| <font style="color:#404040;">byte [0]</font><br/><font style="color:#404040;">byte [1]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[2]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[3]</font> | <font style="color:rgb(0, 0, 0);">前轮左轮里程计最高位 前轮左轮里程计次高位 前轮左轮里程计次低位 前轮左轮里程计最低位</font> | <font style="color:rgb(0, 0, 0);">signed int32</font> | <font style="color:rgb(0, 0, 0);">底盘左轮里程计反馈， 单位：mm</font> | |
| | | | | |
| | | | | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[4]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[5]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[6]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[7]</font> | <font style="color:rgb(0, 0, 0);">前轮右轮里程计最高位 前轮右轮里程计次高位 前轮右轮里程计次低位 前轮右轮里程计最低位</font> | <font style="color:rgb(0, 0, 0);">signed int32</font> | <font style="color:rgb(0, 0, 0);">底盘右轮里程计反馈， 单位：mm</font> | |
| | | | | |


后轮

| 指令名称 | 后轮里程反馈 | | | |
| :---: | :---: | --- | --- | --- |
| 发送节点 | 接收节点 | id | 周期（ms） | 接收超时（ms） |
| 线控底盘 | 决策控制单元 | 0×312 | 20ms | 无 |
| 数据长度 | 0×08 | | | |
| 字节 | 描述 | 数据类型 | 说明 | |
| <font style="color:#404040;">byte [0]</font><br/><font style="color:#404040;">byte [1]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[2]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[3]</font> | <font style="color:rgb(0, 0, 0);">后轮左轮里程计最高位 后轮左轮里程计次高位 后轮左轮里程计次低位 后轮左轮里程计最低位</font> | <font style="color:rgb(0, 0, 0);">signed int32</font> | <font style="color:rgb(0, 0, 0);">底盘左轮里程计反馈， 单位：mm</font> | |
| | | | | |
| | | | | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[4]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[5]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[6]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[7]</font> | <font style="color:rgb(0, 0, 0);">后轮右轮里程计最高位 后轮右轮里程计次高位 后轮右轮里程计次低位 后轮右轮里程计最低位</font> | <font style="color:rgb(0, 0, 0);">signed int32</font> | <font style="color:rgb(0, 0, 0);">底盘右轮里程计反馈， 单位：mm</font> | |
| | | | | |


<font style="color:rgb(0, 0, 0);">遥控器信息反馈帧如下表</font>

| <font style="color:#404040;">指令名称	 </font><font style="color:rgb(0, 0, 0);">遥控器信息反馈</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:#404040;">发送节点</font> | <font style="color:#404040;">接收节点</font> | <font style="color:#404040;">ID</font> | <font style="color:#404040;">周期（ms）</font> | <font style="color:#404040;">接收超时(ms)</font> |
| <font style="color:#404040;">线控底盘</font> | <font style="color:#404040;">决策控制单元</font> | <font style="color:#404040;">0x241</font> | <font style="color:#404040;">20ms</font> | <font style="color:#404040;">无</font> |
| <font style="color:#404040;">数据长度</font> | <font style="color:#404040;">0x08</font> | | | |
| <font style="color:#404040;">位置</font> | <font style="color:#404040;">功能</font> | <font style="color:#404040;">数据类型</font> | <font style="color:#404040;">说明</font> | |
| <font style="color:#404040;">byte [0]</font> | <font style="color:rgb(0, 0, 0);">遥控SW反馈</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">bit[0-1]: SWA ：2-上档 3-下档</font><br/><font style="color:#404040;">bit[2-3]: SWB：2-上</font>档 1-中档 3-下档bit[4-5]: SWC：2-上档 1-中档 3-下档bit[6-7]: SWD：2-上档 3-下档 | |
| <font style="color:#404040;">byte [1]</font> | <font style="color:rgb(0, 0, 0);">右边拨杆左右</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">值域：[-100,100] </font> | |
| <font style="color:#404040;">byte [2]</font> | <font style="color:rgb(0, 0, 0);">右边拨杆上下</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">值域：[-100,100]</font> | |
| <font style="color:#404040;">byte [3]</font> | <font style="color:rgb(0, 0, 0);">左边拨杆上下</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">值域：[-100,100]</font> | |
| <font style="color:#404040;">byte [4]</font> | <font style="color:rgb(0, 0, 0);">左边拨杆左右</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">值域：[-100,100]</font> | |
| <font style="color:#404040;">byte [5]</font> | <font style="color:rgb(0, 0, 0);">左边旋钮VRA</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">值域：[-100,100]</font> | |
| <font style="color:#404040;">byte [6]</font> | <font style="color:rgb(0, 0, 0);">保留</font> | -- | <font style="color:rgb(0, 0, 0);">0x00</font> | |
| <font style="color:#404040;">byte [7]</font> | <font style="color:rgb(0, 0, 0);">计数校验</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:rgb(0, 0, 0);">0-255循环计数</font> | |


总电池BMS数据反馈，其具体协议内容如下

| <font style="color:#404040;">指令名称	 BMS数据反馈</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:#404040;">发送节点</font> | <font style="color:#404040;">接收节点</font> | <font style="color:#404040;">ID</font> | <font style="color:#404040;">周期（ms）</font> | <font style="color:#404040;">接收超时(ms)</font> |
| <font style="color:#404040;">线控底盘</font> | <font style="color:#404040;">决策控制单元</font> | <font style="color:#404040;">0x361</font> | <font style="color:#404040;">500ms</font> | <font style="color:#404040;">无</font> |
| <font style="color:#404040;">数据长度</font> | <font style="color:#404040;">0x08</font> | | | |
| <font style="color:#404040;">位置</font> | <font style="color:#404040;">功能</font> | <font style="color:#404040;">数据类型</font> | <font style="color:#404040;">说明</font> | |
| <font style="color:#404040;">byte [0]</font> | <font style="color:#404040;">电池SOC</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">范围 0~100</font> | |
| <font style="color:#404040;">byte [1]</font> | <font style="color:#404040;">电池SOH</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">范围 0~100</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[2]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[3]</font> | <font style="color:#404040;">电池电压值高八位</font><br/><font style="color:#404040;">电池电压值低八位</font> | <font style="color:#404040;">unsigned int16</font> | <font style="color:#404040;">单位：</font><font style="color:#404040;">0.1V</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[4]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[5]</font> | <font style="color:#404040;">电池电流值高八位</font><br/><font style="color:#404040;">电池电流值低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">单位：</font><font style="color:#404040;">0.1A</font> | |
| <font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[6]</font><br/><font style="color:#404040;">byte</font><font style="color:#404040;"> </font><font style="color:#404040;">[7]</font> | <font style="color:#404040;">电池温度高八位</font><br/><font style="color:#404040;">电池温度低八位</font> | <font style="color:#404040;">signed int16</font> | <font style="color:#404040;">单位：</font><font style="color:#404040;">0.1℃</font> | |




| <font style="color:#404040;">指令名称 	BMS数据反馈</font> | | | | |
| --- | --- | --- | --- | --- |
| <font style="color:#404040;">发送节点</font> | <font style="color:#404040;">接 收 节 点 </font> | <font style="color:#404040;">ID </font> | <font style="color:#404040;">周期（ms）f</font> | <font style="color:#404040;">接收超时(ms)</font> |
| <font style="color:#404040;">线控底盘</font> | <font style="color:#404040;">决策控制单元</font> | <font style="color:#404040;">0x362</font> | <font style="color:#404040;">500ms</font> | <font style="color:#404040;">无</font> |
| <font style="color:#404040;">数据长度</font> | <font style="color:#404040;">0x04</font> | | | |
| <font style="color:#404040;">位置</font> | <font style="color:#404040;">功能</font> | <font style="color:#404040;">数据类型</font> | <font style="color:#404040;">说明</font> | |
| <font style="color:#404040;">byte [0]</font> | <font style="color:#404040;">Alarm Status 1</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">BIT1</font><font style="color:#404040;">：过压</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT2</font><font style="color:#404040;">：欠压</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT3</font><font style="color:#404040;">：高温</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT4</font><font style="color:#404040;">：低温</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT7</font><font style="color:#404040;">：放电过流</font> | |
| <font style="color:#404040;">byte [1]</font> | <font style="color:#404040;">Alarm Status 2</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">BIT0</font><font style="color:#404040;">：充电过流</font> | |
| <font style="color:#404040;">byte [2]</font> | <font style="color:#404040;">Warning Status 1</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">BIT1</font><font style="color:#404040;">：过压</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT2</font><font style="color:#404040;">：欠压</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT3</font><font style="color:#404040;">：高温</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT4</font><font style="color:#404040;">：低温</font><font style="color:#404040;">  </font><font style="color:#404040;">BIT7</font><font style="color:#404040;">：放电过流</font> | |
| <font style="color:#404040;">byte [3]</font> | <font style="color:#404040;">Warning Status 2</font> | <font style="color:#404040;">unsigned int8</font> | <font style="color:#404040;">BIT0</font><font style="color:#404040;">：充电过流</font> | |


### <font style="color:rgb(35, 31, 32);">3.3 RANGER AIR  ROS使用说明</font>
<font style="color:rgb(0, 0, 0);">ROS提供一些标准操作系统服务，例如硬件抽象，底层设备控制，常用功能实现，进程间消息以及数据包管理。ROS是基于一种图状架构，从而不同节点的进程能接受、发布和聚合各种信息（例如传感，控制，状态，规划等等）。目前ROS主要支持UBUNTU。</font>

**<font style="color:rgb(0, 0, 0);">开发准备</font>**

**<font style="color:rgb(0, 0, 0);">硬件准备</font>**

+ <font style="color:rgb(0, 0, 0);">CANlight can通讯模块 X1 </font>
+ <font style="color:rgb(0, 0, 0);">笔记本电脑 X1 </font>
+ <font style="color:rgb(0, 0, 0);">AGILEX RANGER 移动机器人底盘 X1 </font>
+ <font style="color:rgb(0, 0, 0);">AGILEX RANGER 配套遥控器FS-i6s X1 </font>
+ <font style="color:rgb(0, 0, 0);">AGILEX RANGER 顶部航空插座 X1</font>

**<font style="color:rgb(0, 0, 0);">使用示例环境说明</font>**

+ <font style="color:rgb(0, 0, 0);">Ubuntu 20.04 </font>
+ <font style="color:rgb(0, 0, 0);">ROS noetic</font>
+ <font style="color:rgb(0, 0, 0);">Git</font>

**<font style="color:rgb(0, 0, 0);">硬件连接与准备</font>**<font style="color:rgb(0, 0, 0);"> </font>

+ <font style="color:rgb(0, 0, 0);">将顶部航空插头或者尾部插头CAN线引出，将CAN线中的CAN_H和CAN_L分别与CAN_TO_USB适配器相连； </font>
+ <font style="color:rgb(0, 0, 0);">打开移动机器人底盘旋钮开关，检查来两侧的急停开关是否释放； </font>
+ <font style="color:rgb(0, 0, 0);">将CAN_TO_USB连接至笔记本的usb口。连接示意如图3.4所示。</font>

<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1748489354092-fba00963-33a6-40b6-9380-eb5296a0b752.png" width="1188" alt="" title="" crop="0,0,1,1" id="u33741c72" class="ne-image">

<font style="color:rgb(0, 0, 0);">图3.4CAN线连接示意图</font>

**<font style="color:rgb(0, 0, 0);">ROS 安装和环境设置</font>**<font style="color:rgb(0, 0, 0);">：安装具体可以参考：</font>[http://wiki.ros.org/kinetic/Installation/Ubuntu](http://wiki.ros.org/kinetic/Installation/Ubuntu)

**<font style="color:rgb(0, 0, 0);">测试CANABLE硬件与CAN 通讯：</font>**

<font style="color:rgb(0, 0, 0);">使能 gs_usb 内核模块 </font>

```plain
sudo modprobe gs_usb
```

<font style="color:rgb(0, 0, 0);">设置500k波特率和使能can-to-usb适配器 </font>

```plain
sudo ip link set can0 up type can bitrate 500000
```

<font style="color:rgb(0, 0, 0);">如果在前面的步骤中没有发生错误，您应该可以使用命令立即查看can设备 </font>

```plain
ifconfig -a
```

<font style="color:rgb(0, 0, 0);">安装并使用can-utils来测试硬件</font>

```plain
sudo apt install can-utils
```

<font style="color:rgb(0, 0, 0);">若此次can-to-usb已经和机器人相连，且小车已经开启的情况下，使用下列指令可以监听来自底盘的数据了</font>

```plain
candump can0
```

<font style="color:rgb(0, 0, 0);">参考来源： </font>

<font style="color:rgb(0, 0, 0);">[1] https://github.com/agilexrobotics</font>

<font style="color:rgb(0, 0, 0);">[2]https://wiki.rdu.im/_pages/Notes/Embedded-System/Linux/can-bus-in-linux.html </font>

**<font style="color:rgb(0, 0, 0);">AGILEX RANGER ROS PACKAGE 下载与编译：</font>**

+ <font style="color:rgb(0, 0, 0);">下载安装ros 依赖包 </font>

```plain
$ sudo apt install libasio-dev 
$ sudo apt install -y ros-$ROS_DISTRO-teleop-twist-keyboard
```

+ <font style="color:rgb(0, 0, 0);">克隆编译hunter_ros源码 </font>

```plain
$ cd ~/catkin_ws/src
$ git clone https://github.com/agilexrobotics/ranger_ros.git -b air_delta
$ cd ..
$ catkin_make
```

<font style="color:rgb(0, 0, 0);">参考来源：</font>[https://github.com/agilexrobotics/ranger_ros/tree/air_delta](https://github.com/agilexrobotics/ranger_ros/tree/air_delta)

**<font style="color:rgb(0, 0, 0);">启动ROS 节点：</font>**

1. 启动底盘节点

```plain
$ roslaunch agx_bringup robot.launch
```

注意启动之前需要先使能松灵配备的usb_to_can模块，使能指令如下：rosrun ranger_bringup bringup_can2usb.bash。该指令只需要每次给上电的usb_to_can模块执行一次。

1. 启动键盘节点

```plain
rosrun teleop_twist_keyboard teleop_twist_keyboard.py
```

注意查看终端打印，用指定的按键控制ranger运动

### **3.5 车体坐标系说明**
以车体中心为坐标原点，朝前为x轴正方向，朝左为y轴正方向，朝上为z轴正方向

### <font style="color:rgb(35, 31, 32);">3.6 固件升级</font>
为了方便用户对底盘所使用的固件版本进行升级，RANGER AIR底盘提供了固件升级的硬件接口以及与之对应的客户端软件。其客户端界面如下图所示。

**升级准备**

松灵USB TO CAN模块 X 1 

micro USB线X 1

RANGER AIR底盘 X 1

电脑(WINDOWS 操作系统) X 1

**升级过程**

1、<font style="color:rgb(0, 0, 0);">电脑插上USBTOCAN模块，再打开AgxTEST 软件(顺序不能错，先打开软件再插上模块会识别不到设备)</font>

<font style="color:rgb(0, 0, 0);">下载链接：</font>

<font style="color:rgb(0, 0, 0);">链接：</font>[https://pan.baidu.com/s/1-UFFui_GEG_S7Q9nYonQNg?pwd=h7vb](https://pan.baidu.com/s/1-UFFui_GEG_S7Q9nYonQNg?pwd=h7vb)

<font style="color:rgb(0, 0, 0);">提取码：h7vb </font>

<font style="color:rgb(0, 0, 0);">2、点击Open Serial按键，然后按下车体电源按键，连接成功的话会识别到主控的版本信息，如图所示</font>

<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1745826001270-809d51c5-9783-480e-bf68-09b9577744a5.png" width="553" alt="" title="" crop="0,0,1,1" id="u40bac18d" class="ne-image">

<font style="color:rgb(0, 0, 0);">3、点击Load Firmware File按钮加载要升级的固件，加载成功会获取到固件信息，如图所示</font>

<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1745826003417-509ae1b9-32f6-482d-80a1-b3eb1b7adf45.png" width="554" alt="" title="" crop="0,0,1,1" id="u875dc4cd" class="ne-image">

<font style="color:rgb(0, 0, 0);">4、在节点列表框中点击要升级的节点，然后点击Start Upgrade Firmware就可以开始升级固件，升级成功后会弹框提示。</font>

<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1745826003378-b78c47b6-8e6f-4c25-b7b2-76e642f42489.png" width="554" alt="" title="" crop="0,0,1,1" id="uc8fd19e1" class="ne-image">

<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1745826004834-5e5cf110-226b-48cf-b075-4a477a3fe871.png" width="553" alt="" title="" crop="0,0,1,1" id="u0bff1032" class="ne-image">

## **4、产品尺寸**
RANGER DELTA产品尺寸

<img src="https://cdn.nlark.com/yuque/0/2026/png/33709157/1769169652213-f3b1f3b1-ed8c-49f6-b9dd-6a9a7d63fda4.png" width="1165" alt="" title="" crop="0,0,1,1" id="uae459de1" class="ne-image">

安装孔位尺寸图

<img src="https://cdn.nlark.com/yuque/0/2026/png/68774675/1787213079102-bf962e38-626f-4ec6-85ff-f75420cb730d.png" width="651" alt="" title="" crop="0,0,1,1" id="ub7c922e7" class="ne-image">

<img src="https://cdn.nlark.com/yuque/0/2025/png/33709157/1745826004836-0cea1db4-d043-4036-9dd5-72435ca97f26.png" width="992" alt="" title="" crop="0,0,1,1" id="u911a3851" class="ne-image">
