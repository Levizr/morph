cmake_minimum_required(VERSION 3.10)
project(morph_devrt LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 23)
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
pkg_check_modules(HARFBUZZ REQUIRED harfbuzz)

# Runtime directory (for relative includes like "core/node.h")
set(RUNTIME_DIR ${CMAKE_CURRENT_SOURCE_DIR}/..)
set(VENDOR_DIR ${RUNTIME_DIR}/vendor)

# Source files
set(DEV_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/main.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/dev_socket.cpp
)

set(RUNTIME_SOURCES
    ${RUNTIME_DIR}/core/node/node.cpp
    ${RUNTIME_DIR}/core/node/layout.cpp
    ${RUNTIME_DIR}/core/node/style.cpp
    ${RUNTIME_DIR}/core/node/animation.cpp
    ${RUNTIME_DIR}/core/node/flatten.cpp
    ${RUNTIME_DIR}/core/node/events.cpp
    ${RUNTIME_DIR}/core/node/paint_order.cpp
    ${RUNTIME_DIR}/core/window.cpp
    ${RUNTIME_DIR}/core/compositor.cpp
    ${RUNTIME_DIR}/render/gl_renderer.cpp
    ${RUNTIME_DIR}/reactivity/effect.cpp
    ${RUNTIME_DIR}/reactivity/task.cpp
    ${RUNTIME_DIR}/net/net.cpp
    ${VENDOR_DIR}/glad/glad.c
    ${VENDOR_DIR}/stb_image.c
)

set(RENDERER_SOURCES
    ${RUNTIME_DIR}/renderers/renderer.h
    ${RUNTIME_DIR}/renderers/renderer.cpp
    ${RUNTIME_DIR}/renderers/flash/flash.h
    ${RUNTIME_DIR}/renderers/flash/flash.cpp
    ${RUNTIME_DIR}/renderers/forge/forge.h
    ${RUNTIME_DIR}/renderers/forge/forge.cpp
    ${RUNTIME_DIR}/renderers/forge/damage.h
    ${RUNTIME_DIR}/renderers/forge/damage.cpp
)

add_executable(morph_devrt ${DEV_SOURCES} ${RUNTIME_SOURCES} ${RENDERER_SOURCES})

target_include_directories(morph_devrt PRIVATE
    ${RUNTIME_DIR}        # for "core/node.h", "render/gl_renderer.h"
    ${VENDOR_DIR}         # for "vendor/glad/glad.h"
    ${FREETYPE_INCLUDE_DIRS}
    ${HARFBUZZ_INCLUDE_DIRS}
    ${RUNTIME_DIR}/renderers  # for flash.h, forge.h, damage.h, tile.h, etc.
)

# All features enabled for dev mode (dead code elimination not needed in dev)
target_compile_definitions(morph_devrt PRIVATE
    MORPH_FEATURE_SCROLL
    MORPH_FEATURE_RADIUS
    MORPH_FEATURE_TEXT
    MORPH_FEATURE_BOLD
    MORPH_FEATURE_POSITION
    MORPH_FEATURE_ZINDEX
    MORPH_FEATURE_OPACITY
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
    MORPH_FEATURE_TRANSFORM
    MORPH_FEATURE_ANIMATION
    MORPH_FEATURE_DEV
    MORPH_FEATURE_DEV_RENDERER_SWITCH
    MORPH_RENDERER_FORGE
)

# Symbol visibility for dynamic logic loading (POSIX only; Windows/MinGW
# export everything by default and Apple clang rejects -rdynamic).
if(NOT APPLE)
    target_link_options(morph_devrt PRIVATE -rdynamic)
endif()

if(WIN32)
    target_link_libraries(morph_devrt PRIVATE
        glfw
        OpenGL::GL
        ws2_32
        ${FREETYPE_LIBRARIES}
        ${HARFBUZZ_LIBRARIES}
    )
elseif(APPLE)
    target_link_libraries(morph_devrt PRIVATE
        glfw
        OpenGL::GL
        ${FREETYPE_LIBRARIES}
        ${HARFBUZZ_LIBRARIES}
    )
else()
    target_link_libraries(morph_devrt PRIVATE
        glfw
        OpenGL::GL
        X11
        pthread
        dl
        ${FREETYPE_LIBRARIES}
        ${HARFBUZZ_LIBRARIES}
    )
endif()
