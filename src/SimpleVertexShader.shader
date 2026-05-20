#version 330 core
layout(location = 0) in vec2 aPos; // Matches your 2-component vertex attribute
void main() {
    gl_Position = vec4(aPos.x, aPos.y, 0.0, 1.0);
}
