use syntax::Highlight;

use crate::Color;

pub trait Theme {
    fn bg(&self) -> &Color;
    fn fg(&self) -> &Color;
    fn highlight(&self, highlight: Highlight) -> Option<&Color>;
}

macro_rules! define_theme {
    ($name:ident, $(($color_name:ident, $hex:literal)),*) => {
        #[derive(Clone)]
        pub struct $name {
            $(
                $color_name: Color,
            )*
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    $(
                        $color_name: Color::from_hex($hex),
                    )*
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

define_theme!(
    TokyoNightStorm,
    (fg, "#c0caf5"),
    (fg_dark, "#a9b1d6"),
    (bg, "#24283b"),
    (cyan, "#7dcfff"),
    (green, "#9ece6a"),
    (blue, "#7aa2f7"),
    (blue5, "#89ddff"),
    (blue1, "#2ac3de"),
    (orange, "#ff9e64"),
    (red, "#f7768e"),
    (green1, "#73daca"),
    (comment, "#565f89"),
    (magenta, "#bb9af7")
);

impl Theme for TokyoNightStorm {
    #[inline]
    fn bg(&self) -> &Color {
        &self.bg
    }

    #[inline]
    fn fg(&self) -> &Color {
        &self.fg
    }

    #[inline]
    fn highlight(&self, highlight: Highlight) -> Option<&Color> {
        match highlight {
            Highlight::Attribute => None,
            Highlight::Constant => Some(&self.orange),
            Highlight::Constructor => Some(&self.fg_dark),
            Highlight::Comment => Some(&self.comment),
            Highlight::FunctionBuiltin => None,
            Highlight::Function => Some(&self.blue),
            Highlight::Keyword => Some(&self.cyan),
            Highlight::Label => Some(&self.blue),
            Highlight::Operator => Some(&self.blue5),
            Highlight::Property => None, /* Some(&self.green1) */
            Highlight::Punctuation => None,
            Highlight::PunctuationBracket => Some(&self.fg_dark),
            Highlight::PunctuationDelimiter => Some(&self.blue5),
            Highlight::String => Some(&self.green),
            Highlight::StringSpecial => None,
            Highlight::Tag => None,
            Highlight::Type => Some(&self.blue1),
            Highlight::TypeBuiltin => None,
            Highlight::Variable => None,
            Highlight::VariableBuiltin => Some(&self.red),
            Highlight::VariableParameter => None,
        }
    }
}
