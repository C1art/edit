use render::{
    CellShape, Cells, Checkerboard, Circle, EllipseRing, FourColorGradient, Grid, PixelEffects,
};
use timeline::{CellKind, Param, Ticks, VideoEffect};

use crate::pixel_effects::encoded;

/// Sets one of the Generate category's drawn effects in `pixel`, leaving others alone.
pub(crate) fn set_generate_effect(pixel: &mut PixelEffects, effect: &VideoEffect, time: Ticks) {
    let unit = |param: &Param| param.value_at(time) / 100.0;
    match effect {
        VideoEffect::Checkerboard(timeline::Checkerboard {
            anchor,
            size,
            color,
            opacity,
        }) => {
            pixel.generate.checkerboard = Some(Checkerboard {
                anchor: anchor.each_ref().map(unit),
                size: size.each_ref().map(|side| side.value_at(time)),
                color: encoded(*color),
                opacity: unit(opacity).clamp(0.0, 1.0),
            });
        }
        VideoEffect::Grid(timeline::Grid {
            anchor,
            size,
            border,
            color,
            opacity,
            invert,
        }) => {
            pixel.generate.grid = Some(Grid {
                anchor: anchor.each_ref().map(unit),
                size: size.each_ref().map(|side| side.value_at(time)),
                border: border.value_at(time),
                color: encoded(*color),
                opacity: unit(opacity).clamp(0.0, 1.0),
                invert: *invert,
            });
        }
        VideoEffect::Circle(timeline::Circle {
            center,
            radius,
            feather,
            color,
            opacity,
            invert,
        }) => {
            pixel.generate.circle = Some(Circle {
                center: center.each_ref().map(unit),
                radius: radius.value_at(time),
                feather: feather.value_at(time),
                color: encoded(*color),
                opacity: unit(opacity).clamp(0.0, 1.0),
                invert: *invert,
            });
        }
        VideoEffect::FourColorGradient(timeline::FourColorGradient {
            points,
            colors,
            blend,
            opacity,
        }) => {
            pixel.generate.four_color = Some(FourColorGradient {
                points: points.each_ref().map(|point| point.each_ref().map(unit)),
                colors: colors.map(encoded),
                blend: unit(blend),
                opacity: unit(opacity).clamp(0.0, 1.0),
            });
        }
        VideoEffect::Ellipse(ellipse) => {
            pixel.pattern.ellipse = Some(EllipseRing {
                center: ellipse.center.each_ref().map(unit),
                size: [&ellipse.width, &ellipse.height].map(|side| side.value_at(time)),
                thickness: ellipse.thickness.value_at(time),
                softness: unit(&ellipse.softness),
                inside: encoded(ellipse.inside_color),
                outside: encoded(ellipse.outside_color),
                over_picture: ellipse.composite_on_original,
            });
        }
        VideoEffect::CellPattern(cells) => {
            pixel.pattern.cells = Some(Cells {
                shape: match cells.pattern {
                    CellKind::Bubbles => CellShape::Bubbles,
                    CellKind::Crystals => CellShape::Crystals,
                    CellKind::Plates => CellShape::Plates,
                    CellKind::StaticPlates => CellShape::StaticPlates,
                    CellKind::Crystallize => CellShape::Crystallize,
                    CellKind::Pillow => CellShape::Pillow,
                },
                invert: cells.invert,
                contrast: unit(&cells.contrast),
                disperse: unit(&cells.disperse),
                size: cells.size.value_at(time),
                offset: cells.offset.each_ref().map(|axis| axis.value_at(time)),
                evolution: cells.evolution.value_at(time) / 360.0,
                blend: unit(&cells.blend_with_original),
            });
        }
        _ => {}
    }
}
