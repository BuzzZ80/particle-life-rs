#[repr(C)]
#[derive(Clone, Copy, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Circle {
    pub color: i32,
    pub rad: f32,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
}