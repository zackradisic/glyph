# Roadmap

1. ~~Implement MVP editing features~~
    * ~~Visual mode~~
    * ~~Undo/redo~~
2. LSP support (at this point Glyph can replace my main editor)
    * Diagnostics
    * Go to definition
    * Code actions
    * Suggestions
    * Hover buffer for peek definition
3. Other niceties
    * File tree
    * Vim surround
    * Tab/status line
    * Git diff & hunk viewer


# Future niceties 
## Resizing
Seems simple enough, handle the SDL event and capture the updated size. Use this to
update the global variables. Not a priority for me, not resizing frequently enough.

## Syntax Highlighting Optimizations
I imagine implementing LSP support may result in performance degradation. Performance with current syntax highlighting is pretty good. If performance optimizations are necessary here are 
a few ideas:

### Incremental Parsing
Need to edit `tree-sitter-highlighter` to accept the previous syntax tree as input to allow incremental parsing.

### Concurrency
Tree-sitter parsing the syntax tree and collecting the corresponding colors for the highlights can be executed in a
separate thread while we calculate vertices for the text in another.