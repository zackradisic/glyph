attribute vec4 coord;
varying vec2 texpos;

void main(void) {
  gl_Position = vec4(coord.xy, 0, 1);
  texpos = coord.zw;
}
