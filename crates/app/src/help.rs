// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 使用說明與關於視窗。
//! 內文以簡單標記撰寫：**粗體**、`程式碼`。

use egui::text::LayoutJob;
use egui::{Color32, FontId, RichText, TextFormat, Ui};

enum Block {
    P(&'static str),
    H4(&'static str),
    Ul(&'static [&'static str]),
    /// 巢狀清單：前導文字與子項目
    UlNested(&'static str, &'static [&'static str]),
    Ol(&'static [&'static str]),
    Note(&'static str),
    Table(&'static [[&'static str; 4]]),
}

struct Section {
    title: &'static str,
    blocks: &'static [Block],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "快速開始",
        blocks: &[
            Block::Ol(&[
                "按 **開啟**（Ctrl+O）選擇 PDF，或直接把 PDF 檔**拖曳**到視窗中。加密的 PDF 會先詢問密碼。",
                "在工具列選擇工具（高亮、底線、文字、線條、箭頭），在頁面上拖曳或點擊即可新增註記。",
                "用 **選取** 工具（V）點選已加入的註記，可拖曳移動、改顏色或樣式、按 Delete 刪除。",
                "完成後按 **另存新檔**（Ctrl+S）輸出完整 PDF；或按 **匯出頁面 / 加密**（Ctrl+E）只輸出部分頁面並加上密碼。",
            ]),
            Block::Note("編輯期間**不會修改原檔**，一律另存為新檔案。"),
        ],
    },
    Section {
        title: "選取工具（V）",
        blocks: &[Block::Ul(&[
            "**點選**註記即可選取（藍色虛線框）；按住 Ctrl 點選可多選。",
            "**拖曳**選取的註記可移動位置。",
            "選取後在屬性工具列**更改顏色、字型、大小、線寬、箭頭、線型**等，會立即套用到選取的註記。",
            "**Delete** 或 Backspace 刪除選取的註記。",
            "**雙擊文字**可修改文字內容；把內容清空即刪除該文字。",
        ])],
    },
    Section {
        title: "高亮與底線（H / U）",
        blocks: &[Block::Ul(&[
            "在文字上**拖曳**，會像一般閱讀器一樣選取文字並加上高亮或底線，可跨多行。",
            "拖曳前先在「顏色」列選擇顏色；之後也可用選取工具改色。",
            "狀態列會顯示選到的文字，按 **Ctrl+C** 可複製這段文字。",
            "掃描檔或圖片等**沒有文字層**的頁面，會改以拖曳出的矩形範圍標記。",
        ])],
    },
    Section {
        title: "文字（T）",
        blocks: &[
            Block::Ul(&[
                "在頁面上**點一下**，於對話框輸入文字（可多行）。",
                "點既有的文字會開啟編輯。之後可用選取工具拖曳調整位置。",
                "**字型**：內建 Windows 常用字型（微軟正黑體、細明體、標楷體等），選「其他字型檔…」可載入任何 TTF／OTF／TTC。",
                "**大小**：4–200 pt，可輸入小數。",
                "**外框**：勾選後加上方框，可設定外框顏色與粗細。",
                "**背景**：勾選後填滿底色，顏色可設定透明度（例如半透明黃色當便利貼）。",
                "外框與背景和文字之間保留 4 pt 留白。",
            ]),
            Block::Note("存檔時文字會寫入頁面內容並只嵌入用到的字（字型子集），檔案不會變大太多；但存檔後在其他軟體中無法再改字，請在本程式內完成修改。"),
        ],
    },
    Section {
        title: "線條與箭頭（L / A）",
        blocks: &[
            Block::Ul(&[
                "在頁面上**拖曳**畫出線條或箭頭，箭頭畫在放開滑鼠的那一端。按住 **Shift** 可鎖定 0°／45°／90°。",
                "**線寬**：0.5–20 pt。",
                "**箭頭**：箭頭長度 2–120 pt（寬度為長度的一半）。",
                "**單向／雙向**：雙向時起點與終點都有箭頭。",
                "**線型**：實線或虛線；虛線可調整**虛線**（每段長度）與**間隔**。",
            ]),
            Block::Note("線條與箭頭存成標準 PDF 線條註解，Acrobat 等軟體中仍可選取、移動或刪除。"),
        ],
    },
    Section {
        title: "顏色與屬性工具列",
        blocks: &[Block::Ul(&[
            "「顏色」列有 9 個常用色票，按「自訂」色塊可選任意顏色。",
            "每個工具會**各自記住**顏色：例如高亮預設黃色、底線預設紅色、文字預設黑色。",
            "屬性工具列**只顯示目前用得到的設定**：文字工具顯示字型、大小、外框、背景；線條／箭頭顯示線寬、箭頭、線型等；使用選取工具時依選到的註記類型顯示。",
        ])],
    },
    Section {
        title: "另存新檔與匯出加密",
        blocks: &[
            Block::H4("另存新檔（Ctrl+S）"),
            Block::P("輸出包含所有註記的完整 PDF。可直接覆蓋原檔，因為程式已把原檔內容讀入記憶體。"),
            Block::H4("匯出頁面 / 加密（Ctrl+E）"),
            Block::Ul(&[
                "**頁碼範圍**：例如 `3`、`1,3,5-8`；按「目前頁」或「全部」快速填入。頁面依輸入順序輸出。",
                "**包含註記**：取消勾選則輸出原始頁面。",
                "**加密（AES-256）**：設定**開啟密碼**後，開檔時需輸入密碼。",
                "**權限密碼**（選填）：用來保護權限設定；可決定是否允許列印、複製內容、修改與註解。",
            ]),
            Block::Note("權限限制只有在「權限密碼」與「開啟密碼」**不同**時才有效；若相同，知道開啟密碼的人就擁有全部權限。"),
        ],
    },
    Section {
        title: "列印",
        blocks: &[Block::Ul(&[
            "**列印**（Ctrl+P）：開啟系統列印對話框，可選印表機、份數、紙張大小、彩色／黑白（印表機「喜好設定」），以及頁面範圍：**全部**、**目前頁面**或**指定頁碼**。",
            "**列印預覽**（Ctrl+Shift+P）：先看列印結果，可切換紙張方向、縮放，確認後直接列印。",
            "列印內容**包含所有註記**（高亮、底線、文字、外框、線條、箭頭），與「另存新檔」的結果相同。",
            "頁面比紙張大時會等比縮小到可列印範圍；否則以實際大小置中列印。**橫向頁面**印在直式紙張時會自動旋轉。",
            "想輸出成 PDF 檔，可在印表機選「Microsoft Print to PDF」；不過建議直接用「另存新檔」，可保留文字層與註解。",
        ])],
    },
    Section {
        title: "檔案總管整合",
        blocks: &[
            Block::P("安裝時可勾選「**檔案總管整合**」的兩個選項："),
            Block::Ul(&[
                "**預覽窗格**：在檔案總管選取 PDF 後按 **Alt+P**（或「檢視 → 預覽窗格」），右側即顯示 PDF 內容，可用滑鼠滾輪、方向鍵、PgUp／PgDn 捲動瀏覽所有頁面，右上角顯示目前頁碼。",
                "**縮圖**：以「大圖示」等檢視方式瀏覽資料夾時，PDF 會以第 1 頁作為縮圖。",
            ]),
            Block::P("預覽與縮圖都會顯示 PDF 內已有的註解（包括本程式加入的高亮、線條、文字等）。加密且需要密碼的 PDF 不會顯示內容。"),
            Block::Note("安裝時會記住原本的 PDF 預覽／縮圖程式（例如 PDF-XChange、Adobe），解除安裝後自動還原。已經產生過縮圖的檔案，Windows 可能沿用舊縮圖；可用「磁碟清理 → 縮圖」清除快取後重新產生。"),
        ],
    },
    Section {
        title: "檢視與圖示",
        blocks: &[
            Block::Ul(&[
                "**縮放**：Ctrl＋滑鼠滾輪、工具列的放大／縮小，或 Ctrl+0 符合寬度。",
                "**換頁**：PgUp／PgDn、工具列箭頭，或直接輸入頁碼。",
                "**尺規**（Ctrl+R 顯示／隱藏）：頁面上方與左側的刻度尺，原點為頁面左上角，白色區段為頁面範圍；滑鼠位置會以紅線標在尺規上，狀態列同時顯示座標。點尺規左上角的單位可切換**公釐、公分、英吋、點**（也可從「檢視 → 尺規單位」切換）。尺規只用於對位，不會出現在存檔或列印結果中。",
            ]),
            Block::UlNested("**工具列圖示**：「檢視 → 工具列圖示」可選擇圖示風格：", &["線條（簡潔單色）", "實心（粗體醒目）", "彩色（各工具代表色）", "雙色（藍色調）"]),
            Block::P("並可選擇按鈕顯示方式：圖示＋下方文字、圖示＋右側文字、只顯示圖示、只顯示文字。設定會自動記住。"),
        ],
    },
    Section {
        title: "快捷鍵一覽",
        blocks: &[Block::Table(&[
            ["按鍵", "功能", "按鍵", "功能"],
            ["V", "選取", "Ctrl+O", "開啟"],
            ["H", "高亮", "Ctrl+S", "另存新檔"],
            ["U", "底線", "Ctrl+E", "匯出頁面 / 加密"],
            ["T", "文字", "Ctrl+Z", "復原"],
            ["L", "線條", "Ctrl+Y", "取消復原"],
            ["A", "箭頭", "Delete", "刪除選取的註記"],
            ["Shift＋拖曳", "線條鎖定 45°", "Ctrl+C", "複製選取的文字"],
            ["PgUp / PgDn", "上一頁 / 下一頁", "Ctrl+滾輪", "縮放"],
            ["Ctrl+0", "符合寬度", "F1", "使用說明"],
            ["Ctrl+P", "列印", "Ctrl+Shift+P", "列印預覽"],
            ["Ctrl+R", "顯示／隱藏尺規", "", ""],
        ])],
    },
    Section {
        title: "常見問題",
        blocks: &[
            Block::P("**Q：按 Ctrl+Z 沒有反應？**\n若剛剛在「大小」「線寬」等輸入框打字，請先點一下頁面，讓焦點回到頁面再按 Ctrl+Z；或使用工具列的「復原」按鈕。"),
            Block::P("**Q：拖曳高亮時選不到文字？**\n該頁可能是掃描圖片，沒有文字層。此時會改以拖曳的矩形範圍標記。"),
            Block::P("**Q：中文字存檔後變成方塊？**\n請確認所選字型含有中文字（例如 Arial 沒有中文），改用微軟正黑體、細明體或標楷體。"),
            Block::P("**Q：列印出來的字有點模糊？**\n列印時頁面以 300 dpi 點陣輸出，一般文件已足夠清晰；若需要向量品質，請先「另存新檔」，再用 PDF 閱讀器列印該檔案。"),
            Block::P("**Q：加密檔忘記密碼？**\nAES-256 加密無法破解或還原密碼，請妥善保存密碼與原始檔。"),
            Block::P("**Q：可以再修改已存檔 PDF 裡的註記嗎？**\n高亮、底線、線條、箭頭是標準註解，可在 Acrobat 等軟體中修改；新增的文字已寫入頁面內容，無法再編輯。"),
        ],
    },
];

