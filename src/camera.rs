pub struct Camera {
    pub pos: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
}

impl Camera {
    pub fn transform(&self) -> [[f32; 4]; 4] {
        let t = self.pos;
        let s = self.scale;
        let r = self.rotation;
        [
            [s * r.cos(), s * r.sin(), 0.0, 0.0],
            [-s * r.sin(), s * r.cos(), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [t[0], t[1], 0.0, 1.0],
        ]
    }
}
