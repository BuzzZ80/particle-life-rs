struct circle {
    color: vec3<f32>,
    rad: f32,
    pos: vec2<f32>,
    vel: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: mat4x4<f32>;
@group(0) @binding(1)
var<uniform> size: vec2<u32>;
@group(0) @binding(2)
var<uniform> dt: f32;

@group(1) @binding(0)
var<storage, read_write> circles: array<circle>;

@vertex
fn vs_main(
    model: VertexInput,
    @builtin(instance_index) ix: u32,
    @builtin(vertex_index) vx: u32,
) -> VertexOutput {
    var out: VertexOutput;
    let instance = circles[ix];

    if (vx % u32(6) == u32(0)) {
        let rmin = 5.0;
        let racc = 6.0;
        let mu = 2.0;
        var acc = 1.0;

        var a = vec2(0.0, 0.0);
        for (var c: u32 = u32(0); c < arrayLength(&circles); c++) {
            if ix == c {continue;}  // dont compare to self

            // get vector and length between self and other
            let diff = circles[ix].pos - circles[c].pos;
            let d = length(diff);

            // If too close, push back. otherwise, 
            if d < rmin {
                a -= normalize(diff) * racc * (pow(d/rmin, 2.0) - 1.0);
            } else {
                a -= normalize(diff) * acc / (d - rmin + 1.0);
            }
        }

        // Subtract force of friction
        a -= (mu * pow(length(instance.vel), 2.0)) * normalize(instance.vel);

        circles[ix].pos += circles[ix].vel * dt + (0.5) * a * dt * dt; 
        circles[ix].vel += a * dt;
    }

    out.tex_coords = model.position;
    out.color = instance.color;

    var pos = vec2(model.position * instance.rad + instance.pos);
    pos.x *= f32(size.y) / f32(size.x);

    out.clip_position = camera * vec4<f32>(pos, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let l = smoothstep(0.0, 0.05, 1.0 - length(in.tex_coords));
    return vec4<f32>(in.color, l);
}