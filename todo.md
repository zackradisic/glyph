# Features

## Resizing
Seems simple enough, handle the SDL event and capture the updated size. Use this to
update the global variables

## Visual line
Keep state on selection, start and pos. On the graphics side of things it seems simple enough, just an outline over the already existing points we create when queuing text.

When we pass through the highlighted text in `queue_text()` when we add additional 
points that get added as highlighted vertexes.

## Color
Probably requires us to input the RGBA colors for each character along with vertex information.
I think in the fragment shader right now it's getting the color from a single uniform `color`,
so we need to pass in a vector of colors.

I think this is how Jamie's code works, so I'm not too worried about performance. As far as time/space complexity goes, it's probably not too bad
since just storing 4 additional RGBA values. We don't even need the precision of f32s, we can just make them 
u8s too.

## Syntax Highlighting
The standard in this domain seems to be [Treesitter](https://github.com/tree-sitter/tree-sitter/tree/master/highlight). More research is necessary but 