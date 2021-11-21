# Roadmap

1. Implement MVP editing features
    * Visual mode
    * Finish other Vim movements (`B`, `b`, `E`, `e`, fix `f`/`F`)
    * Undo/redo
2. LSP support (at this point Glyph can replace my main editor)
3. Other niceties
    * File tree
    * Vim surround
    * Tab/status line
    * Git diff & hunk viewer

## Resizing
Seems simple enough, handle the SDL event and capture the updated size. Use this to
update the global variables

## Visual mode
Keep state on selection, start and pos. On the graphics side of things just an outline over the already existing points we create when queuing text seems easy to achieve. The color can be just have the opacity cranked
down a bit.

When we pass through the highlighted text in `queue_text()` we add additional points that get added as highlighted vertexes. 

## Syntax Highlighting Optimizations
Performance with current syntax highlighting is pretty good. If performance optimizations are necessary here are 
a few ideas:

### Incremental Parsing
Need to edit `tree-sitter-highlighter` to accept the previous syntax tree as input to allow incremental parsing.

### Concurrency
Tree-sitter parsing the syntax tree and collecting the corresponding colors for the highlights can be executed in a
separate thread while we calculate vertices for the text in another.
