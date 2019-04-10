#version 450

layout (location = 0) in vec3 coordinates;  // xyz
layout (location = 1) in vec4 rgba;        // rgba
layout (location = 2) in vec2 uv;           // uv

layout (location = 0) out vec4 color;
layout (location = 1) out vec2 texture_coordinates;

void main() {
    color = rgba;
    texture_coordinates = uv;
    gl_Position = vec4(coordinates.xy, 1.0, 1.0);
}