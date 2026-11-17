//! How an RDP console trades looks for speed. The browser names one of three
//! profiles (`speed=fast|balanced|sharp`); anything else is `balanced`. Each
//! maps to guacd's RDP performance parameters — names as guacd lists them in
//! its `args` instruction (checked against the installed guacd 1.3.0).

/// The speed profile of one RDP connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Speed {
    /// Least to draw: 16-bit colour, no wallpaper, themes or font smoothing.
    Fast,
    /// Themes and smooth fonts, but nothing that repaints the whole screen.
    #[default]
    Balanced,
    /// Everything on, as the desktop looks when sat at.
    Sharp,
}

impl Speed {
    /// Only the three known names count; anything else is the default.
    pub(super) fn parse(value: Option<&str>) -> Self {
        match value {
            Some("fast") => Speed::Fast,
            Some("sharp") => Speed::Sharp,
            _ => Speed::Balanced,
        }
    }

    /// The value this profile gives a guacd RDP parameter, or None for one it
    /// leaves alone (sent empty, so guacd's default holds — which keeps the
    /// bitmap, glyph and off-screen caches on: `disable-*-caching` stay unset).
    pub(super) fn value(self, name: &str) -> Option<&'static str> {
        let on = |yes: bool| Some(if yes { "true" } else { "false" });
        match name {
            "color-depth" => Some(match self {
                Speed::Fast => "16",
                Speed::Balanced => "24",
                Speed::Sharp => "32",
            }),
            "enable-wallpaper" | "enable-full-window-drag" | "enable-desktop-composition"
            | "enable-menu-animations" => on(self == Speed::Sharp),
            "enable-theming" | "enable-font-smoothing" => on(self != Speed::Fast),
            _ => None,
        }
    }

    /// The DPI to ask for: `fast` always draws at 96 (fewer pixels to send).
    pub(super) fn dpi(self, asked: u32) -> u32 {
        if self == Speed::Fast { 96 } else { asked }
    }
}

/// What the page asks of the display in the tunnel's query: its size (the
/// window, or the fixed size it chose), DPI and speed profile.
pub(super) fn display(query: &str) -> (u32, u32, u32, Speed) {
    let q = |key: &str| super::parse_query_param(query, key);
    let dims = |k: &str, d: u32| q(k).and_then(|v| v.parse().ok()).filter(|n| *n > 0 && *n <= 8000).unwrap_or(d);
    let speed = Speed::parse(q("speed").as_deref());
    (dims("GUAC_WIDTH", 1280), dims("GUAC_HEIGHT", 800), speed.dpi(dims("GUAC_DPI", 96)), speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOKS: [&str; 6] = [
        "enable-wallpaper",
        "enable-theming",
        "enable-font-smoothing",
        "enable-full-window-drag",
        "enable-desktop-composition",
        "enable-menu-animations",
    ];

    fn looks(speed: Speed) -> Vec<&'static str> {
        LOOKS.iter().map(|name| speed.value(name).unwrap()).collect()
    }

    #[test]
    fn fast_turns_every_effect_off_at_16_bit() {
        assert_eq!(Speed::Fast.value("color-depth"), Some("16"));
        assert_eq!(looks(Speed::Fast), ["false"; 6]);
    }

    #[test]
    fn balanced_keeps_themes_and_smooth_fonts_only() {
        assert_eq!(Speed::Balanced.value("color-depth"), Some("24"));
        assert_eq!(looks(Speed::Balanced), ["false", "true", "true", "false", "false", "false"]);
    }

    #[test]
    fn sharp_turns_everything_on_at_32_bit() {
        assert_eq!(Speed::Sharp.value("color-depth"), Some("32"));
        assert_eq!(looks(Speed::Sharp), ["true"; 6]);
    }

    #[test]
    fn no_profile_touches_the_caches_or_anything_else() {
        for speed in [Speed::Fast, Speed::Balanced, Speed::Sharp] {
            for name in ["disable-bitmap-caching", "disable-offscreen-caching", "disable-glyph-caching", "hostname", "password", "dpi"] {
                assert_eq!(speed.value(name), None, "{name}");
            }
        }
    }

    #[test]
    fn only_the_three_names_are_accepted() {
        assert_eq!(Speed::parse(Some("fast")), Speed::Fast);
        assert_eq!(Speed::parse(Some("balanced")), Speed::Balanced);
        assert_eq!(Speed::parse(Some("sharp")), Speed::Sharp);
        for other in ["", "FAST", "fast ", "ultra", "sharp&x", "true"] {
            assert_eq!(Speed::parse(Some(other)), Speed::Balanced, "{other:?}");
        }
        assert_eq!(Speed::parse(None), Speed::Balanced);
    }

    #[test]
    fn the_query_names_the_profile_and_fast_forces_96_dpi() {
        assert_eq!(display("target=a&GUAC_WIDTH=1600&GUAC_HEIGHT=900&GUAC_DPI=144&speed=fast"), (1600, 900, 96, Speed::Fast));
        assert_eq!(display("GUAC_WIDTH=1600&GUAC_HEIGHT=900&GUAC_DPI=144&speed=sharp"), (1600, 900, 144, Speed::Sharp));
        assert_eq!(display("GUAC_DPI=144&speed=nonsense"), (1280, 800, 144, Speed::Balanced));
        assert_eq!(display(""), (1280, 800, 96, Speed::Balanced));
    }
}