/// 把簡單標記轉成排版：**粗體**、`程式碼`
fn rich(ui: &Ui, text: &str, size: f32, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut bold = false;
    let mut code = false;
    let mut buf = String::new();
    let strong = ui.visuals().strong_text_color();
    let flush = |job: &mut LayoutJob, buf: &mut String, bold: bool, code: bool| {
        if buf.is_empty() {
            return;
        }
        let fmt = TextFormat {
            font_id: if code { FontId::monospace(size) } else { FontId::proportional(size) },
            color: if bold { strong } else { color },
            background: if code { ui.visuals().code_bg_color } else { Color32::TRANSPARENT },
            ..Default::default()
        };
        job.append(buf, 0.0, fmt);
        buf.clear();
    };
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            flush(&mut job, &mut buf, bold, code);
            bold = !bold;
        } else if c == '`' {
            flush(&mut job, &mut buf, bold, code);
            code = !code;
        } else {
            buf.push(c);
        }
    }
    flush(&mut job, &mut buf, bold, code);
    job.wrap.max_width = ui.available_width();
    job
}

fn paragraph(ui: &mut Ui, text: &str) {
    let color = ui.visuals().text_color();
    let job = rich(ui, text, 14.0, color);
    ui.label(job);
}

fn bullet(ui: &mut Ui, marker: &str, text: &str) {
    ui.horizontal_top(|ui| {
        ui.add_sized([22.0, 18.0], egui::Label::new(marker));
        ui.vertical(|ui| paragraph(ui, text));
    });
}

