//! The layer shader, specialized to the effects each layer uses. Every effect is
//! written into one program, and drivers inline all of it into every entry point, so
//! compiling the whole of it is slow everywhere and beyond some drivers altogether.
//!
//! Blocks of the shader are marked with directives on lines of their own:
//!
//! ```wgsl
//! // #if style_shadow.w point_shadow
//! ...kept when any of these uniform fields differs from its neutral value...
//! // #else
//! ...kept otherwise...
//! // #endif
//! ```
//!
//! A field is neutral when it holds what [`Placement::FULL`] writes there, which is
//! every effect switched off. A block may only be left out when, with all its fields
//! neutral, the code it guards does nothing the `#else` side doesn't.

use std::collections::HashMap;
use std::ops::Range;

use crate::placement::Placement;

const SOURCE: &str = concat!(
    include_str!("../shaders/blit.wgsl"),
    include_str!("../shaders/blend.wgsl"),
    include_str!("../shaders/style.wgsl"),
    include_str!("../shaders/reshape.wgsl"),
    include_str!("../shaders/recolor.wgsl"),
    include_str!("../shaders/key.wgsl"),
    include_str!("../shaders/matte.wgsl"),
    include_str!("../shaders/adjust.wgsl"),
    include_str!("../shaders/mask.wgsl"),
    include_str!("../shaders/secondary.wgsl"),
    include_str!("../shaders/lumetri.wgsl"),
    include_str!("../shaders/generate.wgsl"),
    include_str!("../shaders/optics.wgsl"),
    include_str!("../shaders/clip_wipes.wgsl"),
    include_str!("../shaders/detail.wgsl"),
    include_str!("../shaders/edges.wgsl"),
    include_str!("../shaders/tone.wgsl"),
    include_str!("../shaders/shift.wgsl"),
    include_str!("../shaders/pattern.wgsl"),
    include_str!("../shaders/lighting.wgsl"),
    include_str!("../shaders/noise_alpha.wgsl"),
    include_str!("../shaders/cineon.wgsl"),
    include_str!("../shaders/paint.wgsl"),
    include_str!("../shaders/combine.wgsl"),
    include_str!("../shaders/color.wgsl"),
    include_str!("../shaders/wipe.wgsl"),
);

/// Bytes in one `vec4<f32>` of the uniform.
const VEC4: usize = 16;

/// Which of the shader's conditional blocks a layer needs: one bit per distinct
/// condition, in the order they first appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Features(u128);

impl Features {
    pub(crate) const NONE: Self = Self(0);

    fn has(self, condition: usize) -> bool {
        // Conditions past the bits there are always kept.
        condition >= 128 || self.0 & (1 << condition) != 0
    }
}

enum Segment {
    Text(&'static str),
    Block {
        condition: usize,
        kept: Vec<Segment>,
        otherwise: Vec<Segment>,
    },
}

/// The parsed shader: its text and blocks, and the uniform bytes each block's
/// condition reads.
pub(crate) struct LayerShader {
    segments: Vec<Segment>,
    /// Byte ranges of the uniform per condition; `None` for a condition naming a field
    /// the uniform doesn't have, which is always kept.
    conditions: Vec<Option<Vec<Range<usize>>>>,
    neutral: Vec<u8>,
}

impl LayerShader {
    pub(crate) fn new() -> Self {
        Self::parse(SOURCE, Placement::FULL.to_bytes()).0
    }

