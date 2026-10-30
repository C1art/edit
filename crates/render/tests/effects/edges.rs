mod common;

use common::{compositor, layer, solid, SIZE};
use render::{AlphaGlow, Background, Bevel, Edges, PixelEffects, RenderError};
use timeline::BlendMode;

fn rendered(pixels: &[u8], edges: Edges) -> Result<Vec<[u8; 4]>, RenderError> {
    let mut picture = layer(pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        edges,
        ..PixelEffects::NONE
    };
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    compositor.composite((SIZE, SIZE), &[picture])?;
    Ok(compositor.read_output()?.as_chunks::<4>().0.to_vec())
}

fn at(output: &[[u8; 4]], x: u32, y: u32) -> [u8; 4] {
    output[(y * SIZE + x) as usize]
}

#[test]
#[ignore = "needs a GPU"]
fn bevel_edges_lights_the_side_facing_the_light_and_shades_the_far_side() -> Result<(), RenderError>
{
    let grey = solid([128; 3]);
    let output = rendered(
        &grey,
        Edges {
            // A pixel's width in from each side, lit from the left.
            bevel: Some(Bevel {
                thickness: 0.25,
                light_angle: -90f32.to_radians(),
                light_color: [1.0; 3],
                light_intensity: 0.5,
            }),
            ..Edges::NONE
        },
    )?;
    let [lit, inside, top, shaded] =
        [(0, 1), (1, 1), (1, 0), (3, 1)].map(|(x, y)| at(&output, x, y)[0]);
    assert!(lit > 200, "{output:?}");
    assert!(shaded < 110, "{output:?}");
    // Faces turned square to the light are neither lit nor shaded.
    assert!(
        inside.abs_diff(128) <= 1 && top.abs_diff(128) <= 1,
        "{output:?}"
    );
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn alpha_glow_fades_out_from_the_pictures_edge() -> Result<(), RenderError> {
    // The left half is transparent, the right half opaque red.
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| match index % SIZE < 2 {
            true => [0; 4],
            false => [255, 0, 0, 255],
        })
        .collect();
    let output = rendered(
        &pixels,
        Edges {
            alpha_glow: Some(AlphaGlow {
                size: 2.0,
                brightness: 1.0,
                start_color: [0.0, 0.0, 1.0],
                end_color: None,
                fade_out: true,
            }),
            ..Edges::NONE
        },
    )?;
    let [far, near, picture] = [0, 1, 2].map(|x| at(&output, x, 1));
    assert!(near[3] > far[3] && far[3] > 0, "{output:?}");
    assert_eq!(near[..3], [0, 0, 255], "the glow is the start color");
    assert_eq!(
        picture,
        [255, 0, 0, 255],
        "the glow stays behind the picture"
    );
    Ok(())
}
