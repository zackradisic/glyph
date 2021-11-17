# Features

# Scrolling beyond window size should move window to cursor
Just calculate offset based on line number of cursor.

## Cursor movements
* Moving between lines should move cursor to first non-space character of the line.
* Making new lines should make the line and place cursor on the same position, creating
  additional whitespace characters.

## Resizing
Seems simple enough, handle the SDL event and capture the updated size. Use this to
update the global variables

## Visual line
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
