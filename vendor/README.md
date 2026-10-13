# Vendored crates

These directories are copied unchanged from [PdfCraft](https://github.com/storytold/pdfcraft)'s `vendor/` folder
(PdfCraft is MIT OR Apache-2.0). They are permissively licensed crates that PdfCraft carries small patches for —
mostly hardening against malformed or hostile PDFs (allocation caps, recursion guards, saturating arithmetic) plus
a few rendering fixes. Every patch is marked `PdfCraft patch:` in the source. Each directory keeps its upstream
licence files unchanged.

| Crate | Version | Licence |
|---|---|---|
| hayro | 0.7.1 | Apache-2.0 OR MIT |
| hayro-interpret | 0.7.0 | Apache-2.0 OR MIT |
| hayro-syntax | 0.7.2 | Apache-2.0 OR MIT |
| hayro-jbig2 | 0.3.0 | Apache-2.0 OR MIT |
| lopdf | 0.45.0 | MIT |

They are wired in through `[patch.crates-io]` in the workspace `Cargo.toml`. See PdfCraft's
`vendor/README.md` for the per-patch details.