    /// The shader in `source`, and what was wrong with its directives, which are
    /// read leniently: blocks with unknown fields are always kept, and blocks left
    /// open end with the source.
    fn parse(source: &'static str, neutral: Vec<u8>) -> (Self, Vec<String>) {
        let fields = uniform_fields(source);
        let mut problems = Vec::new();
        let mut named: HashMap<String, usize> = HashMap::new();
        let mut conditions = Vec::new();
        // The open blocks, innermost last: condition, kept so far, and the `#else`
        // side once reached.
        let mut open: Vec<(usize, Vec<Segment>, Option<Vec<Segment>>)> = Vec::new();
        let mut top = Vec::new();
        let mut text_start = 0;
        let mut offset = 0;
        for line in source.split_inclusive('\n') {
            let directive = line.trim().strip_prefix("// #");
            let line_start = offset;
            offset += line.len();
            let Some(directive) = directive else {
                continue;
            };
            let current = match open.last_mut() {
                Some((_, kept, None)) => kept,
                Some((_, _, Some(otherwise))) => otherwise,
                None => &mut top,
            };
            if text_start < line_start {
                current.push(Segment::Text(&source[text_start..line_start]));
            }
            text_start = offset;
            let mut words = directive.split_whitespace();
            match words.next() {
                Some("if") => {
                    let terms: Vec<&str> = words.collect();
                    if terms.is_empty() {
                        problems.push(format!("`#if` without fields: {}", line.trim()));
                    }
                    let condition = *named.entry(terms.join(" ")).or_insert_with(|| {
                        conditions.push(condition_ranges(&fields, &terms, &mut problems));
                        conditions.len() - 1
                    });
                    open.push((condition, Vec::new(), None));
                }
                Some("else") => match open.last_mut() {
                    Some((_, _, otherwise @ None)) => *otherwise = Some(Vec::new()),
                    _ => problems.push("`#else` outside an `#if`".to_owned()),
                },
                Some("endif") => match open.pop() {
                    Some((condition, kept, otherwise)) => {
                        let block = Segment::Block {
                            condition,
                            kept,
                            otherwise: otherwise.unwrap_or_default(),
                        };
                        match open.last_mut() {
                            Some((_, kept, None)) => kept.push(block),
                            Some((_, _, Some(otherwise))) => otherwise.push(block),
                            None => top.push(block),
                        }
                    }
                    None => problems.push("`#endif` without an `#if`".to_owned()),
                },
                _ => problems.push(format!("unknown directive: {}", line.trim())),
            }
        }
        if text_start < source.len() {
            top.push(Segment::Text(&source[text_start..]));
        }
        if !open.is_empty() {
            problems.push(format!("{} `#if` left open", open.len()));
            // Whatever was inside still counts, as if each block were closed here.
            while let Some((condition, kept, otherwise)) = open.pop() {
                let block = Segment::Block {
                    condition,
                    kept,
                    otherwise: otherwise.unwrap_or_default(),
                };
                match open.last_mut() {
                    Some((_, kept, None)) => kept.push(block),
                    Some((_, _, Some(otherwise))) => otherwise.push(block),
                    None => top.push(block),
                }
            }
        }
        if conditions.len() > 128 {
            problems.push(format!("{} conditions, past 128", conditions.len()));
        }
        let shader = Self {
            segments: top,
            conditions,
            neutral,
        };
        (shader, problems)
    }

    /// The blocks a layer drawn with `uniform`, the bytes of its placement, needs.
    pub(crate) fn features(&self, uniform: &[u8]) -> Features {
        let differs =
            |range: &Range<usize>| uniform.get(range.clone()) != self.neutral.get(range.clone());
        let bits = self
            .conditions
            .iter()
            .take(128)
            .enumerate()
            .filter(|(_, ranges)| {
                ranges
                    .as_ref()
                    .is_none_or(|ranges| ranges.iter().any(differs))
            })
            .fold(0u128, |bits, (index, _)| bits | 1 << index);
        Features(bits)
    }

    /// The shader with only the blocks `features` keeps.
    pub(crate) fn source(&self, features: Features) -> String {
        let mut out = String::with_capacity(SOURCE.len());
        self.write_segments(&self.segments, features, &mut out);
        out
    }

    fn write_segments(&self, segments: &[Segment], features: Features, out: &mut String) {
        for segment in segments {
            match segment {
                Segment::Text(text) => out.push_str(text),
                Segment::Block {
                    condition,
                    kept,
                    otherwise,
                } => {
                    let unknown = self.conditions.get(*condition).is_none_or(Option::is_none);
                    let shown = if unknown || features.has(*condition) {
                        kept
                    } else {
                        otherwise
                    };
                    self.write_segments(shown, features, out);
                }
            }
        }
    }

