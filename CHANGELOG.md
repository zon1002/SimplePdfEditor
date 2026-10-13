# 修改紀錄 (Changelog)

所有版本依 [語義化版本](https://semver.org/lang/zh-TW/) 命名：`主版號.次版號.修訂號`。
v1.x（C++ / Qt / MuPDF 版）的紀錄見原專案的 `CHANGELOG.md`。

---

## [v2.0.0] — 2026-10-14

以 Rust 重新撰寫，架構參考 [PdfCraft](https://github.com/storytold/pdfcraft)，功能與 v1.6.0 相同。

### 變更

- **PDF 引擎改為純 Rust**：hayro 負責解析、渲染與文字擷取，lopdf 負責修改、存檔與 AES-256（V5／R6）加密；
  沿用 PdfCraft 修補過的版本（`vendor/`，強化畸形 PDF 的防護），文字擷取與行排序改寫自 PdfCraft。
- **介面改為 egui / eframe**：以 wgpu（DirectX 12）繪製，無法使用時自動改用 OpenGL；介面字型使用系統的微軟正黑體 UI。
- **安裝不需任何額外套件**：C 執行環境靜態連結（`+crt-static`），程式只有 `SimplePdfEditor.exe` 與 `SimplePdfPreview.dll`；
  安裝程式移除 Microsoft Visual C++ 可轉發套件的下載與安裝，升級時刪除 v1.x 的 Qt／MuPDF DLL 與外掛資料夾。
- **字型子集嵌入**改用 subsetter，寫成 Type0／Identity-H 字型並附 ToUnicode（存檔後的文字可複製、搜尋）。
- **註記外觀串流**全部自行產生：高亮以相乘混色（Multiply），底線、線條、箭頭與編輯畫面使用相同幾何。
- **編輯畫面預覽**：文字註記以 tiny-skia 依相同字型外框繪製，高亮以相乘混色疊在頁面上，與存檔結果一致（`--selftest` 輸出 `preview.png` 與 `full_p1.png` 供比對）。
- **匯出部分頁面**時，未匯出頁面的內容不會因書籤、命名目的地或表單欄位的參照而殘留在檔案中。
- **檔案總管外掛**以 windows-rs 重寫（CLSID 不變），所有 COM 方法與視窗程序攔截例外，避免影響檔案總管。
- **列印**改用 Win32 列印對話框（頁面範圍、份數、自動分頁、彩色／黑白），在背景執行緒列印並顯示可取消的進度。
- 設定改存於 `%APPDATA%\SimplePdfEditor\settings.ini`。

### 移除

- 開發用參數 `--icons`、`--snapshot`、`--printtest`、`--appicons`（`--selftest` 保留）。
