# Features

## Color/Syntax highlighting
Probably requires us to input the RGBA colors for each character along with vertex information.
I think in the fragment shader right now it's getting the color from a single uniform `color`,
so we need to pass in a vector of colors.

I think this is how Jamie's code works, so I'm not too worried about performance. As far as time/space complexity goes, it's probably not too bad
since just storing 4 additional RGBA values. We don't even need the precision of f32s, we can just make them 
u8s too.


# Cursor text actions

* Cursor/backspace at start of line
  * Should go to the previous line and delete the current line. In the case of the first line do nothing

* Cursor inserting not at end of line
  * In between text should insert inbetween
  * Beyond text should also work
    * Currently doesn't because it doesn't add, so it spills over into next line