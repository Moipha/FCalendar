// Release 模式下避免 Windows 额外弹出控制台窗口，勿删
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    f_calendar_lib::run()
}
