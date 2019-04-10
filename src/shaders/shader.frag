#version 450

layout (location = 0) in vec4 rgba;
layout (location = 1) in vec2 texture_coordinates;

layout (location = 0) out vec4 color;

layout (set = 1, binding = 0) uniform sampler texture_sampler;
layout (set = 1, binding = 1) uniform texture2D texture_data;

void main() {
    color = texture(sampler2D(texture_data, texture_sampler), texture_coordinates);
}