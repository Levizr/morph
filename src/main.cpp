#include <glad/glad.h>
#include <GLFW/glfw3.h>
#include <iostream>
#include <vector>
#include "shader_sources.h"
#include "common/shader.hpp"

// Define a Vertex structure for organized layout
struct Vertex {
    float position[2];
    float color[3]; // Added color to demonstrate per-vertex batch variations
};

int main()
{
    if (!glfwInit()) { 
        std::cerr << "Failed to initialize GLFW" << std::endl;
        return -1;
    }

    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    GLFWwindow *window = glfwCreateWindow(720, 400, "GLFW Batch Rendering", NULL, NULL);
    if (!window) {
        std::cerr << "Failed to create GLFW window" << std::endl;
        glfwTerminate();
        return -1;
    }

    glfwMakeContextCurrent(window);

    if (!gladLoadGLLoader((GLADloadproc)glfwGetProcAddress)) {
        glfwDestroyWindow(window);
        glfwTerminate();
        return -1;
    }

    // Set Max Limits for the batch size
    const size_t MaxQuadCount = 1000;
    const size_t MaxVertexCount = MaxQuadCount * 4;
    const size_t MaxIndexCount = MaxQuadCount * 6;

    // Generate Indices upfront for Quads (0-1-2, 2-3-0 pattern)
    std::vector<GLuint> indices(MaxIndexCount);
    uint32_t offset = 0;
    for (size_t i = 0; i < MaxIndexCount; i += 6) {
        indices[i + 0] = offset + 0;
        indices[i + 1] = offset + 1;
        indices[i + 2] = offset + 2;
        indices[i + 3] = offset + 2;
        indices[i + 4] = offset + 3;
        indices[i + 5] = offset + 0;
        offset += 4;
    }

    // Setup OpenGL Objects
    GLuint VAO, VBO, EBO;
    glGenVertexArrays(1, &VAO);
    glBindVertexArray(VAO);

    // VBO configured with dynamic draw allocating max possible memory size
    glGenBuffers(1, &VBO);
    glBindBuffer(GL_ARRAY_BUFFER, VBO);
    glBufferData(GL_ARRAY_BUFFER, MaxVertexCount * sizeof(Vertex), nullptr, GL_DYNAMIC_DRAW);

    // EBO configured with static draw since indexing patterns don't change
    glGenBuffers(1, &EBO);
    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, EBO);
    glBufferData(GL_ELEMENT_ARRAY_BUFFER, indices.size() * sizeof(GLuint), indices.data(), GL_STATIC_DRAW);

    // Attribute 0: Position
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)offsetof(Vertex, position));
    
    // Attribute 1: Color
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)offsetof(Vertex, color));

    GLuint programID = LoadShadersFromString(vertexShaderSource, fragmentShaderSource);
    glUseProgram(programID);

    glClearColor(0.1f, 0.1f, 0.1f, 1.0f);

    // CPU-side dynamic buffer for the current frame
    std::vector<Vertex> verticesBatch;

    while (!glfwWindowShouldClose(window))
    {
        glClear(GL_COLOR_BUFFER_BIT);

        // Clear CPU cache from previous frame
        verticesBatch.clear();

        // --- BATCHING OBJECT 1 (Red Quad) ---
        verticesBatch.push_back({{-0.5f,  0.5f}, {1.0f, 0.0f, 0.0f}});
        verticesBatch.push_back({{-0.5f, -0.5f}, {1.0f, 0.0f, 0.0f}});
        verticesBatch.push_back({{ 0.0f, -0.5f}, {1.0f, 0.0f, 0.0f}});
        verticesBatch.push_back({{ 0.0f,  0.5f}, {1.0f, 0.0f, 0.0f}});

        // --- BATCHING OBJECT 2 (Blue Quad) ---
        verticesBatch.push_back({{ 0.1f,  0.5f}, {0.0f, 0.0f, 1.0f}});
        verticesBatch.push_back({{ 0.1f, -0.5f}, {0.0f, 0.0f, 1.0f}});
        verticesBatch.push_back({{ 0.6f, -0.5f}, {0.0f, 0.0f, 1.0f}});
        verticesBatch.push_back({{ 0.6f,  0.5f}, {0.0f, 0.0f, 1.0f}});

        // 1. Upload the entire batch data to GPU at once
        glBindBuffer(GL_ARRAY_BUFFER, VBO);
        glBufferSubData(GL_ARRAY_BUFFER, 0, verticesBatch.size() * sizeof(Vertex), verticesBatch.data());

        // 2. Calculate indices count based on submitted vertex count
        GLsizei indexCountToDraw = (verticesBatch.size() / 4) * 6;

        // 3. One single Draw call for all objects
        glBindVertexArray(VAO);
        glDrawElements(GL_TRIANGLES, indexCountToDraw, GL_UNSIGNED_INT, nullptr);

        glfwSwapBuffers(window);
        glfwPollEvents();
    }

    glDeleteVertexArrays(1, &VAO);
    glDeleteBuffers(1, &VBO);
    glDeleteBuffers(1, &EBO);
    glfwTerminate();
    return 0;
}
