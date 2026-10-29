// Mirror, the warps that move the picture's plane: Corner Pin, Transform and Basic 3D,
// and the page fold of Page Turn and Page Peel, measured in fractions of the picture.

// How far in from the fold a curl's roll reaches, in the picture's longer sides.
const CURL_WIDTH: f32 = 0.08;
// The darkest a curl's shadow makes what lies under the lifted page.
const CURL_SHADOW: f32 = 0.45;
// A flat page's back is a little darker, as it faces away from the light.
const PAGE_BACK_SHADE: f32 = 0.8;
// How much of the picture a curling page's pale back lets show through.
const PAPER_SHOW_THROUGH: f32 = 0.4;
const PAPER: f32 = 0.75;

// Reflects the side of the mirror's line that its normal points to from the other.
fn mirrored(uv: vec2<f32>) -> vec2<f32> {
    let mirror = placement.reshape_mirror;
    if mirror.w < 0.5 {
        return uv;
    }
    let picture = picture_size();
    let aspect = picture / max(picture.x, picture.y);
    let normal = vec2<f32>(cos(mirror.z), sin(mirror.z));
    let p = (uv - mirror.xy) * aspect;
    let beyond = dot(p, normal);
    if beyond <= 0.0 {
        return uv;
    }
    return (p - 2.0 * beyond * normal) / aspect + mirror.xy;
}

// Where the point at `uv` of the warped picture lies in the picture before warping;
// outside it where the plane would face away.
fn unpinned(uv: vec2<f32>) -> vec2<f32> {
    let rows = placement.reshape_unpin;
    if rows[0].w < 0.5 {
        return uv;
    }
    let point = vec3<f32>(uv, 1.0);
    let w = dot(rows[2].xyz, point);
    if w <= 1e-6 {
        return vec2<f32>(-1.0);
    }
    return vec2<f32>(dot(rows[0].xyz, point), dot(rows[1].xyz, point)) / w;
}

struct Page {
    // Where on the picture the point shows; outside it where the page has lifted away.
    uv: vec2<f32>,
    // 1 on the page's back.
    back: f32,
    // How far the point lies from the fold, in the picture's longer sides.
    depth: f32,
    // A curl's shadow on what the lifted page uncovers.
    shadow: f32,
}

// The page folds over a line square to its corner's diagonal, which sweeps from that
// corner past the opposite one; the part beyond the line lies mirrored over the rest.
fn folded(uv: vec2<f32>) -> Page {
    let fold = placement.reshape_fold;
    if fold.w < 0.5 {
        return Page(uv, 0.0, 0.0, 0.0);
    }
    let picture = picture_size();
    let aspect = picture / max(picture.x, picture.y);
    let normal = normalize(fold.xy);
    let corner = dot(0.5 * aspect * fold.xy, normal);
    let p = (uv - 0.5) * aspect;
    let beyond = dot(p, normal) - corner * (1.0 - 2.0 * fold.z);
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    if beyond >= 0.0 {
        let shade = CURL_SHADOW * (1.0 - smoothstep(0.0, CURL_WIDTH, beyond));
        return Page(vec2<f32>(-1.0), 0.0, beyond, shade * f32(inside && fold.w > 1.5));
    }
    let back = (p - 2.0 * beyond * normal) / aspect + vec2<f32>(0.5);
    if all(back >= vec2<f32>(0.0)) && all(back <= vec2<f32>(1.0)) {
        return Page(back, 1.0, -beyond, 0.0);
    }
    return Page(uv, 0.0, -beyond, 0.0);
}

// The page's back: a curl's is pale like paper and lit along its roll.
fn page_side(rgb: vec3<f32>, page: Page) -> vec3<f32> {
    if page.back < 0.5 {
        return rgb;
    }
    if placement.reshape_fold.w < 1.5 {
        return rgb * PAGE_BACK_SHADE;
    }
    let roll = smoothstep(0.0, CURL_WIDTH, page.depth);
    return mix(vec3<f32>(PAPER), rgb, PAPER_SHOW_THROUGH) * mix(1.3, 0.85, roll);
}
