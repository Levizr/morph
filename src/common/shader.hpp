#ifndef SHADER_HPP
#define SHADER_HPP

#include <glad/glad.h>

GLuint LoadShaders(const char * vertex_file_path, const char * fragment_file_path);
GLuint LoadShadersFromString(const char * vertex_source, const char * fragment_source);

#endif