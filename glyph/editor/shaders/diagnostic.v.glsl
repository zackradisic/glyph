attribute vec3 aPos;
attribute float y_translate;
attribute float x_translate;
attribute vec4 vertex_color;

varying vec4 v_color;

void main() {
  // mat4 aMat4 = mat4(1.0, 0.0, 0.0,  x_translate, 
  //                   0.0, 1.0, 0.0,  (y_translate - (8.0/600.0)) * -1.0, 
  //                   0.0, 0.0, 1.0,  0.0,  
  //                   0.0, 0.0, 0.0,  1.0);

  gl_Position = vec4(aPos.x + x_translate, aPos.y + (y_translate * -1.0) , aPos.z, 1.0);
  v_color = vertex_color;
  // gl_Position = vec4(aPos.xyz, 1.0) * aMat4;
}