attribute vec4 coord;
varying vec2 texpos;

void main(void) {
  
  // mat4 aMat4 = mat4(1.0, 0.0, 0.0,  (1.0 / 800.0) * 200.0, 
  //                   0.0, 1.0, 0.0,  0.0, 
  //                   0.0, 0.0, 1.0,  0.0,  
  //                   0.0, 0.0, 0.0,  1.0);

  mat4 aMat4 = mat4(1.0, 0.0, 0.0,  0.0, 
                    0.0, 1.0, 0.0,  0.0, 
                    0.0, 0.0, 1.0,  0.0,  
                    0.0, 0.0, 0.0,  1.0);
  
  gl_Position = vec4(coord.xy, 0, 1) * aMat4;
  texpos = coord.zw;
}