fn show_blocks(ui: &mut Ui, blocks: &[Block]) {
    for b in blocks {
        match b {
            Block::P(t) => paragraph(ui, t),
            Block::H4(t) => {
                ui.add_space(4.0);
                ui.label(RichText::new(*t).strong().size(15.0));
            }
            Block::Ul(items) => {
                for it in *items {
                    bullet(ui, "•", it);
                }
            }
            Block::UlNested(head, items) => {
                bullet(ui, "•", head);
                ui.indent("nested", |ui| {
                    for it in *items {
                        bullet(ui, "◦", it);
                    }
                });
            }
            Block::Ol(items) => {
                for (i, it) in items.iter().enumerate() {
                    bullet(ui, &format!("{}.", i + 1), it);
                }
            }
            Block::Note(t) => {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(250, 204, 21, 46))
                    .stroke(egui::Stroke::new(0.0, Color32::TRANSPARENT))
                    .inner_margin(egui::Margin { left: 10, right: 8, top: 6, bottom: 6 })
                    .show(ui, |ui| {
                        let r = ui.max_rect();
                        ui.painter().vline(r.left() - 8.0, r.top() - 6.0..=r.bottom() + 6.0, egui::Stroke::new(3.0, Color32::from_rgb(0xEA, 0xB3, 0x08)));
                        paragraph(ui, t);
                    });
            }
            Block::Table(rows) => {
                egui::Grid::new(ui.next_auto_id()).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                    for (i, row) in rows.iter().enumerate() {
                        for cell in row {
                            if i == 0 {
                                ui.label(RichText::new(*cell).strong());
                            } else {
                                ui.label(*cell);
                            }
                        }
                        ui.end_row();
                    }
                });
            }
        }
        ui.add_space(6.0);
    }
}

