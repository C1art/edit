use crate::common::{compositor, filled, layer, solid, SIZE};
use render::{LayerImage, LayerLook, Picture};
use timeline::{BlendMode, MatteChannel};

fn decode(encoded: u8) -> f32 {
    let c = f32::from(encoded) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(linear: f32) -> f32 {
    let c = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    c * 255.0
}

#[test]
#[ignore = "needs a GPU"]
fn track_mattes_keep_the_fill_by_the_chosen_channel_of_the_matte() {
    let mut compositor = compositor();
    let fill = solid([200, 100, 50]);
    // Opaque, so its alpha keeps all of the fill, and dark grey, so its luma a little.
    let matte = filled([64, 64, 64, 255]);
    let luma = 64.0 / 255.0;
    let cases = [
        (MatteChannel::Alpha, false, 1.0),
        (MatteChannel::Alpha, true, 0.0),
        (MatteChannel::Luma, false, luma),
        (MatteChannel::Luma, true, 1.0 - luma),
        // Grey has no saturation, and every channel and the lightness equal its level.
        (MatteChannel::Red, false, luma),
        (MatteChannel::Saturation, false, 0.0),
        (MatteChannel::Lightness, true, 1.0 - luma),
    ];
    for (channel, reverse, kept) in cases {
        let matted = LayerImage {
            picture: Picture::Matted {
                fill: vec![layer(&fill, BlendMode::Normal)],
                matte: vec![layer(&matte, BlendMode::Normal)],
                channel,
                reverse,
            },
            look: LayerLook::IDENTITY,
            grade: None,
        };
        compositor.composite((SIZE, SIZE), &[matted]).unwrap();
        let pixel = compositor.read_output().unwrap();
        for (index, value) in [200, 100, 50].into_iter().enumerate() {
            let wanted = encode(decode(value) * kept);
            let got = f32::from(pixel[index]);
            assert!(
                (got - wanted).abs() <= 2.0,
                "{channel:?} reversed {reverse} channel {index}: got {got}, wanted {wanted}"
            );
        }
    }
}
