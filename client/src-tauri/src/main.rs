// 仅在非调试的 Windows 发布构建中隐藏控制台窗口，不要删除这行属性。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    robot_teleop_client_lib::run()
}
