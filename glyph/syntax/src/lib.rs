use macros::make_highlights;
use once_cell::sync::Lazy;

pub use tree_sitter;
pub use tree_sitter_highlight;
use tree_sitter_highlight::HighlightConfiguration;
pub use tree_sitter_javascript;
pub use tree_sitter_rust;

make_highlights!(
    "attribute",
    "comment",
    "constant",
    "constructor",
    "function.builtin",
    "function",
    "keyword",
    "label",
    "operator",
    "param",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter"
);

pub static TS_CFG: Lazy<HighlightConfiguration> = Lazy::new(|| {
    let mut cfg = HighlightConfiguration::new(
        tree_sitter_typescript::language_typescript(),
        tree_sitter_typescript::HIGHLIGHT_QUERY,
        "",
        tree_sitter_typescript::LOCALS_QUERY,
    )
    .unwrap();

    cfg.configure(HIGHLIGHTS);

    cfg
});

pub static GO_CFG: Lazy<HighlightConfiguration> = Lazy::new(|| {
    let mut cfg = HighlightConfiguration::new(
        tree_sitter_go::language(),
        tree_sitter_go::HIGHLIGHT_QUERY,
        "",
        "",
    )
    .unwrap();

    cfg.configure(HIGHLIGHTS);

    cfg
});

pub static JS_CFG: Lazy<HighlightConfiguration> = Lazy::new(|| {
    let mut cfg = HighlightConfiguration::new(
        tree_sitter_javascript::language(),
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::INJECTION_QUERY,
        tree_sitter_javascript::LOCALS_QUERY,
    )
    .unwrap();

    cfg.configure(HIGHLIGHTS);

    cfg
});

pub static RUST_CFG: Lazy<HighlightConfiguration> = Lazy::new(|| {
    let mut cfg = HighlightConfiguration::new(
        tree_sitter_rust::language(),
        tree_sitter_rust::HIGHLIGHT_QUERY,
        "",
        "",
    )
    .unwrap();

    cfg.configure(HIGHLIGHTS);

    cfg
});
