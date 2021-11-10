# Features

## Scrolling
The main design decision here is how granular we want the scrolling to be. If we
want it to be on a line/char basis, then it is pretty simple because then we can
just write code to skip lines/columns in the main text render loop.

If we want more smoother scrolling, then we need to perform x and y translations
on the vertex shader

## Color/Syntax highlighting
Probably requires us to input the RGBA colors for each character along with vertex information.
I think in the fragment shader right now it's getting the color from a single uniform `color`,
so we need to pass in a vector of colors.

I think this is how Jamie's code works, so I'm not too worried about performance. As far as time/space complexity goes, it's probably not too bad
since just storing 4 additional RGBA values. We don't even need the precision of f32s, we can just make them 
u8s too.
