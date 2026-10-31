use std::sync::Arc;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// What a VST3 plugin saved of itself, kept as the plugin wrote it. Shared, so copies of
/// a sequence don't copy it; a project carries it as base64.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PluginState(Arc<[u8]>);

impl PluginState {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes.into())
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for PluginState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for PluginState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        decode(&text)
            .map(Self::new)
            .ok_or_else(|| D::Error::custom("plugin state is not base64"))
    }
}

fn encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut group = [0; 3];
        group[..chunk.len()].copy_from_slice(chunk);
        let bits = u32::from_be_bytes([0, group[0], group[1], group[2]]);
        for index in 0..4 {
            let shown = index <= chunk.len();
            let sextet = (bits >> (18 - 6 * index)) & 0x3f;
            text.push(if shown {
                char::from(ALPHABET[sextet as usize])
            } else {
                '='
            });
        }
    }
    text
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if !text.len().is_multiple_of(4) {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
    let groups = text.len() / 4;
    for (index, group) in text.chunks(4).enumerate() {
        let padding = group.iter().rev().take_while(|&&c| c == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != groups) {
            return None;
        }
        let mut bits = 0u32;
        for &c in &group[..4 - padding] {
            let sextet = ALPHABET.iter().position(|&letter| letter == c)?;
            bits = bits << 6 | sextet as u32;
        }
        bits <<= 6 * padding;
        bytes.extend_from_slice(&bits.to_be_bytes()[1..4 - padding]);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_tail_length_and_rejects_garbage() {
        let bytes: Vec<u8> = (0..=255).collect();
        for length in 0..8 {
            let text = encode(&bytes[250 - length..250]);
            assert_eq!(decode(&text).as_deref(), Some(&bytes[250 - length..250]));
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        for bad in ["TWF", "TW=u", "T===", "TWE=TWFu", "TW!u"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }
}
