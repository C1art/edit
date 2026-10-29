use crossbeam::channel::Receiver;

use crate::compositor::{extent, Compositor};
use crate::RenderError;

const BYTES_PER_PIXEL: u32 = 4;

/// A presented frame on its way back from the GPU.
pub struct Readback {
    device: wgpu::Device,
    buffer: wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
    mapped: Receiver<Result<(), wgpu::BufferAsyncError>>,
    row_bytes: u32,
    padded_row_bytes: u32,
    height: u32,
}

impl Compositor {
    /// Copies the last presented frame to the CPU as tightly packed, sRGB-encoded RGBA8.
    /// Blocks until the GPU finishes, so call it from a worker thread.
    pub fn read_output(&self) -> Result<Vec<u8>, RenderError> {
        self.start_readback()?.finish()
    }

    /// Starts copying the last presented frame back without waiting for it, so the CPU
    /// can prepare the next frame while the GPU finishes this one. Frames composited
    /// meanwhile don't disturb the copy.
    pub fn start_readback(&self) -> Result<Readback, RenderError> {
        let texture = self.output_texture().ok_or(RenderError::NoFrame)?;
        let (width, height) = (texture.width(), texture.height());
        let row_bytes = width * BYTES_PER_PIXEL;
        let padded_row_bytes = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            extent(width, height),
        );
        let submission = self.queue.submit([encoder.finish()]);

        let (sender, mapped) = crossbeam::channel::bounded(1);
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        Ok(Readback {
            device: self.device.clone(),
            buffer,
            submission,
            mapped,
            row_bytes,
            padded_row_bytes,
            height,
        })
    }
}

impl Readback {
    /// The frame as tightly packed, sRGB-encoded RGBA8. Blocks until the GPU finishes,
    /// so call it from a worker thread.
    pub fn finish(self) -> Result<Vec<u8>, RenderError> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(self.submission),
                timeout: None,
            })
            .map_err(|error| RenderError::Readback(error.to_string()))?;
        self.mapped
            .recv()
            .map_err(|error| RenderError::Readback(error.to_string()))?
            .map_err(|error| RenderError::Readback(error.to_string()))?;

        let mapped = self
            .buffer
            .get_mapped_range(..)
            .map_err(|error| RenderError::Readback(error.to_string()))?;
        let mut pixels = Vec::with_capacity((self.row_bytes * self.height) as usize);
        for row in mapped.chunks_exact(self.padded_row_bytes as usize) {
            pixels.extend_from_slice(&row[..self.row_bytes as usize]);
        }
        drop(mapped);
        self.buffer.unmap();
        Ok(pixels)
    }
}
