// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 文字辨識（OCR）：PaddleOCR 模型以 ONNX Runtime 執行。
//! 語言包（引擎、偵測與辨識模型）可在安裝時選擇，或事後下載、從檔案加入。

pub mod engine;
pub mod packs;

pub use engine::{OcrChar, OcrEngine, OcrLine, RgbImage};
pub use packs::{InstalledPack, Manifest, PackInfo, PackKind};
