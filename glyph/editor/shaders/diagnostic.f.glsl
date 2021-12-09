varying vec4 v_color;

void main() {
    // gl_FragColor = vec4(0.05882353, 0.7490196, 1.0, 0.2); 
    gl_FragColor = vec4(v_color.xyz, 0.2);
} 
