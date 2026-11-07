//! CaskaydiaCove Nerd Font, compiled into the binary (no external files).
//!
//! It becomes the **primary** family for both `Proportional` and `Monospace`;
//! egui's bundled fonts stay behind it as fallback (emoji, anything Cascadia
//! lacks, e.g. `U+22A0`). The whole Nerd Fonts range comes along, so UI
//! strings can use `\u{F0EB}`-style glyphs without another font.
//!
//! Base text size is 14 px (egui's default is 12.5) — the owner's choice.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

static CASKAYDIA: &[u8] = include_bytes!("../assets/CaskaydiaCoveNerdFontMono-Regular.ttf");
const FAMILY: &str = "caskaydia";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(FAMILY.to_owned(), FontData::from_static(CASKAYDIA));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        // Index 0 = primary; the bundled defaults remain as fallback entries.
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, FAMILY.to_owned());
    }
    ctx.set_fonts(fonts);

    // Call after the theme: this clones the current style (Mocha visuals) and
    // only replaces the text sizes.
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style(style);
}
