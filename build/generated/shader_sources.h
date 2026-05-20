#ifndef SHADER_SOURCES_H
#define SHADER_SOURCES_H

static const char* vertexShaderSource = R"(
#version 330 core
layout(location = 0) in vec2 aPos; // Matches your 2-component vertex attribute
void main() {
    gl_Position = vec4(aPos.x, aPos.y, 0.0, 1.0);
}

)";

static const char* fragmentShaderSource = R"(
#version 330 core
out vec4 FragColor;
void main() {
    FragColor = vec4(1.0, 0.0, 0.0, 1.0); // Outputs a bright RED triangle
}

)";

#endif
