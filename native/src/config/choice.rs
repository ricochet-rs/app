/// Declares an enum whose variants are saved as fixed strings and shown with a title.
macro_rules! choice {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => ($raw:literal, $title:literal)),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The string ricochet reads from `ricochet-config.toml`.
            pub fn raw(self) -> &'static str {
                match self {
                    $(Self::$variant => $raw),+
                }
            }

            pub fn title(self) -> &'static str {
                match self {
                    $(Self::$variant => $title),+
                }
            }

            pub fn parse(raw: &str) -> Option<Self> {
                match raw {
                    $($raw => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use choice;
