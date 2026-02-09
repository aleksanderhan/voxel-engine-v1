use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct VoxVoxel {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub color_index: u8,
}

#[derive(Debug, Clone)]
pub struct VoxModel {
    pub size: [u32; 3],
    pub voxels: Vec<VoxVoxel>,
}

#[derive(Debug, Clone)]
pub struct VoxFile {
    pub models: Vec<VoxModel>,
    pub palette: [u32; 256],
}

#[derive(Debug)]
pub enum VoxError {
    Io(io::Error),
    InvalidHeader,
    UnexpectedEof,
}

impl From<io::Error> for VoxError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl VoxFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, VoxError> {
        let data = fs::read(path)?;
        Self::from_bytes(&data)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, VoxError> {
        if data.len() < 8 || &data[0..4] != b"VOX " {
            return Err(VoxError::InvalidHeader);
        }
        let mut cursor = 8usize;
        let mut models = Vec::new();
        let mut current_size: Option<[u32; 3]> = None;
        let mut palette = [0u32; 256];

        while cursor + 12 <= data.len() {
            let id = &data[cursor..cursor + 4];
            cursor += 4;
            let content_size = read_u32(data, &mut cursor)? as usize;
            let children_size = read_u32(data, &mut cursor)? as usize;

            if cursor + content_size > data.len() {
                return Err(VoxError::UnexpectedEof);
            }

            let content_start = cursor;
            match id {
                b"SIZE" => {
                    let x = read_u32(data, &mut cursor)?;
                    let y = read_u32(data, &mut cursor)?;
                    let z = read_u32(data, &mut cursor)?;
                    current_size = Some([x, y, z]);
                }
                b"XYZI" => {
                    let count = read_u32(data, &mut cursor)? as usize;
                    let mut voxels = Vec::with_capacity(count);
                    for _ in 0..count {
                        if cursor + 4 > data.len() {
                            return Err(VoxError::UnexpectedEof);
                        }
                        let x = data[cursor];
                        let y = data[cursor + 1];
                        let z = data[cursor + 2];
                        let color_index = data[cursor + 3];
                        cursor += 4;
                        voxels.push(VoxVoxel { x, y, z, color_index });
                    }
                    let size = current_size.unwrap_or([0, 0, 0]);
                    models.push(VoxModel { size, voxels });
                }
                b"RGBA" => {
                    for i in 0..256 {
                        if cursor + 4 > data.len() {
                            return Err(VoxError::UnexpectedEof);
                        }
                        let color = u32::from_le_bytes([
                            data[cursor],
                            data[cursor + 1],
                            data[cursor + 2],
                            data[cursor + 3],
                        ]);
                        cursor += 4;
                        if i < 255 {
                            palette[i + 1] = color;
                        }
                    }
                }
                _ => {
                }
            }
            let content_end = content_start + content_size;
            if cursor < content_end {
                cursor = content_end;
            }

            let _ = children_size;
        }

        if palette.iter().all(|&color| color == 0) {
            for i in 1..256 {
                let value = i as u32;
                palette[i] = 0xFF00_0000 | (value << 16) | (value << 8) | value;
            }
        }

        Ok(Self { models, palette })
    }
}

fn read_u32(data: &[u8], cursor: &mut usize) -> Result<u32, VoxError> {
    if *cursor + 4 > data.len() {
        return Err(VoxError::UnexpectedEof);
    }
    let value = u32::from_le_bytes([
        data[*cursor],
        data[*cursor + 1],
        data[*cursor + 2],
        data[*cursor + 3],
    ]);
    *cursor += 4;
    Ok(value)
}
