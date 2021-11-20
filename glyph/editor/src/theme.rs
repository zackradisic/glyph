use syntax::Highlight;

use crate::Color;

pub trait Theme {
    fn bg(&self) -> &Color;
    fn fg(&self) -> &Color;
    fn highlight(&self, highlight: Highlight) -> Option<&Color>;
}

macro_rules! define_theme {
    ($name:ident, $(($color_name:ident $hex:literal)),*) => {
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
    (fg "#c0caf5"),
    (fg_dark "#a9b1d6"),
    (bg "#24283b"),
    (cyan "#7dcfff"),
    (green "#9ece6a"),
    (blue "#7aa2f7"),
    (blue5 "#89ddff"),
    (blue1 "#2ac3de"),
    (orange "#ff9e64"),
    (red "#f7768e"),
    (green1 "#73daca"),
    (comment "#565f89"),
    (magenta "#bb9af7"),
    (yellow "#e0af68")
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
            Highlight::Keyword => Some(&self.magenta),
            Highlight::Label => Some(&self.blue),
            Highlight::Operator => Some(&self.blue5),
            Highlight::Property => None, /* Some(&self.green1) */
            Highlight::Param => Some(&self.yellow),
            Highlight::Punctuation => None,
            Highlight::PunctuationBracket => Some(&self.fg_dark),
            Highlight::PunctuationDelimiter => Some(&self.blue5),
            Highlight::PunctuationSpecial => Some(&self.cyan),
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

define_theme!(
    GithubDark,
    (bg "#0d1117"),
    (fg_dark "#4d5566"),
    (fg "#c9d1d9"),
    (comment "#8b949e"),
    (constant "#79c0ff"),
    (string "#A5D6FF"),
    (func "#d2a8ff"),
    (func_param "#c9d1d9"),
    (variable "#FFA657"),
    (keyword "#ff7b72")
);

impl Theme for GithubDark {
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
            Highlight::Constant => Some(&self.constant),
            Highlight::Constructor => Some(&self.fg),
            Highlight::Comment => Some(&self.comment),
            Highlight::FunctionBuiltin => None,
            Highlight::Function => Some(&self.func),
            Highlight::Keyword => Some(&self.keyword),
            // Highlight::Label => Some(&self.blue),
            Highlight::Operator => Some(&self.keyword),
            Highlight::Property => Some(&self.fg),
            Highlight::Punctuation => None,
            Highlight::PunctuationBracket => Some(&self.fg_dark),
            Highlight::PunctuationDelimiter => Some(&self.keyword),
            Highlight::PunctuationSpecial => Some(&self.variable),
            Highlight::String => Some(&self.string),
            Highlight::StringSpecial => None,
            Highlight::Tag => None,
            Highlight::Type => Some(&self.variable),
            Highlight::TypeBuiltin => None,
            Highlight::Variable => Some(&self.variable),
            Highlight::VariableBuiltin => Some(&self.keyword),
            Highlight::VariableParameter => None,
            _ => None,
        }
    }
}
