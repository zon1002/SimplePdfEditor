// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 簡易 PDF 編輯器（Rust 版）
//!
//! 用法：
//!   SimplePdfEditor.exe [檔案.pdf]
//!   SimplePdfEditor.exe --selftest <in.pdf> <輸出資料夾>   不開視窗，加上各種註記並輸出（供自動驗證）

#![windows_subsystem = "windows"]

mod app;
mod bars;
mod dialogs;
mod help;
mod icons;
mod preview;
mod print;
mod ruler;
mod settings;
mod view;

use app::App;
use dialogs::Pending;
use preview::PreviewAction;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// 介面字型：微軟正黑體 UI（找不到時用 egui 內建字型，中文可能無法顯示）
fn setup_fonts(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let mut fonts = egui::FontDefinitions::default();
    // msjh.ttc 第 2 個字型為「Microsoft JhengHei UI」
    for (file, index) in [("msjh.ttc", 1u32), ("msyh.ttc", 1), ("mingliu.ttc", 0)] {
        if let Ok(data) = std::fs::read(format!("{windir}\\Fonts\\{file}")) {
            let mut fd = egui::FontData::from_owned(data);
            fd.index = index;
            fonts.font_data.insert("cjk".into(), std::sync::Arc::new(fd));
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                f.insert(0, "cjk".into());
            }
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                f.push("cjk".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    });
    // Ctrl +／- 用來縮放頁面，不縮放整個介面
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}

fn window_icon() -> Option<egui::IconData> {
    let pix = tiny_skia::Pixmap::decode_png(include_bytes!("../../../icons/app.png")).ok()?;
    let (w, h) = (pix.width(), pix.height());
    let img = pdfcore::raster::pixmap_to_image(pix);
    Some(egui::IconData { rgba: img.rgba, width: w, height: h })
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);

        // 關閉視窗：有未儲存的編輯時先詢問
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.model.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.dialog.is_none() {
                self.dialog = Some(dialogs::Dialog::Discard { then: Pending::Close });
            }
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 拖放 PDF 開啟
        let dropped: Vec<String> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_string_lossy().into_owned()).collect());
        if let Some(p) = dropped.into_iter().find(|p| p.to_lowercase().ends_with(".pdf")) {
            if self.dialog.is_none() {
                self.request(Pending::OpenPath(p));
            }
        }

        self.handle_shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.show_menu(ui));
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(2.0);
            self.show_main_toolbar(ui);
            ui.add_space(1.0);
        });
        egui::Panel::top("props").show(ui, |ui| {
            ui.add_space(2.0);
            self.show_props_toolbar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.show_status(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.show_view(ui));

        help::show_help(&ctx, &mut self.help);
        if self.about_open {
            if let Some(k) = help::show_about(&ctx, &mut self.about_open) {
                let names: &[&str] = if k == 0 { &["LICENSE.txt", "LICENSE"] } else { &["THIRD_PARTY_NOTICES.md"] };
                match help::find_doc_file(names) {
                    Some(p) => open_with_shell(&p.to_string_lossy()),
                    None => self.error_box(format!("找不到 {}。\n完整授權條款請見 https://www.gnu.org/licenses/agpl-3.0.html", names[0])),
                }
            }
        }
        if let Some(pv) = self.preview.as_mut() {
            match pv.show(&ctx) {
                PreviewAction::None => {}
                PreviewAction::Close => self.preview = None,
                PreviewAction::Print => {
                    self.page = pv.page.min(self.page_count().saturating_sub(1));
                    self.preview = None;
                    self.print();
                }
            }
        }
        dialogs::show(self, &ctx);
        dialogs::show_print_progress(self, &ctx);

        let title = self.window_title();
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

fn open_with_shell(path: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, w};
    // SAFETY: 參數皆為有效的字串
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(path), None, None, SW_SHOWNORMAL);
    }
}

fn hwnd_of(cc: &eframe::CreationContext<'_>) -> isize {
    match cc.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

fn run(renderer: eframe::Renderer, file: Option<String>) -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default().with_title(app::APP_TITLE).with_inner_size([1180.0, 860.0]).with_drag_and_drop(true);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, renderer, ..Default::default() };
    eframe::run_native(
        "SimplePdfEditor",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            let mut app = App::new(hwnd_of(cc));
            if let Some(f) = file {
                app.open_path(&f);
            }
            Ok(Box::new(app))
        }),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--selftest") {
        // GUI 程式沒有主控台，結果另外寫到 selftest.log
        let (input, out) = (args.get(1).cloned().unwrap_or_default(), args.get(2).cloned().unwrap_or_else(|| ".".into()));
        let _ = std::fs::create_dir_all(&out);
        let code = match pdfcore::selftest::run(&input, &out) {
            Ok(log) => {
                let _ = std::fs::write(format!("{out}/selftest.log"), log);
                0
            }
            Err(e) => {
                let _ = std::fs::write(format!("{out}/selftest.log"), format!("{e}\n"));
                1
            }
        };
        std::process::exit(code);
    }
    let file = args.into_iter().find(|a| !a.starts_with("--"));
    // 以 wgpu（DX12）繪製；顯示卡或驅動無法使用時改用 OpenGL
    if run(eframe::Renderer::Wgpu, file.clone()).is_err() {
        let _ = run(eframe::Renderer::Glow, file);
    }
}
