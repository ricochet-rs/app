//! Basecoat's semantic tokens with the values from ricochet's `ricochet-ui/style/theme.css`.

use gpui::{App, Hsla, Pixels, Rgba, Window};
use gpui_component::{ActiveTheme, Theme};

/// A colour written as the same OKLCH triple ricochet's `theme.css` uses.
#[derive(Clone, Copy)]
pub struct Oklch(pub f32, pub f32, pub f32);

impl Oklch {
    #[allow(clippy::excessive_precision)] // the published Oklab matrix, kept digit for digit
    pub fn color(self) -> Hsla {
        let Self(lightness, chroma, hue) = self;
        let radians = hue.to_radians();
        let (a, b) = (chroma * radians.cos(), chroma * radians.sin());
        let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
        let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
        let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powi(3);
        let red = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        let green = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        let blue = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;
        let encode = |linear: f32| {
            let clamped = linear.clamp(0.0, 1.0);
            if clamped <= 0.0031308 {
                12.92 * clamped
            } else {
                1.055 * clamped.powf(1.0 / 2.4) - 0.055
            }
        };
        Rgba {
            r: encode(red),
            g: encode(green),
            b: encode(blue),
            a: 1.0,
        }
        .into()
    }
}

/// One token's light and dark values, as `:root` and `.dark` give them in `theme.css`.
struct Token {
    light: Oklch,
    dark: Oklch,
}

impl Token {
    const fn new(light: Oklch, dark: Oklch) -> Self {
        Self { light, dark }
    }

    fn pick(&self, dark: bool) -> Hsla {
        if dark {
            self.dark.color()
        } else {
            self.light.color()
        }
    }
}

const BACKGROUND: Token = Token::new(Oklch(1.0, 0.0, 0.0), Oklch(0.1448, 0.015, 330.91));
const FOREGROUND: Token = Token::new(Oklch(0.145, 0.015, 343.46), Oklch(0.9851, 0.0, 0.0));
const SIDEBAR: Token = Token::new(Oklch(1.0, 0.0, 0.0), Oklch(0.2046, 0.012, 330.91));
const CARD: Token = Token::new(Oklch(1.0, 0.0, 0.0), Oklch(0.2046, 0.012, 330.91));
const PRIMARY: Token = Token::new(Oklch(0.205, 0.025, 343.46), Oklch(0.8561, 0.0681, 301.14));
const PRIMARY_FOREGROUND: Token = Token::new(Oklch(0.985, 0.0, 0.0), Oklch(0.1448, 0.015, 330.91));
const MUTED: Token = Token::new(Oklch(0.97, 0.005, 343.46), Oklch(0.2686, 0.01, 330.91));
const MUTED_FOREGROUND: Token = Token::new(Oklch(0.5, 0.02, 343.46), Oklch(0.709, 0.0, 0.0));
const ACCENT: Token = Token::new(Oklch(0.97, 0.005, 343.46), Oklch(0.3715, 0.02, 330.91));
const DESTRUCTIVE: Token = Token::new(Oklch(0.577, 0.245, 27.325), Oklch(0.7022, 0.1892, 22.2279));
const SUCCESS: Token = Token::new(Oklch(0.48, 0.13, 160.0), Oklch(0.8, 0.12, 160.0));
const WARNING: Token = Token::new(Oklch(0.68, 0.17, 60.0), Oklch(0.82, 0.15, 75.0));
const BORDER: Token = Token::new(Oklch(0.9, 0.008, 343.46), Oklch(0.2768, 0.012, 330.91));
const INPUT: Token = Token::new(Oklch(0.9, 0.008, 343.46), Oklch(0.325, 0.01, 330.91));
const RING: Token = Token::new(Oklch(0.5441, 0.1579, 320.0), Oklch(0.8561, 0.0681, 301.14));

/// Follows the system appearance, then paints gpui-component's theme with ricochet's tokens and square corners.
pub fn apply(window: Option<&mut Window>, cx: &mut App) {
    Theme::sync_system_appearance(window, cx);
    let theme = Theme::global_mut(cx);
    let dark = theme.is_dark();
    let c = &mut theme.colors;
    c.background = BACKGROUND.pick(dark);
    c.foreground = FOREGROUND.pick(dark);
    c.sidebar = SIDEBAR.pick(dark);
    c.sidebar_foreground = FOREGROUND.pick(dark);
    c.sidebar_border = BORDER.pick(dark);
    c.sidebar_accent = ACCENT.pick(dark);
    c.sidebar_accent_foreground = FOREGROUND.pick(dark);
    c.popover = CARD.pick(dark);
    c.popover_foreground = FOREGROUND.pick(dark);
    c.primary = PRIMARY.pick(dark);
    c.primary_hover = PRIMARY.pick(dark).opacity(0.9);
    c.primary_active = PRIMARY.pick(dark).opacity(0.8);
    c.primary_foreground = PRIMARY_FOREGROUND.pick(dark);
    c.secondary = MUTED.pick(dark);
    c.secondary_hover = ACCENT.pick(dark);
    c.secondary_active = ACCENT.pick(dark);
    c.secondary_foreground = FOREGROUND.pick(dark);
    c.muted = MUTED.pick(dark);
    c.muted_foreground = MUTED_FOREGROUND.pick(dark);
    c.accent = ACCENT.pick(dark);
    c.accent_foreground = FOREGROUND.pick(dark);
    c.danger = DESTRUCTIVE.pick(dark);
    c.danger_hover = DESTRUCTIVE.pick(dark).opacity(0.9);
    c.danger_active = DESTRUCTIVE.pick(dark).opacity(0.8);
    c.danger_foreground = PRIMARY_FOREGROUND.light.color();
    c.success = SUCCESS.pick(dark);
    c.warning = WARNING.pick(dark);
    c.border = BORDER.pick(dark);
    c.input = INPUT.pick(dark);
    c.ring = RING.pick(dark);
    c.link = RING.pick(dark);
    c.title_bar = BACKGROUND.pick(dark);
    c.title_bar_border = BORDER.pick(dark);
    theme.radius = Pixels::ZERO;
    theme.radius_lg = Pixels::ZERO;
    theme.shadow = false;
}

/// ricochet's tokens that gpui-component has no field for.
pub struct Palette;

impl Palette {
    pub fn card(cx: &App) -> Hsla {
        CARD.pick(cx.theme().is_dark())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_and_black_convert_exactly() {
        let white = Oklch(1.0, 0.0, 0.0).color();
        assert!((white.l - 1.0).abs() < 1e-3, "{white:?}");
        let black = Oklch(0.0, 0.0, 0.0).color();
        assert!(black.l.abs() < 1e-3, "{black:?}");
    }
}
