pub struct Camera {
    pub pos: [f32; 2],
    pub scale: f32,
}

impl Camera {
    pub fn transform(&self) -> [[f32; 4]; 4] {
        let t = self.pos;
        let s = self.scale;
        [
            [ s  ,  0.0, 0.0, 0.0],
            [ 0.0,  s  , 0.0, 0.0],
            [ 0.0,  0.0, 1.0, 0.0],
            [t[0] * s, t[1] * s, 0.0, 1.0],
        ]
    }
}