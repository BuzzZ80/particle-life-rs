#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Circle {
    pub color: [f32; 3],
    pub rad: f32,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
}