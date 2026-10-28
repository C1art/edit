use crate::RenderError;

/// Grading tools offer LUTs from 2 to 256 points per side; larger ones are not made.
const MAX_SIZE: u32 = 256;

/// A 3D lookup table from gamma-encoded RGB to RGB, red changing fastest as in `.cube`
/// files and in a 3D texture's rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Lut3d {
    pub(crate) size: u32,
    pub(crate) table: Vec<[f32; 3]>,
}

impl Lut3d {
    /// Reads an Adobe/Resolve `.cube` 3D LUT over the domain 0 to 1.
    pub fn parse_cube(text: &str) -> Result<Self, RenderError> {
        let mut size = None;
        let mut table = Vec::new();
        let mut last_line = 0;
        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            let invalid = |reason| RenderError::InvalidLut {
                line: line_number,
                reason,
            };
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            last_line = line_number;
            let mut words = line.split_whitespace();
            let Some(first) = words.next() else {
                continue;
            };
            if first.starts_with(|c: char| c.is_ascii_alphabetic()) {
                let numbers: Option<Vec<f32>> = words.map(|word| word.parse().ok()).collect();
                let numbers = numbers.unwrap_or_default();
                match first {
                    "LUT_3D_SIZE" => {
                        let side = numbers
                            .first()
                            .map(|&side| side as u32)
                            .filter(|side| (2..=MAX_SIZE).contains(side));
                        size = Some(side.ok_or_else(|| invalid("size must be 2 to 256"))?);
                    }
                    "LUT_1D_SIZE" => return Err(invalid("only 3D LUTs are supported")),
                    "DOMAIN_MIN" if numbers.iter().any(|&value| value != 0.0) => {
                        return Err(invalid("only a domain from 0 to 1 is supported"));
                    }
                    "DOMAIN_MAX" if numbers.iter().any(|&value| value != 1.0) => {
                        return Err(invalid("only a domain from 0 to 1 is supported"));
                    }
                    "LUT_3D_INPUT_RANGE" if numbers != [0.0, 1.0] => {
                        return Err(invalid("only an input range from 0 to 1 is supported"));
                    }
                    // TITLE and vendor keywords don't change the table.
                    _ => {}
                }
                continue;
            }
            let values: Option<Vec<f32>> = line
                .split_whitespace()
                .map(|word| word.parse().ok())
                .collect();
            match values.as_deref() {
                Some(&[red, green, blue]) => table.push([red, green, blue]),
                _ => return Err(invalid("expected three numbers")),
            }
        }
        let size = size.ok_or(RenderError::InvalidLut {
            line: last_line,
            reason: "missing LUT_3D_SIZE",
        })?;
        if table.len() != (size as usize).pow(3) {
            return Err(RenderError::InvalidLut {
                line: last_line,
                reason: "the table doesn't have size³ entries",
            });
        }
        Ok(Self { size, table })
    }

    pub fn size(&self) -> u32 {
        self.size
    }
}

/// Packs RGB as half-float RGBA, opaque, for a filterable `Rgba16Float` texture.
pub(crate) fn half_rgba(rgb: &[[f32; 3]]) -> Vec<u8> {
    rgb.iter()
        .flat_map(|&[red, green, blue]| [red, green, blue, 1.0])
        .flat_map(|value| half_bits(value).to_le_bytes())
        .collect()
}

/// IEEE 754 binary16, rounding to nearest; values past the largest half become
/// infinite and tiny ones subnormal or zero.
fn half_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    if exponent >= 31 {
        sign | 0x7c00
    } else if exponent <= 0 {
        if exponent < -10 {
            return sign;
        }
        let full = (mantissa | 0x80_0000) >> (1 - exponent);
        sign | ((full + 0x1000) >> 13) as u16
    } else {
        // A carry out of the mantissa rounds up into the exponent, as it should.
        let rounded = ((exponent as u32) << 10 | mantissa >> 13) + ((mantissa >> 12) & 1);
        sign | rounded as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_files_read_with_comments_titles_and_domains() {
        let text = "# made by hand\nTITLE \"Warm\"\nLUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\n\
                    DOMAIN_MAX 1.0 1.0 1.0\n\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n\
                    0 1 1\n1e0 1 0.5\n";
        let lut = Lut3d::parse_cube(text).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.table[7], [1.0, 1.0, 0.5]);
        let short = "LUT_3D_SIZE 2\n0 0 0\n";
        assert!(matches!(
            Lut3d::parse_cube(short),
            Err(RenderError::InvalidLut { line: 2, .. })
        ));
        assert!(Lut3d::parse_cube("LUT_1D_SIZE 1024\n").is_err());
    }

    #[test]
    fn halves_round_to_nearest() {
        assert_eq!(half_bits(1.0), 0x3c00);
        assert_eq!(half_bits(-0.5), 0xb800);
        assert_eq!(half_bits(65_504.0), 0x7bff);
        assert_eq!(half_bits(1e6), 0x7c00);
        // Just over halfway between 1 and the next half rounds up.
        assert_eq!(half_bits(1.0 + 1.6 / 1024.0), 0x3c02);
        // The smallest subnormal half.
        assert_eq!(half_bits(2f32.powi(-24)), 0x0001);
    }
}
