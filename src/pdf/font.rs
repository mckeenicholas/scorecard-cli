use std::fs;
use std::path::Path;

use printpdf::font::{ParsedFont, PdfFontParseWarning};

#[cfg(target_os = "linux")]
const CANDIDATE_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

#[cfg(target_os = "macos")]
const CANDIDATE_FONTS: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/System/Library/Fonts/AppleSDGothicNeo.ttc",
];

#[cfg(target_os = "windows")]
const CANDIDATE_FONTS: &[&str] = &[
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\simsun.ttc",
    "C:\\Windows\\Fonts\\malgun.ttf",
    "C:\\Windows\\Fonts\\meiryo.ttc",
    "C:\\Windows\\Fonts\\yugothr.ttc",
    "C:\\Windows\\Fonts\\arialuni.ttf",
];

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
const CANDIDATE_FONTS: &[&str] = &[];

/// Helper for discovering and loading Unicode fonts.
pub struct FontResolver;

impl FontResolver {
    /// Resolves and loads a font:
    /// 1. If `custom_path` is given, attempts to load that file.
    /// 2. Otherwise, probes platform-specific system font locations.
    ///
    /// Returns `None` if no font could be loaded or parsed.
    pub fn resolve(custom_path: Option<&Path>) -> Option<ParsedFont> {
        if let Some(font) = custom_path.and_then(Self::load_from_path) {
            return Some(font);
        }

        Self::find_system_font()
    }

    /// Loads and parses a font file from the specified path.
    pub fn load_from_path(path: &Path) -> Option<ParsedFont> {
        let bytes = fs::read(path).ok()?;
        let mut warnings: Vec<PdfFontParseWarning> = Vec::new();
        ParsedFont::from_bytes(&bytes, 0, &mut warnings)
    }

    /// Probes system font paths for the current operating system.
    pub fn find_system_font() -> Option<ParsedFont> {
        CANDIDATE_FONTS
            .iter()
            .map(Path::new)
            .filter(|p| p.exists())
            .find_map(Self::load_from_path)
    }
}

#[cfg(test)]
mod tests {
    use printpdf::PdfDocument;
    use printpdf::ops::PdfPage;
    use printpdf::serialize::PdfSaveOptions;
    use printpdf::units::Mm;

    use super::*;

    #[test]
    fn test_font_resolver_detection() {
        if let Some(font) = FontResolver::find_system_font() {
            assert!(
                font.lookup_glyph_index(u32::from('张'))
                    .is_some_and(|gid| gid > 0)
            );
            assert!(
                font.lookup_glyph_index(u32::from('三'))
                    .is_some_and(|gid| gid > 0)
            );
        }
    }

    #[test]
    fn test_cjk_pdf_generation_with_loaded_font() {
        let Some(font) = FontResolver::find_system_font() else {
            return;
        };

        let mut doc = PdfDocument::new("CJK Test");
        let font_id = doc.add_font(&font);

        let mut ops = Vec::new();
        crate::pdf::renderer::ScorecardRenderer::draw_card(
            &mut ops,
            &crate::scorecard::Scorecard::new(
                "CJK Open 2026",
                crate::scorecard::WcaEvent::E333,
                1,
                1,
                crate::scorecard::Competitor {
                    name: "Zhang San",
                    local_name: Some("张三"),
                    registrant_id: std::num::NonZeroUsize::MIN,
                    wca_id: crate::wcif::WcaId::parse("2026ZHAN01"),
                },
            )
            .into(),
            crate::pdf::layout::RectSpec::new(18.0, 18.0, 270.0, 380.0),
            Some(&font_id),
        );

        let page = PdfPage::new(Mm(105.0), Mm(148.0), ops);
        doc.pages = vec![page];

        let mut warnings = Vec::new();
        let mut lopdf_doc = doc.to_lopdf_document(&PdfSaveOptions::default(), &mut warnings);
        let mut pdf_bytes = Vec::new();
        lopdf_doc.save_to(&mut pdf_bytes).unwrap();
        assert!(!pdf_bytes.is_empty());
    }
}
