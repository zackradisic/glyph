# Features

## Color/Syntax highlighting
Probably requires us to input the RGBA colors for each character along with vertex information.
I think in the fragment shader right now it's getting the color from a single uniform `color`,
so we need to pass in a vector of colors.

I think this is how Jamie's code works, so I'm not too worried about performance. As far as time/space complexity goes, it's probably not too bad
since just storing 4 additional RGBA values. We don't even need the precision of f32s, we can just make them 
u8s too.

# Cleaning

## Swap window and clear and draw every frame
I think gl::ClearColor and window.gl_swap_window() should be called every time, not only when we
draw text. For example if we have a cursor and it moves, the window will have the previous cursor
positions unless we clear window.

This means we should cache the text and draw it only when we need to.

Apparently you can clear just a texture (see [here](https://stackoverflow.com/questions/35880814/how-to-blank-my-opengl-texture))