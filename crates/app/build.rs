// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

// exe 圖示與版本資訊（檔案總管「內容」頁顯示）
fn main() {
    println!("cargo:rerun-if-changed=../../icons/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../icons/app.ico");
        res.set("FileDescription", "簡易 PDF 編輯器");
        res.set("ProductName", "簡易 PDF 編輯器");
        res.set("CompanyName", "Fufeng Project");
        res.set("LegalCopyright", "Copyright (C) 2026 Fufeng Project — MIT OR Apache-2.0");
        res.set("OriginalFilename", "SimplePdfEditor.exe");
        res.set_language(0x0404);
        if let Err(e) = res.compile() {
            println!("cargo:warning=無法加入 exe 圖示：{e}");
        }
    }
}
