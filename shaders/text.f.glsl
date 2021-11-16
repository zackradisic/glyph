varying vec2 texpos;
varying vec4 v_color;

uniform sampler2D tex;

void main(void) {
  gl_FragColor = vec4(1, 1, 1, texture2D(tex, texpos).a) * v_color;
}