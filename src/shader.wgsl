struct circle {
    color: i32,
    rad: f32,
    pos: vec2<f32>,
    vel: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> dt: f32;
@group(0) @binding(1)
var constraints: texture_2d<f32>;
@group(0) @binding(2)
var colors: texture_1d<f32>;
@group(0) @binding(3)
var d_sampler: sampler;

@group(1) @binding(0)
var<storage, read_write> circles: array<circle>;

@compute @workgroup_size(1)
fn compute_main(
    @builtin(workgroup_id) wgid: vec3<u32>,
    @builtin(num_workgroups) wgs: vec3<u32>,
) {
    let i = wgid.x;

    let rmin: f32 = 3.0;
    let racc: f32 = 2.0;
    let rmax: f32 = 5.0;

    let mu: f32 = 2.0;

    var a = vec2(0.0, 0.0);

    for (var j: u32 = u32(1); j < wgs.x; j++) {
        // get vector and length between self and other
        let diff = circles[i].pos - circles[j].pos;
        let d = length(diff);
        if (diff.x == 0.0) || (diff.y == 0.0) || (d >= rmax) {continue;}

        var acc = textureLoad(constraints, vec2(circles[i].color, circles[j].color), 0).x;

        if d < rmin {
            a -= normalize(diff) * racc * (d / rmin - 1.0);
        } else {
            a -= normalize(diff) * -(acc/rmax)*(d-rmin)*(d-rmax);
        }
    }

    a -= (mu * pow(length(circles[i].vel), 2.0)) * normalize(circles[i].vel);

    circles[i].vel += min(a * dt, vec2(1.0, 1.0));
    circles[i].pos += min(circles[i].vel * dt, vec2(1.0, 1.0)); 
}

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: i32,
};

@group(0) @binding(0)
var<uniform> camera: mat4x4<f32>;
@group(0) @binding(1)
var<uniform> size: vec2<u32>;

@vertex
fn vs_main(
    model: VertexInput,
    @builtin(instance_index) ix: u32,
) -> VertexOutput {
    var out: VertexOutput;
    let instance = circles[ix];

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
    return vec4<f32>(textureLoad(colors, in.color, 0).xyz, l);
}