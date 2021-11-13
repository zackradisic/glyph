#define PERIOD 0.5
#define BLINK_THRESHOLD 0.5

uniform bool is_blinking;
uniform float time;
uniform float last_stroke;

void main() {
  if (is_blinking) {
    float t = time - last_stroke;
    float threshold = float(t < BLINK_THRESHOLD);
    float blink = mod(floor(t / PERIOD), float(2));
    // gl_FragColor = vec4(1.0) * min(threshold + blink, 1.0);
    gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);
  } else {
    gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);
  }
} 
