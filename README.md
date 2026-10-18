# 簡易 PDF 編輯器（SimplePdfEditor）— Rust 版

Rust / egui / hayro / lopdf 的輕量 PDF 註記工具（v2.3.0）。
由 C++17 / Qt6 / MuPDF 的 [v1.6.0](../SimplePdfEditor) 改寫，功能維持不變，架構參考 [PdfCraft](https://github.com/storytold/pdfcraft)
（同為純 Rust 的 PDF 工具：使用相同的 hayro 渲染、lopdf 存檔與 egui 介面，並沿用其修補版本與文字擷取模組）。

**安裝時不需要任何額外套件**：C 執行環境靜態連結，程式只有 `SimplePdfEditor.exe` 與檔案總管外掛
`SimplePdfPreview.dll` 兩個檔案，不需安裝 Microsoft Visual C++ 可轉發套件，也沒有 Qt 等 DLL。

## 下載

`dist/` 內為 v2.3.0 發行版：

| 檔案 | 說明 |
|------|------|
| `SimplePdfEditor_Setup_v2.3.0.exe` | 安裝程式（可選檔案總管預覽／縮圖整合、「開啟檔案」選單、桌面捷徑） |
| `SimplePdfEditor_v2.3.0_portable.zip` | 免安裝版：解壓縮後直接執行 `SimplePdfEditor.exe`（不含檔案總管整合） |

## 功能

開啟 PDF 後為**檢視模式**，不會改動任何內容；按工具列的 **編輯** 才能加入註記。
**取消編輯** 會放棄所有尚未儲存的修改、回到原始檔案；在編輯模式中 **另存新檔** 後，程式改為開啟存好的檔案並回到檢視模式。
除非在編輯模式中另存新檔，否則 PDF 原始內容一律不會被改動。

| 工具 | 快捷鍵 | 說明 |
|------|--------|------|
| 選取 | V | 點選註記、拖曳移動、Delete 刪除；雙擊文字可編輯；Ctrl+點選可多選；選取後可改顏色、字型、大小、線寬 |
| 高亮 | H | 拖曳選取文字加上螢光標記，顏色可選（無文字層的掃描檔以矩形標記） |
| 底線 | U | 拖曳選取文字加上底線，顏色可選 |
| 文字 | T | 在頁面任意位置點一下輸入文字（可多行），字型、大小、顏色可選，可加外框（顏色、粗細）與背景色（可半透明），之後可拖曳調整位置 |
| 線條 | L | 拖曳畫線，Shift 鎖定 45°；顏色、線寬可選，可設實線／虛線並調整虛線長與間隔 |
| 箭頭 | A | 拖曳畫箭頭（終點為箭頭端），Shift 鎖定 45°；箭頭大小、線寬可調，可選單向／雙向，可設虛線並調整虛線長與間隔 |
| 矩形 | R | 拖曳畫矩形框（中間鏤空，不遮蓋內容），Shift 畫正方形；線寬、顏色、實線／虛線可調 |
| 圓角矩形 | Q | 拖曳畫四角圓弧的矩形框，圓弧半徑可調（0–200 pt） |
| 圓形 | O | 拖曳畫圓形／橢圓框（中間鏤空），Shift 畫正圓 |

選取一個框後，四角與四邊中點會出現 8 個控制點，拖曳即可調整大小（Shift＋角落維持正方形／正圓），每次調整可復原。

屬性工具列會依目前工具（或選取的註記）只顯示相關的設定：文字工具顯示字型、大小、外框、背景，線條／箭頭顯示線寬、箭頭、單向／雙向、線型、虛線、間隔。

- **另存新檔**（Ctrl+S）：輸出包含所有註記的完整 PDF。
- **匯出頁面 / 加密**（Ctrl+E）：指定頁碼（例如 `1,3,5-8`）另存新檔，可用 AES-256 加密；
  可設開啟密碼、權限密碼，以及是否允許列印、複製、修改。
- **列印**（Ctrl+P）／**列印預覽**（Ctrl+Shift+P）：系統列印對話框可選頁面範圍（全部、目前頁、指定頁碼）、份數、印表機與紙張、彩色／黑白；
  列印內容含所有註記（與另存新檔結果相同），超出紙張時等比縮小，橫向頁面在直式紙張上自動旋轉。
- **側邊欄**（F4）：視窗左側一整欄，可切換 **頁面**（所有頁面縮圖，點選跳頁、目前頁自動標示）與
  **書籤**（PDF 大綱樹狀顯示，點選跳頁、可展開／收合、標示目前章節）；從「檢視」選單選擇是否顯示，設定自動記住。
- **尺規**（Ctrl+R）：頁面上方與左側顯示刻度，原點為頁面左上角，單位可選公釐／公分／英吋／點（點尺規左上角切換），
  滑鼠位置標示在尺規上並於狀態列顯示座標；僅供對位，不會寫入 PDF 或列印。
- **檔案總管整合**（安裝時可選）：檔案總管預覽窗格（Alt+P）可捲動瀏覽 PDF 所有頁面，大圖示檢視以第 1 頁作為縮圖；
  安裝時備份原本的 PDF 預覽／縮圖程式，解除安裝時還原。外掛為獨立的 `SimplePdfPreview.dll`（見 `crates/shellext/`）。
- 復原 / 取消復原（Ctrl+Z / Ctrl+Y）、Ctrl+滾輪縮放、PgUp/PgDn 換頁、拖放 PDF 開啟、開啟加密 PDF。
- Ctrl+C 複製最後一次高亮／底線所選取的文字。
- **說明**（F1，或工具列右側「說明」按鈕）：詳細使用說明；「說明 → 關於」顯示版本與授權資訊。
- **工具列圖示**（檢視 → 工具列圖示）：四種圖示風格——彩色、線條、實心、雙色，會依系統深色／淺色自動調整；
  按鈕顯示方式可選圖示＋下方文字、圖示＋右側文字、只顯示圖示、只顯示文字。設定自動記住（`%APPDATA%\SimplePdfEditor\settings.ini`）。

## 儲存方式

編輯期間註記只存在記憶體，儲存時才以原始檔的**乾淨副本**寫入，原檔不會被改動：

- 高亮、底線、線條、箭頭、矩形、圓形 → 標準 PDF 註解（Highlight / Underline / Line / Square / Circle），其他閱讀器仍可編輯或刪除。
  外觀串流由程式自行繪製（高亮以相乘混色、線條支援可調箭頭大小與虛線），存檔結果與編輯畫面一致。
- 文字 → 寫入頁面內容（Form XObject）並嵌入字型**子集**（Type0／Identity-H，只含用到的字形，附 ToUnicode 可複製搜尋）；
  外框與背景一併畫入，位置與編輯畫面相同。
- 加密使用 PDF 2.0 的 AES-256（V5 / R6）。

## 建置

需要 Rust（rustup，`x86_64-pc-windows-msvc`，1.92 以上）與 Visual Studio 2022 Build Tools（MSVC 連結器、rc.exe）。
所有相依套件由 Cargo 自動下載；`vendor/` 內為 PdfCraft 修補過的 hayro 與 lopdf。

```powershell
cargo build --release --workspace
```

產出：`target\release\SimplePdfEditor.exe`、`target\release\SimplePdfPreview.dll`、`target\release\shellext_test.exe`。

自動測試：

```powershell
cargo test --release --workspace                                       # 核心：加上各種註記、存檔、加密匯出並重新開啟驗證；介面：不開視窗以 egui_kittest 點擊按鈕、拖曳畫框，驗證編輯／取消編輯與各工具
target\release\SimplePdfEditor.exe --selftest test\sample.pdf test\out  # 同上（不開視窗），另輸出 preview.png 與存檔結果 full_p1.png 供比對
target\release\shellext_test.exe target\release\SimplePdfPreview.dll test\sample.pdf test\out\shell  # 外掛縮圖與預覽
```

打包：

```powershell
powershell -ExecutionPolicy Bypass -File publish.ps1                    # 建置並整理 publish\（含煙霧測試、檢查不依賴 VC++ 執行環境）
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" SimplePdfEditor_Setup.iss   # 產生 dist\SimplePdfEditor_Setup_v2.2.1.exe
```

安裝程式沿用 v1.x 的 AppId，可直接覆蓋升級，並移除舊版的 Qt／MuPDF DLL。

## 程式結構

| 路徑 | 內容 |
|------|------|
| `crates/pdfcore/src/marks.rs` | 註記資料模型、線條／箭頭幾何、復原堆疊（座標單位為 pt） |
| `crates/pdfcore/src/document.rs` | 開檔（含密碼）、渲染、文字選取（hayro） |
| `crates/pdfcore/src/text.rs` | 文字擷取與行排序（改寫自 PdfCraft） |
| `crates/pdfcore/src/fonts.rs` | 字型載入、度量與文字排版（預覽與存檔共用） |
| `crates/pdfcore/src/save.rs` | 套用註記、字型子集嵌入、頁面重排、AES-256 加密（lopdf） |
| `crates/pdfcore/src/raster.rs` | 以 tiny-skia 繪製註記（文字預覽、高亮相乘混色、測試預覽圖） |
| `crates/app/src/app.rs` | 主視窗狀態與動作（開檔、存檔、匯出、列印、復原） |
| `crates/app/src/view.rs` | 頁面檢視、各工具的滑鼠操作、註記繪製 |
| `crates/app/src/bars.rs` | 選單、主工具列、屬性工具列、狀態列、快捷鍵 |
| `crates/app/src/dialogs.rs` | 文字輸入、密碼、匯出 / 加密、未儲存確認等對話框 |
| `crates/app/src/icons.rs` | 程式繪製的工具列圖示（四種風格） |
| `crates/app/src/ruler.rs` | 頁面尺規（刻度密度隨縮放自動調整） |
| `crates/app/src/print.rs`、`preview.rs` | Win32 列印與列印預覽 |
| `crates/app/src/sidebar.rs` | 左側欄：頁面縮圖與書籤 |
| `crates/app/src/help.rs` | 使用說明與關於視窗 |
| `crates/shellext/` | 檔案總管外掛：預覽處理常式、縮圖提供者（COM DLL）與測試程式 |
| `vendor/` | PdfCraft 修補過的 hayro、lopdf（見 `vendor/README.md`） |

## 與 C++ 版的差異

- PDF 引擎由 MuPDF 改為純 Rust 的 hayro（渲染）與 lopdf（存檔），介面由 Qt 改為 egui；不再需要 vcpkg、Qt 與 VC++ 執行環境。
- 文字選取依「最接近的字」決定起訖（MuPDF 為依行內位置），一般拖曳選取結果相同。
- 列印預覽以預設印表機的紙張顯示；直向／橫向切換只影響預覽，實際列印方向請在列印對話框的「喜好設定」中選擇。
- 開發用的 `--icons`、`--snapshot`、`--printtest`、`--appicons` 參數未移植；程式圖示沿用 `icons/app.ico`、`icons/app.png`。

## 授權

Copyright (C) 2026 Fufeng Project

本專案以 **MIT 授權**或 **Apache License 2.0** 雙授權釋出（`MIT OR Apache-2.0`），可任選其一：

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

中英文說明見 [NOTICE.txt](NOTICE.txt)。可自由使用、修改、散布，包含商業與閉源用途，只需保留著作權與授權聲明；
本程式不提供任何擔保。第三方元件（全部為 MIT／Apache-2.0／BSD 等寬鬆授權，沒有 GPL／AGPL 元件）清單見
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)，授權原文見 [`third_party_licenses/`](third_party_licenses/)。

v1.x（C++／Qt／MuPDF 版）因靜態連結 MuPDF 而採用 AGPL-3.0；自 v2.2.1 起，不再使用 MuPDF 的 Rust 版改為上述寬鬆授權。

### 貢獻

除非另有明確聲明，您提交到本專案的貢獻（依 Apache-2.0 的定義）均以上述 MIT OR Apache-2.0 雙授權釋出，不附加其他條款。
每個原始碼檔案開頭以 SPDX 標示授權：

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project
```
