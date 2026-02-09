// src/render/shaders.rs
//
// Centralized shader sources. WGSL has no native include mechanism in wgpu,
// so we concatenate multiple WGSL files into a single source string.

pub const SHADER_WGSL: &str = concat!(
    include_str!("../shaders/shader.wgsl"),
    "\n",
);
pub const BLIT_WGSL: &str = concat!(
    include_str!("../shaders/blit.wgsl"),
    "\n",
);

#[inline]
pub fn shader_wgsl() -> &'static str {
    SHADER_WGSL
}

#[inline]
pub fn blit_wgsl() -> &'static str {
    BLIT_WGSL
}
