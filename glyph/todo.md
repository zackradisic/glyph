# Roadmap

1. Implement MVP editing features
    * Visual mode
    * Undo/redo
2. LSP support (at this point Glyph can replace my main editor)
3. Other niceties
    * File tree
    * Vim surround
    * Tab/status line
    * Git diff & hunk viewer

## Undo/redo
We have two fundamental text editing operations. Insertion and deletion. Anytime we make one of these operations,
we add it's inverse to the undo stack. Anytime we execute an undo, we add the inverse of the undo to the redo stack.

The redo stack gets invalidated (reset) if we make a new operation.

We also have to consider the granularity of the insertion/deletion operations. I like VScode's solution, which 
is each typed word (any character separated by space) counts as one insertion operation. Likewise for deletion. For
example if I type this in vscode:
```
"yo yo yo what's up my glip glops"
```
This is counted as 8 insertion operations. Each undo will undo 1 word.

Nvim treats every edit executed in an insert mode "session" as one operation. For example if I enter insert mode in
nvim and type this sentence, then subsequently exit insert mode:
```
Premature optimization is the root of all evil.
```
This is counted as one insertion operation. If I hit undo, it will undo the entire sentence. I am not fond 
of this behaviour because I like the granularity of undoing on a per word basis. If I wanted to undo the entire
sentence this could easily be achieved through a vim command.


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
