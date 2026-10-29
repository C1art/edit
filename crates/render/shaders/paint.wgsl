// Brush Strokes, Eyedropper Fill and SDR Conform.

const BRUSH_TAPS: i32 = 8;

struct StrokeFrame {
    // Where the stroke through this pixel crosses the middle of its run, in pixels.
    center: vec2<f32>,
    direction: vec2<f32>,
    length: f32,
    cover: f32,
};

// Strokes lie end to end in rows a brush wide along the stroke angle, each row shifted
// and each stroke turned at random, with a random gap at its end where the surface
// shows through.
fn stroke_frame(uv: vec2<f32>) -> StrokeFrame {
    let brush = placement.paint_brush;
    let px = uv * picture_size();
    let along = vec2<f32>(sin(brush.x), -cos(brush.x));
    let across = vec2<f32>(-along.y, along.x);
    let row = floor(dot(px, across) / brush.y);
    let run = dot(px, along) / brush.z + detail_hash(vec2<f32>(row, 0.0), 11.0) * 0.5 + 0.5;
    let stroke = vec2<f32>(row, floor(run));
    let turn = detail_hash(stroke, 23.0) * brush.w * PI * 0.5;
    let gap = (detail_hash(stroke, 37.0) * 0.5 + 0.5) * 0.3;
    var frame: StrokeFrame;
    frame.center = px + along * (0.5 - fract(run)) * brush.z;
    frame.direction = vec2<f32>(
        along.x * cos(turn) - along.y * sin(turn),
        along.x * sin(turn) + along.y * cos(turn),
    );
    frame.length = brush.z;
    frame.cover = saturate(((1.0 - gap) - fract(run)) * brush.z + 0.5);
    return frame;
}

fn stroke_color(frame: StrokeFrame) -> vec3<f32> {
    let size = picture_size();
    var sum = vec3<f32>(0.0);
    for (var tap = 0; tap < BRUSH_TAPS; tap++) {
        let along = (f32(tap) + 0.5) / f32(BRUSH_TAPS) - 0.5;
        sum += graded_at((frame.center + frame.direction * along * frame.length) / size);
    }
    return sum / f32(BRUSH_TAPS);
}

fn brushed(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let look = placement.paint_brush_look;
    if look.x < 0.5 {
        return rgb;
    }
    let frame = stroke_frame(uv);
    let paint = stroke_color(frame);
    var surface = rgb;
    if look.x > 3.5 {
        surface = vec3<f32>(0.0);
    } else if look.x > 2.5 {
        surface = vec3<f32>(1.0);
    } else if look.x > 1.5 {
        // Alpha hides the gaps; the paint's own color keeps their edges from fringing.
        surface = paint;
    }
    return mix(mix(surface, paint, frame.cover), rgb, look.y);
}

fn eyedropper_color() -> vec3<f32> {
    let fill = placement.paint_fill;
    let reach = fill.z / picture_size();
    var sum = graded_at(fill.xy);
    for (var tap = 0; tap < 8; tap++) {
        let angle = f32(tap) * TAU / 8.0;
        let offset = vec2<f32>(cos(angle), sin(angle)) * reach;
        sum += graded_at(fill.xy + offset) + graded_at(fill.xy + offset * 0.5);
    }
    return sum / 17.0;
}

fn eyedropper_filled(rgb: vec3<f32>) -> vec3<f32> {
    if placement.paint_fill.w < 0.5 {
        return rgb;
    }
    return mix(eyedropper_color(), rgb, placement.paint_fill_look.x);
}

fn painted_alpha(alpha: f32, uv: vec2<f32>) -> f32 {
    var painted = alpha;
    let fill = placement.paint_fill_look;
    if placement.paint_fill.w > 0.5 && fill.y < 0.5 {
        painted = mix(1.0, alpha, fill.x);
    }
    let look = placement.paint_brush_look;
    if look.x > 1.5 && look.x < 2.5 {
        painted *= mix(stroke_frame(uv).cover, 1.0, look.y);
    }
    return painted;
}

// SDR Conform: exposure in stops, contrast pivoting on middle grey, then highlights
// above the knee ease toward white instead of clipping.
fn sdr_conformed(rgb: vec3<f32>) -> vec3<f32> {
    let conform = placement.paint_conform;
    if conform.w < 0.5 {
        return rgb;
    }
    let exposed = max(rgb, vec3<f32>(0.0)) * exp2(conform.x * 2.0);
    let contrasted = 0.18 * pow(exposed / 0.18, vec3<f32>(1.0 + conform.y * 0.5));
    let start = 1.0 - conform.z;
    let over = max(contrasted - start, vec3<f32>(0.0));
    let eased = start + conform.z * (1.0 - exp(-over / max(conform.z, 1e-4)));
    return select(min(contrasted, vec3<f32>(1.0)), eased, contrasted > vec3<f32>(start));
}
