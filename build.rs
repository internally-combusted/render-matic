/*  Adapted for shaderc from Mistodon's glsl-to-spirv build script
    https://falseidolfactory.com/2018/06/23/compiling-glsl-to-spirv-at-build-time.html
*/
extern crate shaderc;

use std::error::Error;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

fn main() -> Result<(), Box<Error>> {
    // Create destination path if necessary
    std::fs::create_dir_all("src/shaders/gen")?;

    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();

    for entry in std::fs::read_dir("src/shaders")? {
        let entry = entry?;

        if entry.file_type()?.is_file() {
            let in_path = entry.path();

            // Support only vertex and fragment shaders currently
            let shader_type =
                in_path
                    .extension()
                    .and_then(|ext| match ext.to_string_lossy().as_ref() {
                        "vert" => Some(shaderc::ShaderKind::Vertex),
                        "frag" => Some(shaderc::ShaderKind::Fragment),
                        _ => None,
                    });

            if let Some(shader_type) = shader_type {
                let source = std::fs::read_to_string(&in_path)?;
                let out_path = format!(
                    "src/shaders/gen/{}.spv",
                    in_path.file_name().unwrap().to_string_lossy()
                );

                let binary_artifact = match compiler.compile_into_spirv(
                    &source,
                    shader_type,
                    &out_path,
                    "main",
                    Some(&options),
                ) {
                    Ok(binary) => binary,
                    Err(err) => return Err(Box::new(err)),
                };

                println!("{}", binary_artifact.get_warning_messages());
                std::fs::write(&out_path, &binary_artifact.as_binary_u8())?;
            }
        }
    }

    Ok(())
}
