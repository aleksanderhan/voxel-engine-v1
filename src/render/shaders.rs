// src/render/shaders.rs
//
// Centralized shader sources. WGSL has no native include mechanism in wgpu,
// so we concatenate multiple WGSL files into a single source string.

pub const RAYMARCH_COMPUTE_WGSL: &str = concat!(
    include_str!("../shaders/raymarch.wgsl"),
    "\n",
);
pub const BLIT_WGSL: &str = concat!(
    include_str!("../shaders/blit.wgsl"),
    "\n",
);

#[inline]
pub fn raymarch_compute_wgsl() -> &'static str {
    RAYMARCH_COMPUTE_WGSL
}

#[inline]
pub fn blit_wgsl() -> &'static str {
    BLIT_WGSL
}
