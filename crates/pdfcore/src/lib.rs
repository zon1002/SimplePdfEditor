// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 簡易 PDF 編輯器核心（不含介面）：開檔、渲染、文字選取、註記資料模型、存檔與加密。

pub mod document;
pub mod fonts;
pub mod geom;
pub mod marks;
pub mod pagerange;
pub mod raster;
pub mod save;
pub mod selftest;
pub mod text;

pub use document::{Document, Image, OpenError};
pub use geom::{Affine, Pt, Quad, Rect, Rgba};
pub use marks::{EditModel, LineGeometry, Mark, MarkList, MarkType, line_geometry};
pub use save::SaveOptions;
