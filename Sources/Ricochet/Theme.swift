import AppKit
import SwiftUI

/// A colour written as the same OKLCH triple ricochet's `theme.css` uses.
struct OKLCH {
    let lightness: Double
    let chroma: Double
    let hue: Double

    init(_ lightness: Double, _ chroma: Double, _ hue: Double) {
        self.lightness = lightness
        self.chroma = chroma
        self.hue = hue
    }

    var nsColor: NSColor {
        let radians = hue * .pi / 180
        let a = chroma * cos(radians)
        let b = chroma * sin(radians)
        let cube = { (value: Double) in value * value * value }
        let l = cube(lightness + 0.3963377774 * a + 0.2158037573 * b)
        let m = cube(lightness - 0.1055613458 * a - 0.0638541728 * b)
        let s = cube(lightness - 0.0894841775 * a - 1.2914855480 * b)
        let red = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
        let green = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
        let blue = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
        let encode = { (linear: Double) -> Double in
            let clamped = min(max(linear, 0), 1)
            return clamped <= 0.0031308 ? 12.92 * clamped : 1.055 * pow(clamped, 1 / 2.4) - 0.055
        }
        return NSColor(srgbRed: encode(red), green: encode(green), blue: encode(blue), alpha: 1)
    }
}

extension Color {
    /// A colour that follows the system appearance, as `:root` and `.dark` do in `theme.css`.
    init(light: OKLCH, dark: OKLCH) {
        self.init(
            nsColor: NSColor(name: nil) { appearance in
                appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark.nsColor : light.nsColor
            })
    }
}

/// Basecoat's semantic tokens with the values from `ricochet-ui/style/theme.css`.
enum Theme {
    static let background = Color(light: OKLCH(1, 0, 0), dark: OKLCH(0.1448, 0.015, 330.91))
    static let foreground = Color(light: OKLCH(0.145, 0.015, 343.46), dark: OKLCH(0.9851, 0, 0))
    static let sidebar = Color(light: OKLCH(1, 0, 0), dark: OKLCH(0.2046, 0.012, 330.91))
    static let card = Color(light: OKLCH(1, 0, 0), dark: OKLCH(0.2046, 0.012, 330.91))
    static let primary = Color(light: OKLCH(0.205, 0.025, 343.46), dark: OKLCH(0.8561, 0.0681, 301.14))
    static let primaryForeground = Color(light: OKLCH(0.985, 0, 0), dark: OKLCH(0.1448, 0.015, 330.91))
    static let muted = Color(light: OKLCH(0.97, 0.005, 343.46), dark: OKLCH(0.2686, 0.01, 330.91))
    static let mutedForeground = Color(light: OKLCH(0.5, 0.02, 343.46), dark: OKLCH(0.709, 0, 0))
    static let accent = Color(light: OKLCH(0.97, 0.005, 343.46), dark: OKLCH(0.3715, 0.02, 330.91))
    static let destructive = Color(light: OKLCH(0.577, 0.245, 27.325), dark: OKLCH(0.7022, 0.1892, 22.2279))
    static let success = Color(light: OKLCH(0.48, 0.13, 160), dark: OKLCH(0.8, 0.12, 160))
    static let warning = Color(light: OKLCH(0.48, 0.13, 75), dark: OKLCH(0.8, 0.12, 75))
    static let border = Color(light: OKLCH(0.9, 0.008, 343.46), dark: OKLCH(0.2768, 0.012, 330.91))
    static let input = Color(light: OKLCH(0.9, 0.008, 343.46), dark: OKLCH(0.325, 0.01, 330.91))
    static let ring = Color(light: OKLCH(0.5441, 0.1579, 320), dark: OKLCH(0.8561, 0.0681, 301.14))
}
