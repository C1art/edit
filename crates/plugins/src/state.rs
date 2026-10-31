//! Framing for a plugin's saved state: `MAGIC`, then the component state and the
//! controller state, each prefixed by its length as a little-endian u64.

use crate::PluginError;

const MAGIC: &[u8; 8] = b"EDVST3S1";

pub(crate) fn encode(component: &[u8], controller: &[u8]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(MAGIC.len() + 16 + component.len() + controller.len());
    blob.extend_from_slice(MAGIC);
    for part in [component, controller] {
        blob.extend_from_slice(&(part.len() as u64).to_le_bytes());
        blob.extend_from_slice(part);
    }
    blob
}

/// Returns the component state and the controller state.
pub(crate) fn decode(blob: &[u8]) -> Result<(&[u8], &[u8]), PluginError> {
    let rest = blob.strip_prefix(MAGIC).ok_or(PluginError::InvalidState)?;
    let (component, rest) = split_part(rest)?;
    let (controller, rest) = split_part(rest)?;
    if rest.is_empty() {
        Ok((component, controller))
    } else {
        Err(PluginError::InvalidState)
    }
}

fn split_part(bytes: &[u8]) -> Result<(&[u8], &[u8]), PluginError> {
    let (len, rest) = bytes
        .split_first_chunk::<8>()
        .ok_or(PluginError::InvalidState)?;
    let len = usize::try_from(u64::from_le_bytes(*len)).map_err(|_| PluginError::InvalidState)?;
    if len > rest.len() {
        return Err(PluginError::InvalidState);
    }
    Ok(rest.split_at(len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_rejects_truncated_and_trailing_bytes() {
        let blob = encode(b"component", b"controller");
        assert_eq!(decode(&blob), Ok((&b"component"[..], &b"controller"[..])));
        assert_eq!(
            decode(&blob[..blob.len() - 1]),
            Err(PluginError::InvalidState)
        );
        let mut trailing = blob.clone();
        trailing.push(0);
        assert_eq!(decode(&trailing), Err(PluginError::InvalidState));
    }
}
