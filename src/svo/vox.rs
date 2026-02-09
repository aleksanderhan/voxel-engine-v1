use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use glam::IVec3;

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
    pub offset: IVec3,
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
        let mut trn_transforms: HashMap<i32, IVec3> = HashMap::new();
        let mut parent_map: HashMap<i32, i32> = HashMap::new();
        let mut shape_models: HashMap<i32, Vec<i32>> = HashMap::new();

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
                    models.push(VoxModel {
                        size,
                        voxels,
                        offset: IVec3::ZERO,
                    });
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
                b"nTRN" => {
                    let node_id = read_i32(data, &mut cursor)?;
                    let _attributes = read_dict(data, &mut cursor)?;
                    let child_node_id = read_i32(data, &mut cursor)?;
                    let _reserved_id = read_i32(data, &mut cursor)?;
                    let _layer_id = read_i32(data, &mut cursor)?;
                    let num_frames = read_i32(data, &mut cursor)?;
                    let mut translation = IVec3::ZERO;
                    for frame_index in 0..num_frames {
                        let frame_dict = read_dict(data, &mut cursor)?;
                        if frame_index == 0 {
                            if let Some(value) = frame_dict.get("_t") {
                                translation = parse_translation(value);
                            }
                        }
                    }
                    trn_transforms.insert(node_id, translation);
                    parent_map.insert(child_node_id, node_id);
                }
                b"nSHP" => {
                    let node_id = read_i32(data, &mut cursor)?;
                    let _attributes = read_dict(data, &mut cursor)?;
                    let num_models = read_i32(data, &mut cursor)?;
                    let mut model_ids = Vec::with_capacity(num_models.max(0) as usize);
                    for _ in 0..num_models {
                        let model_id = read_i32(data, &mut cursor)?;
                        let _model_attrs = read_dict(data, &mut cursor)?;
                        model_ids.push(model_id);
                    }
                    shape_models.insert(node_id, model_ids);
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

        if !models.is_empty() && !shape_models.is_empty() {
            let mut model_offsets = vec![IVec3::ZERO; models.len()];
            for (shape_node, model_ids) in shape_models {
                let mut translation = IVec3::ZERO;
                let mut current = shape_node;
                while let Some(parent) = parent_map.get(&current).copied() {
                    if let Some(offset) = trn_transforms.get(&parent) {
                        translation += *offset;
                    }
                    current = parent;
                }
                let translation = IVec3::new(translation.x, translation.z, translation.y);
                for model_id in model_ids {
                    if model_id >= 0 {
                        if let Some(slot) = model_offsets.get_mut(model_id as usize) {
                            *slot = translation;
                        }
                    }
                }
            }
            for (model, offset) in models.iter_mut().zip(model_offsets) {
                model.offset = offset;
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

fn read_i32(data: &[u8], cursor: &mut usize) -> Result<i32, VoxError> {
    if *cursor + 4 > data.len() {
        return Err(VoxError::UnexpectedEof);
    }
    let value = i32::from_le_bytes([
        data[*cursor],
        data[*cursor + 1],
        data[*cursor + 2],
        data[*cursor + 3],
    ]);
    *cursor += 4;
    Ok(value)
}

fn read_dict(data: &[u8], cursor: &mut usize) -> Result<HashMap<String, String>, VoxError> {
    let count = read_i32(data, cursor)?;
    let mut dict = HashMap::new();
    for _ in 0..count {
        let key = read_string(data, cursor)?;
        let value = read_string(data, cursor)?;
        dict.insert(key, value);
    }
    Ok(dict)
}

fn read_string(data: &[u8], cursor: &mut usize) -> Result<String, VoxError> {
    let len = read_i32(data, cursor)? as usize;
    if *cursor + len > data.len() {
        return Err(VoxError::UnexpectedEof);
    }
    let value = String::from_utf8_lossy(&data[*cursor..*cursor + len]).into_owned();
    *cursor += len;
    Ok(value)
}

fn parse_translation(value: &str) -> IVec3 {
    let mut parts = value.split_whitespace();
    let x = parts.next().and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    let y = parts.next().and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    let z = parts.next().and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    IVec3::new(x, y, z)
}
