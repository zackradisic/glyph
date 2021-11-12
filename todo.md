# Features

## Subtractive color blending for cursor / using vertex array objects
Right now when the cursor is over a glyph, it completely overwrites the glyph's color, unlike editors like vim and vscode, where the cursor is blended with the glyph's color using subtractive color blending. This allows you to still see the glyph under the cursor.

I tried using some OpenGL APIs to achieve this, but I think we're using a legacy version
that doesn't support this. To upgrade to the old APIs we have to change shaders and 
we have to use Vertex Array Objects or they won't draw.

## Color/Syntax highlighting
Probably requires us to input the RGBA colors for each character along with vertex information.
I think in the fragment shader right now it's getting the color from a single uniform `color`,
so we need to pass in a vector of colors.

I think this is how Jamie's code works, so I'm not too worried about performance. As far as time/space complexity goes, it's probably not too bad
since just storing 4 additional RGBA values. We don't even need the precision of f32s, we can just make them 
u8s too.
