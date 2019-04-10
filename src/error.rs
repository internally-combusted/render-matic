// error.rs
// Error management.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

/// An attempt to unify errors across the various external crates and to
/// handle our own errors besides.
use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Bind(gfx_hal::device::BindError),
    DescriptorAllocation(gfx_hal::pso::AllocationError),
    HostExecution(gfx_hal::error::HostExecutionError),
    Image(image::ImageError),
    ImageCreation(gfx_hal::image::CreationError),
    ImageView(gfx_hal::image::ViewError),
    Index(),
    Io(io::Error),
    Mapping(gfx_hal::mapping::Error),
    MemoryAllocation(gfx_hal::device::AllocationError),
    None(),
    NoSuitableMemory(),
    OutOfMemory(gfx_hal::device::OutOfMemory),
    OutOfMemoryOrDeviceLost(gfx_hal::device::OomOrDeviceLost),
    SerdeYaml(serde_yaml::Error),
    Shader(gfx_hal::device::ShaderError),
    WindowCreation(winit::CreationError),
    WrongType(&'static str),
}

impl fmt::Display for Error {
    /// Just take whatever was given for the error and print it out.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "{:?}", self)?;
        Ok(())
    }
}

// TODO: Seems like a macro could probably handle all this nonsense.

impl From<gfx_hal::device::BindError> for Error {
    fn from(err: gfx_hal::device::BindError) -> Error {
        Error::Bind(err)
    }
}

impl From<gfx_hal::device::AllocationError> for Error {
    fn from(err: gfx_hal::device::AllocationError) -> Error {
        Error::MemoryAllocation(err)
    }
}

impl From<gfx_hal::device::OutOfMemory> for Error {
    fn from(err: gfx_hal::device::OutOfMemory) -> Error {
        Error::OutOfMemory(err)
    }
}

impl From<gfx_hal::device::OomOrDeviceLost> for Error {
    fn from(err: gfx_hal::device::OomOrDeviceLost) -> Error {
        Error::OutOfMemoryOrDeviceLost(err)
    }
}

impl From<gfx_hal::device::ShaderError> for Error {
    fn from(err: gfx_hal::device::ShaderError) -> Error {
        Error::Shader(err)
    }
}

impl From<gfx_hal::error::HostExecutionError> for Error {
    fn from(err: gfx_hal::error::HostExecutionError) -> Error {
        Error::HostExecution(err)
    }
}

impl From<gfx_hal::image::CreationError> for Error {
    fn from(err: gfx_hal::image::CreationError) -> Error {
        Error::ImageCreation(err)
    }
}

impl From<gfx_hal::image::ViewError> for Error {
    fn from(err: gfx_hal::image::ViewError) -> Error {
        Error::ImageView(err)
    }
}

impl From<gfx_hal::mapping::Error> for Error {
    fn from(err: gfx_hal::mapping::Error) -> Error {
        Error::Mapping(err)
    }
}

impl From<gfx_hal::pso::AllocationError> for Error {
    fn from(err: gfx_hal::pso::AllocationError) -> Error {
        Error::DescriptorAllocation(err)
    }
}

impl From<image::ImageError> for Error {
    fn from(err: image::ImageError) -> Error {
        Error::Image(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Error {
        Error::SerdeYaml(err)
    }
}

impl From<winit::CreationError> for Error {
    fn from(err: winit::CreationError) -> Error {
        Error::WindowCreation(err)
    }
}
