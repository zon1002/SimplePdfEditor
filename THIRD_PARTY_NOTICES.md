# 第三方元件授權（Third-party notices）

簡易 PDF 編輯器（Rust 版）本身以 **AGPL-3.0-or-later** 授權（見 [LICENSE](LICENSE)）。
執行檔 `SimplePdfEditor.exe` 與檔案總管外掛 `SimplePdfPreview.dll` 靜態連結下列 Rust 套件，
全部為與 AGPL-3.0 相容的寬鬆授權（MIT、Apache-2.0、BSD、Zlib、ISC、BSL-1.0、Unicode-3.0 等）；
多重授權（`A OR B`）的套件，本程式選用其中的寬鬆授權（例如 `Apache-2.0 OR GPL-2.0-only` 選用 Apache-2.0）。

各套件的授權原文（取自套件本身的 LICENSE / COPYING / NOTICE 檔）彙整於
[`third_party_licenses/rust-crates-licenses.txt`](third_party_licenses/rust-crates-licenses.txt)。

## 主要元件

| 元件 | 用途 | 授權 |
|------|------|------|
| [PdfCraft](https://github.com/storytold/pdfcraft) | 參考架構；`crates/pdfcore/src/text.rs` 改寫自其文字擷取模組；`vendor/` 中 hayro、lopdf 的修補版本取自 PdfCraft | MIT OR Apache-2.0 |
| [hayro](https://github.com/LaurenzV/hayro)（hayro-syntax、hayro-interpret、hayro-jbig2） | PDF 解析、渲染、文字擷取 | Apache-2.0 OR MIT |
| [lopdf](https://github.com/J-F-Liu/lopdf) | PDF 修改、存檔、AES-256 加密 | MIT |
| [egui / eframe](https://github.com/emilk/egui) | 使用者介面 | MIT OR Apache-2.0 |
| [wgpu](https://github.com/gfx-rs/wgpu)、glow | 繪圖後端（DirectX 12／OpenGL） | MIT OR Apache-2.0 |
| [tiny-skia](https://github.com/linebender/tiny-skia) | 文字註記預覽、圖示繪製 | BSD-3-Clause |
| [subsetter](https://github.com/typst/subsetter) | 嵌入字型子集 | MIT OR Apache-2.0 |
| [ttf-parser](https://github.com/harfbuzz/ttf-parser)、skrifa | 字型解析 | MIT OR Apache-2.0 |
| [rfd](https://github.com/PolyMeilex/rfd) | 開啟／存檔對話框 | MIT |
| [windows-rs](https://github.com/microsoft/windows-rs) | Win32 API（列印、檔案總管外掛） | MIT OR Apache-2.0 |
| egui 內建字型（epaint_default_fonts） | Ubuntu-Light（Ubuntu Font Licence 1.0）、Hack、Noto Emoji（SIL OFL 1.1）、emoji-icon-font（MIT） | 見授權原文 |

介面中文字型使用 Windows 內建的「微軟正黑體」，於執行時從系統字型資料夾讀取，**不隨程式散布**。
新增文字時嵌入 PDF 的字型子集，來自使用者電腦上的字型，其嵌入權限依該字型的授權。

## 完整套件清單（232 個）

| 套件 | 版本 | 授權 |
|------|------|------|
| accesskit | 0.24.1 | MIT OR Apache-2.0 |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aes | 0.9.3 | MIT OR Apache-2.0 |
| ahash | 0.8.12 | MIT OR Apache-2.0 |
| alloc-no-stdlib | 2.0.4 | BSD-3-Clause |
| alloc-stdlib | 0.2.4 | BSD-3-Clause |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| arboard | 3.6.1 | MIT OR Apache-2.0 |
| arrayref | 0.3.9 | BSD-2-Clause |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| ash | 0.38.0+1.3.281 | MIT OR Apache-2.0 |
| bit-set | 0.10.0 | Apache-2.0 OR MIT |
| bit-vec | 0.9.1 | Apache-2.0 OR MIT |
| bitflags | 1.3.2 | MIT/Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 |
| block-padding | 0.4.2 | MIT OR Apache-2.0 |
| brotli | 8.0.4 | BSD-3-Clause AND MIT |
| brotli-decompressor | 5.0.3 | BSD-3-Clause/MIT |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| cbc | 0.2.1 | MIT OR Apache-2.0 |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 |
| cipher | 0.5.2 | MIT OR Apache-2.0 |
| clipboard-win | 5.4.1 | BSL-1.0 |
| codespan-reporting | 0.13.1 | Apache-2.0 |
| color | 0.3.3 | Apache-2.0 OR MIT |
| const-oid | 0.10.2 | Apache-2.0 OR MIT |
| core_detect | 1.0.0 | MIT/Apache-2.0 |
| cpubits | 0.1.1 | MIT OR Apache-2.0 |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib |
| digest | 0.11.3 | MIT OR Apache-2.0 |
| document-features | 0.2.12 | MIT OR Apache-2.0 |
| dpi | 0.1.2 | Apache-2.0 AND MIT |
| ecb | 0.2.1 | MIT OR Apache-2.0 |
| ecolor | 0.36.2 | MIT OR Apache-2.0 |
| eframe | 0.36.2 | MIT OR Apache-2.0 |
| egui | 0.36.2 | MIT OR Apache-2.0 |
| egui-wgpu | 0.36.2 | MIT OR Apache-2.0 |
| egui-winit | 0.36.2 | MIT OR Apache-2.0 |
| egui_glow | 0.36.2 | MIT OR Apache-2.0 |
| either | 1.19.0 | MIT OR Apache-2.0 |
| emath | 0.36.2 | MIT OR Apache-2.0 |
| encoding_rs | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| epaint | 0.36.2 | MIT OR Apache-2.0 |
| epaint_default_fonts | 0.36.2 | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| error-code | 3.4.0 | BSL-1.0 |
| euclid | 0.22.14 | MIT OR Apache-2.0 |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| fax | 0.2.7 | MIT |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| fearless_simd | 0.4.1 | Apache-2.0 OR MIT |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| foldhash | 0.2.0 | Zlib |
| font-types | 0.11.3 | MIT OR Apache-2.0 |
| font-types | 0.12.6 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| glifo | 0.1.1 | Apache-2.0 OR MIT |
| glifo | 0.2.0 | Apache-2.0 OR MIT |
| glow | 0.17.0 | MIT OR Apache-2.0 OR Zlib |
| glutin | 0.32.3 | Apache-2.0 |
| glutin-winit | 0.5.0 | MIT |
| glutin_egl_sys | 0.7.1 | Apache-2.0 |
| glutin_wgl_sys | 0.6.1 | Apache-2.0 |
| gpu-allocator | 0.28.0 | MIT OR Apache-2.0 |
| guillotiere | 0.7.0 | MIT/Apache-2.0 |
| half | 2.7.1 | MIT OR Apache-2.0 |
| harfrust | 0.12.0 | MIT |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| hayro | 0.7.1 | Apache-2.0 OR MIT |
| hayro-ccitt | 0.3.0 | Apache-2.0 OR MIT |
| hayro-cmap | 0.1.0 | Apache-2.0 OR MIT |
| hayro-interpret | 0.7.0 | Apache-2.0 OR MIT |
| hayro-jbig2 | 0.3.0 | Apache-2.0 OR MIT |
| hayro-jpeg2000 | 0.3.5 | Apache-2.0 OR MIT |
| hayro-postscript | 0.1.0 | Apache-2.0 OR MIT |
| hayro-syntax | 0.7.2 | Apache-2.0 OR MIT |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 |
| image | 0.25.10 | MIT OR Apache-2.0 |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| inout | 0.2.2 | MIT OR Apache-2.0 |
| itertools | 0.15.0 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| khronos-egl | 6.0.0 | MIT/Apache-2.0 |
| kurbo | 0.13.1 | Apache-2.0 OR MIT |
| libc | 0.2.190 | MIT OR Apache-2.0 |
| libloading | 0.8.9 | ISC |
| libm | 0.2.16 | MIT |
| linebender_resource_handle | 0.1.1 | Apache-2.0 OR MIT |
| litrs | 1.0.1 | MIT OR Apache-2.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| lopdf | 0.45.0 | MIT |
| md-5 | 0.11.0 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT |
| naga | 30.0.1 | MIT OR Apache-2.0 |
| naga-types | 30.0.1 | MIT OR Apache-2.0 |
| nohash-hasher | 0.2.0 | Apache-2.0 OR MIT |
| nom | 8.0.0 | MIT |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| ordered-float | 5.5.0 | MIT |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| peniko | 0.6.1 | Apache-2.0 OR MIT |
| phf | 0.13.1 | MIT |
| phf_generator | 0.13.1 | MIT |
| phf_macros | 0.13.1 | MIT |
| phf_shared | 0.13.1 | MIT |
| pic-scale | 0.7.12 | BSD-3-Clause OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| png | 0.17.16 | MIT OR Apache-2.0 |
| png | 0.18.1 | MIT OR Apache-2.0 |
| pollster | 1.0.1 | Apache-2.0/MIT |
| polycool | 0.4.0 | MIT OR Apache-2.0 |
| presser | 0.3.1 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| profiling | 1.0.18 | MIT OR Apache-2.0 |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| quick-error | 2.0.1 | MIT/Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| rand | 0.10.3 | MIT OR Apache-2.0 |
| rand_core | 0.10.1 | MIT OR Apache-2.0 |
| range-alloc | 0.1.5 | MIT OR Apache-2.0 |
| rangemap | 1.8.0 | MIT/Apache-2.0 |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| read-fonts | 0.39.2 | MIT OR Apache-2.0 |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 |
| read-fonts | 0.44.0 | MIT OR Apache-2.0 |
| renderdoc-sys | 1.1.0 | MIT OR Apache-2.0 |
| rfd | 0.17.2 | MIT |
| rustc-hash | 1.1.0 | Apache-2.0/MIT |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| self_cell | 1.3.0 | Apache-2.0 OR GPL-2.0-only |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| sha2 | 0.11.1 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 |
| siphasher | 1.0.4 | MIT OR Apache-2.0 |
| skrifa | 0.42.1 | MIT OR Apache-2.0 |
| skrifa | 0.44.0 | MIT OR Apache-2.0 |
| skrifa | 0.47.0 | MIT OR Apache-2.0 |
| smallvec | 1.16.3 | MIT OR Apache-2.0 |
| smol_str | 0.2.2 | MIT OR Apache-2.0 |
| spirv | 0.4.0+sdk-1.4.341.0 | Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 |
| strict-num | 0.1.1 | MIT |
| stringprep | 0.1.5 | MIT/Apache-2.0 |
| subsetter | 0.2.6 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| synstructure | 0.14.0 | MIT |
| termcolor | 1.4.1 | Unlicense OR MIT |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| tiff | 0.11.3 | MIT |
| tiny-skia | 0.11.4 | BSD-3-Clause |
| tiny-skia-path | 0.11.4 | BSD-3-Clause |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| tracing | 0.1.44 | MIT |
| tracing-core | 0.1.36 | MIT |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 |
| type-map | 0.5.1 | MIT/Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 |
| unicode-general-category | 1.1.0 | Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-normalization | 0.1.25 | MIT OR Apache-2.0 |
| unicode-properties | 0.1.4 | MIT/Apache-2.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| unicode-width | 0.2.2 | MIT OR Apache-2.0 |
| uuid | 1.28.0 | Apache-2.0 OR MIT |
| vello_common | 0.0.8 | Apache-2.0 OR MIT |
| vello_common | 0.0.9 | Apache-2.0 OR MIT |
| vello_common | 0.1.0 | Apache-2.0 OR MIT |
| vello_cpu | 0.0.8 | Apache-2.0 OR MIT |
| vello_cpu | 0.1.0 | Apache-2.0 OR MIT |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
| weezl | 0.1.12 | MIT OR Apache-2.0 |
| weezl | 0.2.1 | MIT OR Apache-2.0 |
| wgpu | 30.0.1 | MIT OR Apache-2.0 |
| wgpu-core | 30.0.1 | MIT OR Apache-2.0 |
| wgpu-core-deps-windows-linux-android | 30.0.1 | MIT OR Apache-2.0 |
| wgpu-hal | 30.0.1 | MIT OR Apache-2.0 |
| wgpu-naga-bridge | 30.0.1 | MIT OR Apache-2.0 |
| wgpu-types | 30.0.1 | MIT OR Apache-2.0 |
| winapi-util | 0.1.11 | Unlicense OR MIT |
| windows | 0.62.2 | MIT OR Apache-2.0 |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 |
| windows-core | 0.62.2 | MIT OR Apache-2.0 |
| windows-future | 0.3.2 | MIT OR Apache-2.0 |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 |
| windows-result | 0.4.1 | MIT OR Apache-2.0 |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 |
| winit | 0.30.13 | Apache-2.0 |
| write-fonts | 0.48.1 | MIT OR Apache-2.0 |
| yoke | 0.8.3 | Unicode-3.0 |
| yoke-derive | 0.8.4 | Unicode-3.0 |
| zerocopy | 0.8.62 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.62 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zlib-rs | 0.6.8 | Zlib |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib |
