use syntax::Highlight;

use crate::Color;

pub trait Theme {
    fn bg(&self) -> &Color;
    fn fg(&self) -> &Color;
    fn highlight(&self, highlight: Highlight) -> Option<&Color>;
}

#[derive(Clone)]
pub struct TokyoNightStorm {
    fg: Color,
    bg: Color,
    cyan: Color,
}

impl TokyoNightStorm {
    pub fn new() -> Self {
        Self {
            fg: Color::from_hex("#c0caf5"),
            bg: Color::from_hex("#24283b"),
            cyan: Color::from_hex("#7dcfff"),
        }
    }
}

impl Default for TokyoNightStorm {
    fn default() -> Self {
        Self::new()
    }
}
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
            Highlight::Constant => None,
            Highlight::FunctionBuiltin => None,
            Highlight::Function => None,
            Highlight::Keyword => Some(&self.cyan),
            Highlight::Operator => None,
            Highlight::Property => None,
            Highlight::Punctuation => None,
            Highlight::PunctuationBracket => None,
            Highlight::PunctuationDelimiter => None,
            Highlight::String => None,
            Highlight::StringSpecial => None,
            Highlight::Tag => None,
            Highlight::Type => None,
            Highlight::TypeBuiltin => None,
            Highlight::Variable => None,
            Highlight::VariableBuiltin => None,
            Highlight::VariableParameter => None,
        }
    }
}
