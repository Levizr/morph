cmake_minimum_required(VERSION 3.16)
project(gui_system)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# ── Dependencies ─────────────────────────────
find_package(OpenGL REQUIRED)
find_package(glfw3 REQUIRED)
find_package(Freetype REQUIRED)

# ── html2gl compiler ─────────────────────────
# add_subdirectory(html2gl)

# ── GENERATED FILE PATH ──────────────────────
set(GENERATED_CPP ${CMAKE_SOURCE_DIR}/src/output.cpp)

# ── AUTO GENERATE output.cpp BEFORE BUILD ────
# add_custom_command(
#     OUTPUT ${GENERATED_CPP}
#     COMMAND html2gl ${CMAKE_SOURCE_DIR}/example.html -o ${GENERATED_CPP}
#     DEPENDS html2gl ${CMAKE_SOURCE_DIR}/example.html
#     WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
#     COMMENT "Generating OpenGL code from HTML..."
# )

# ── Embed shader files as C++ strings ────────────
file(READ ${CMAKE_SOURCE_DIR}/src/SimpleVertexShader.shader VERTEX_SHADER_SOURCE)
file(READ ${CMAKE_SOURCE_DIR}/src/SimpleFragmentShader.shader FRAGMENT_SHADER_SOURCE)
configure_file(${CMAKE_SOURCE_DIR}/src/shader_sources.h.in
               ${CMAKE_BINARY_DIR}/generated/shader_sources.h)

# ── Main renderer (ProjectMorph) ─────────────────────
add_executable(ProjectMorph
    src/main.cpp
    src/common/loadShader.cpp
    src/glad.c
    # ${GENERATED_CPP}
)

target_include_directories(ProjectMorph PRIVATE include ${CMAKE_BINARY_DIR}/generated)

target_link_libraries(ProjectMorph
    OpenGL::GL
    glfw
    Freetype::Freetype
)

# # ── Ensure generation happens before build ───
# add_custom_target(generate_code
#     DEPENDS ${GENERATED_CPP}
# )

add_dependencies(ProjectMorph glfw)