/// 使用說明視窗的狀態
#[derive(Default)]
pub struct HelpState {
    pub open: bool,
    /// 要捲動到的章節
    pub jump: Option<usize>,
    pub current: usize,
}

pub fn show_help(ctx: &egui::Context, st: &mut HelpState) {
    if !st.open {
        return;
    }
    let mut open = !ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Window::new("使用說明")
        .open(&mut open)
        .default_size([900.0, 640.0])
        .min_size([600.0, 400.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Panel::left("help_toc").resizable(false).exact_size(200.0).show(ui, |ui| {
                ui.add_space(4.0);
                for (i, s) in SECTIONS.iter().enumerate() {
                    if ui.selectable_label(st.current == i, RichText::new(s.title).size(14.0)).clicked() {
                        st.current = i;
                        st.jump = Some(i);
                    }
                    ui.add_space(2.0);
                }
            });
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.label(RichText::new("簡易 PDF 編輯器 使用說明").size(22.0).strong());
                    for (i, s) in SECTIONS.iter().enumerate() {
                        ui.add_space(10.0);
                        let r = ui.label(RichText::new(s.title).size(18.0).strong());
                        if st.jump == Some(i) {
                            r.scroll_to_me(Some(egui::Align::TOP));
                            st.jump = None;
                        }
                        ui.separator();
                        show_blocks(ui, s.blocks);
                    }
                });
            });
        });
    if !open {
        st.open = false;
    }
}

/// 在程式資料夾（安裝後）或上層資料夾（開發時的 target\release）尋找授權檔
pub fn find_doc_file(names: &[&str]) -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    for _ in 0..4 {
        for n in names {
            let p = dir.join(n);
            if p.exists() {
                return Some(p);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 依 AGPL 第 0 條「適當的法律聲明」：顯示著作權、無擔保聲明、授權條款與檢視方式。
/// 回傳使用者按下的按鈕：Some(0) 授權條款、Some(1) 第三方授權
pub fn show_about(ctx: &egui::Context, open: &mut bool) -> Option<usize> {
    let mut clicked = None;
    let mut keep = *open;
    egui::Window::new("關於 簡易 PDF 編輯器")
        .open(&mut keep)
        .collapsible(false)
        .resizable(false)
        .default_width(500.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.label(RichText::new(format!("簡易 PDF 編輯器 {APP_VERSION}")).size(18.0).strong());
            ui.add_space(6.0);
            paragraph(ui, "PDF 高亮、底線、文字、線條／箭頭註記，頁面匯出與 AES-256 加密、列印，以及檔案總管預覽整合。");
            paragraph(ui, "Copyright © 2026 Fufeng Project");
            paragraph(ui, "本程式為自由軟體，依 **GNU Affero 通用公眾授權條款第 3 版或（依您的選擇）任何更新版本**（AGPL-3.0-or-later）授權，您可以重新散布及修改。");
            paragraph(ui, "**本程式不提供任何擔保**，在法律允許的範圍內，亦不包含可售性或特定用途適用性的默示擔保。");
            paragraph(ui, "原始碼：請見隨附的 README.md");
            ui.label(
                RichText::new("使用元件：egui / eframe、hayro、lopdf（以上取自 PdfCraft 的修補版本）、tiny-skia、subsetter 等（MIT／Apache-2.0 等寬鬆授權），各第三方元件授權見「第三方授權」。")
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("授權條款").clicked() {
                    clicked = Some(0);
                }
                if ui.button("第三方授權").clicked() {
                    clicked = Some(1);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("關閉").clicked() {
                        *open = false;
                    }
                });
            });
        });
    if !keep {
        *open = false;
    }
    clicked
}