    #[cfg(test)]
    fn every_feature(&self) -> Features {
        Features(u128::MAX)
    }
}

/// The byte ranges `terms` name: whole fields, or one component of a `vec4` field as
/// `field.x` through `field.w`.
fn condition_ranges(
    fields: &HashMap<&str, Range<usize>>,
    terms: &[&str],
    problems: &mut Vec<String>,
) -> Option<Vec<Range<usize>>> {
    let mut ranges = Vec::with_capacity(terms.len());
    for term in terms {
        let (name, component) = match term.split_once('.') {
            Some((name, component)) => (name, Some(component)),
            None => (*term, None),
        };
        let Some(field) = fields.get(name) else {
            problems.push(format!("unknown uniform field `{name}`"));
            return None;
        };
        let range = match component {
            None => field.clone(),
            Some(component) => {
                let Some(index) = ["x", "y", "z", "w"].iter().position(|c| *c == component) else {
                    problems.push(format!("unknown component in `{term}`"));
                    return None;
                };
                if field.len() != VEC4 {
                    problems.push(format!("`{term}` picks a component of an array"));
                    return None;
                }
                let start = field.start + index * 4;
                start..start + 4
            }
        };
        ranges.push(range);
    }
    Some(ranges)
}

/// Each field of the shader's `Placement` struct and its bytes in the uniform. Every
/// field is a `vec4<f32>` or an array of them, so they pack without padding.
fn uniform_fields(source: &str) -> HashMap<&str, Range<usize>> {
    let body = source
        .split_once("struct Placement {")
        .and_then(|(_, rest)| rest.split_once("};"))
        .map_or("", |(body, _)| body);
    let mut fields = HashMap::new();
    let mut offset = 0;
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        let Some((name, kind)) = line.split_once(':') else {
            continue;
        };
        let kind = kind.trim().trim_end_matches(',');
        let count = match kind.strip_prefix("array<vec4<f32>,") {
            Some(count) => count.trim_end_matches('>').trim().parse().unwrap_or(0),
            None if kind == "vec4<f32>" => 1,
            None => 0,
        };
        let length = count * VEC4;
        fields.insert(name.trim(), offset..offset + length);
        offset += length;
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(source: &str) -> Result<(), String> {
        let module = wgpu::naga::front::wgsl::parse_str(source)
            .map_err(|error| error.emit_to_string(source))?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .map(drop)
        .map_err(|error| error.emit_to_string(source))
    }

    #[test]
    fn the_shader_parses_and_every_variant_validates() {
        let (shader, problems) = LayerShader::parse(SOURCE, Placement::FULL.to_bytes());
        assert_eq!(problems, Vec::<String>::new());
        let fields = uniform_fields(SOURCE);
        let size = fields.values().map(|range| range.end).max();
        assert_eq!(size, Some(shader.neutral.len()));
        validate(&shader.source(Features::NONE)).unwrap();
        validate(&shader.source(shader.every_feature())).unwrap();
        for condition in 0..shader.conditions.len() {
            let alone = Features(1 << condition);
            validate(&shader.source(alone)).unwrap_or_else(|error| panic!("{error}"));
            let all_but = Features(!(1 << condition));
            validate(&shader.source(all_but)).unwrap_or_else(|error| panic!("{error}"));
        }
    }

    #[test]
    fn blocks_follow_the_fields_their_condition_names() {
        let source = "struct Placement {\n    a: vec4<f32>,\n    // b\n    b: array<vec4<f32>, 2>,\n};\n\
            one\n// #if a.y\ntwo\n// #else\nthree\n// #endif\n// #if b\nfour\n// #if a.y\nfive\n// #endif\n// #endif\n";
        let neutral = vec![0; 48];
        let (shader, problems) = LayerShader::parse(source, neutral.clone());
        assert_eq!(problems, Vec::<String>::new());
        let mut uniform = neutral.clone();
        let text = |uniform: &[u8]| {
            let source = shader.source(shader.features(uniform));
            source.lines().skip(5).collect::<Vec<_>>().join(" ")
        };
        assert_eq!(text(&uniform), "one three");
        uniform[0] = 1;
        assert_eq!(text(&uniform), "one three");
        uniform[4] = 1;
        assert_eq!(text(&uniform), "one two");
        uniform[40] = 1;
        assert_eq!(text(&uniform), "one two four five");
        uniform[4] = 0;
        assert_eq!(text(&uniform), "one three four");
    }

    #[test]
    fn unknown_fields_and_unbalanced_directives_are_reported_and_kept() {
        let source =
            "struct Placement {\n    a: vec4<f32>,\n};\n// #if nope\nkept\n// #endif\n// #endif\n";
        let (shader, problems) = LayerShader::parse(source, vec![0; 16]);
        assert_eq!(problems.len(), 2);
        assert!(shader.source(Features::NONE).contains("kept"));
    }
}
