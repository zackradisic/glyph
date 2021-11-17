attribute vec3 aPos;
attribute float y_translate;
attribute float x_translate;

void main() {

  gl_Position = vec4(aPos.x + x_translate, aPos.y + (y_translate * -1.0) , aPos.z, 1.0);
}