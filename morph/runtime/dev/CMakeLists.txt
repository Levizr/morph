cmake_minimum_required(VERSION 3.10)
project(morph_devrt LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

cmake_policy(SET CMP0072 NEW)

# Output to morph/bin/
set(MORPH_BIN_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../../bin)
file(MAKE_DIRECTORY ${MORPH_BIN_DIR})
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${MORPH_BIN_DIR})

# Find packages
find_package(OpenGL REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(FREETYPE REQUIRED freetype2)

# Runtime directory (for relative includes like "core/node.h")
set(RUNTIME_DIR ${CMAKE_CURRENT_SOURCE_DIR}/..)
set(VENDOR_DIR ${RUNTIME_DIR}/vendor)

# Source files
set(DEV_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/main.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/dev_socket.cpp
)

set(RUNTIME_SOURCES
    ${RUNTIME_DIR}/core/node.cpp
    ${RUNTIME_DIR}/core/window.cpp
    ${RUNTIME_DIR}/render/gl_renderer.cpp
    ${RUNTIME_DIR}/core/glad.c
    ${VENDOR_DIR}/stb_image.c
)

add_executable(morph_devrt ${DEV_SOURCES} ${RUNTIME_SOURCES})

target_include_directories(morph_devrt PRIVATE
    ${RUNTIME_DIR}        # for "core/node.h", "render/gl_renderer.h"
    ${VENDOR_DIR}         # for "vendor/glad/glad.h"
    ${FREETYPE_INCLUDE_DIRS}
)

# All features enabled for dev mode (dead code elimination not needed in dev)
target_compile_definitions(morph_devrt PRIVATE
    MORPH_FEATURE_SCROLL
    MORPH_FEATURE_RADIUS
    MORPH_FEATURE_TEXT
    MORPH_FEATURE_BOLD
    MORPH_FEATURE_POSITION
    MORPH_FEATURE_FLEX
    MORPH_FEATURE_CURSOR
    MORPH_FEATURE_BORDER
    MORPH_FEATURE_DISPLAY_NONE
    MORPH_FEATURE_INLINE
    MORPH_FEATURE_MARGIN_COLLAPSE
    MORPH_FEATURE_MIN_MAX
    MORPH_FEATURE_BORDER_BOX
    MORPH_FEATURE_IMAGE
    MORPH_FEATURE_DIRTY_RENDERING
)

target_link_libraries(morph_devrt PRIVATE
    glfw
    OpenGL::GL
    X11
    pthread
    dl
    ${FREETYPE_LIBRARIES}
)
