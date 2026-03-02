use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TideArtVariant {
    OceanCurrent,
    HarborFog,
    SakuraTide,
    MatchaGlass,
    LanternEmber,
    MoonlitKoi,
    WinterPlum,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub tide_art: TideArtVariant,
    pub background: Color,
    pub surface: Color,
    pub panel_border: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub highlight: Color,
    pub text: Color,
    pub muted: Color,
    pub gauge_track: Color,
}

pub const THEMES: [Theme; 7] = [
    Theme {
        name: "Ocean Current",
        tide_art: TideArtVariant::OceanCurrent,
        background: Color::Rgb(7, 18, 34),
        surface: Color::Rgb(13, 31, 54),
        panel_border: Color::Rgb(47, 92, 130),
        accent: Color::Rgb(82, 181, 255),
        accent_soft: Color::Rgb(38, 74, 104),
        highlight: Color::Rgb(136, 231, 255),
        text: Color::Rgb(227, 241, 255),
        muted: Color::Rgb(143, 175, 203),
        gauge_track: Color::Rgb(15, 49, 82),
    },
    Theme {
        name: "Harbor Fog",
        tide_art: TideArtVariant::HarborFog,
        background: Color::Rgb(18, 16, 14),
        surface: Color::Rgb(34, 29, 24),
        panel_border: Color::Rgb(105, 91, 74),
        accent: Color::Rgb(242, 167, 83),
        accent_soft: Color::Rgb(77, 56, 32),
        highlight: Color::Rgb(255, 218, 138),
        text: Color::Rgb(248, 237, 222),
        muted: Color::Rgb(193, 172, 146),
        gauge_track: Color::Rgb(61, 45, 28),
    },
    Theme {
        name: "Sakura Tide",
        tide_art: TideArtVariant::SakuraTide,
        background: Color::Rgb(28, 18, 24),
        surface: Color::Rgb(49, 31, 40),
        panel_border: Color::Rgb(158, 103, 127),
        accent: Color::Rgb(244, 151, 184),
        accent_soft: Color::Rgb(89, 53, 70),
        highlight: Color::Rgb(255, 210, 224),
        text: Color::Rgb(255, 242, 245),
        muted: Color::Rgb(214, 177, 190),
        gauge_track: Color::Rgb(74, 43, 57),
    },
    Theme {
        name: "Matcha Glass",
        tide_art: TideArtVariant::MatchaGlass,
        background: Color::Rgb(20, 28, 22),
        surface: Color::Rgb(35, 49, 39),
        panel_border: Color::Rgb(98, 138, 105),
        accent: Color::Rgb(157, 207, 149),
        accent_soft: Color::Rgb(59, 81, 62),
        highlight: Color::Rgb(218, 242, 191),
        text: Color::Rgb(241, 247, 229),
        muted: Color::Rgb(176, 196, 164),
        gauge_track: Color::Rgb(49, 68, 52),
    },
    Theme {
        name: "Lantern Ember",
        tide_art: TideArtVariant::LanternEmber,
        background: Color::Rgb(24, 16, 12),
        surface: Color::Rgb(42, 27, 19),
        panel_border: Color::Rgb(145, 88, 49),
        accent: Color::Rgb(240, 120, 54),
        accent_soft: Color::Rgb(91, 49, 28),
        highlight: Color::Rgb(255, 205, 129),
        text: Color::Rgb(251, 236, 214),
        muted: Color::Rgb(203, 165, 132),
        gauge_track: Color::Rgb(70, 40, 23),
    },
    Theme {
        name: "Moonlit Koi",
        tide_art: TideArtVariant::MoonlitKoi,
        background: Color::Rgb(14, 20, 37),
        surface: Color::Rgb(22, 35, 59),
        panel_border: Color::Rgb(78, 110, 155),
        accent: Color::Rgb(239, 104, 88),
        accent_soft: Color::Rgb(67, 48, 66),
        highlight: Color::Rgb(255, 214, 202),
        text: Color::Rgb(239, 244, 250),
        muted: Color::Rgb(167, 185, 205),
        gauge_track: Color::Rgb(31, 47, 77),
    },
    Theme {
        name: "Winter Plum",
        tide_art: TideArtVariant::WinterPlum,
        background: Color::Rgb(22, 15, 22),
        surface: Color::Rgb(36, 24, 36),
        panel_border: Color::Rgb(113, 83, 107),
        accent: Color::Rgb(181, 130, 162),
        accent_soft: Color::Rgb(67, 45, 66),
        highlight: Color::Rgb(229, 202, 222),
        text: Color::Rgb(243, 236, 242),
        muted: Color::Rgb(181, 165, 181),
        gauge_track: Color::Rgb(55, 37, 55),
    },
];

pub fn find_theme_index(name: &str) -> Option<usize> {
    let needle = normalize(name);

    THEMES
        .iter()
        .position(|theme| normalize(theme.name) == needle)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::find_theme_index;

    #[test]
    fn theme_lookup_accepts_case_and_spacing_variations() {
        assert_eq!(find_theme_index("ocean-current"), Some(0));
        assert_eq!(find_theme_index("harbor fog"), Some(1));
        assert_eq!(find_theme_index("sakura tide"), Some(2));
        assert_eq!(find_theme_index("matcha glass"), Some(3));
        assert_eq!(find_theme_index("lantern-ember"), Some(4));
        assert_eq!(find_theme_index("moonlit koi"), Some(5));
        assert_eq!(find_theme_index("winter plum"), Some(6));
    }
}
