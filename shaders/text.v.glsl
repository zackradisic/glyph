attribute vec4 coord;
attribute float y_translate;
attribute float x_translate;
varying vec2 texpos;

void main(void) {
  mat4 aMat4 = mat4(1.0, 0.0, 0.0,  x_translate, 
                    0.0, 1.0, 0.0,  y_translate * -1.0, 
                    0.0, 0.0, 1.0,  0.0,  
                    0.0, 0.0, 0.0,  1.0);
  
  gl_Position = vec4(coord.xy, 0, 1) * aMat4;
  texpos = coord.zw;
}
