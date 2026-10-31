use crate::common;
use std::sync::Arc;

use crate::common::{compositor, layer, solid};
use render::{Grade, RenderError, CURVE_SIZE};
use timeline::{BlendMode, CurvePoint, HueCurve, HueCurves, HUE_CURVE_NEUTRAL};

fn graded(rgb: [u8; 3], curves: &HueCurves) -> Result<[u8; 3], RenderError> {
    let mut compositor = compositor();
    let pixels = solid(rgb);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.grade = Some(Arc::new(Grade {
        hue_curves: Some(curves.table(CURVE_SIZE)),
        ..Grade::default()
    }));
    compositor.composite((common::SIZE, common::SIZE), &[picture])?;
    let output = compositor.read_output()?;
    Ok([output[0], output[1], output[2]])
}

#[test]
#[ignore = "needs a GPU"]
fn hue_vs_sat_greys_reds_and_leaves_blues() {
    let point = |input, output| CurvePoint { input, output };
    let curves = HueCurves {
        hue_vs_sat: HueCurve {
            points: vec![
                point(0, 0),
                point(250, HUE_CURVE_NEUTRAL),
                point(750, HUE_CURVE_NEUTRAL),
            ],
        },
        ..HueCurves::default()
    };
    let [r, g, b] = graded([200, 40, 40], &curves).unwrap();
    assert!(
        r.abs_diff(g) <= 2 && g.abs_diff(b) <= 2,
        "red became {r} {g} {b}"
    );
    let blue = graded([40, 60, 200], &curves).unwrap();
    let unchanged = blue
        .iter()
        .zip([40, 60, 200])
        .all(|(a, b)| a.abs_diff(b) <= 1);
    assert!(unchanged, "blue became {blue:?}");
}
