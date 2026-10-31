use std::sync::Mutex;

/// Pictures freed lately, kept for the decoder to write its next ones into: a fresh
/// 4K picture costs about 2 ms of page faults before it holds a single pixel.
static SPARE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

/// Smaller pictures are cheap enough to allocate.
const SMALLEST: usize = 1 << 20;
/// Enough for a few 4K pictures in flight; an idle editor holds no more than this.
const MOST_BYTES: usize = 160 << 20;

/// A picture buffer of `len` bytes, with whatever it last held when one was spare.
pub(crate) fn take(len: usize) -> Vec<u8> {
    let reused = (len >= SMALLEST)
        .then(|| SPARE.try_lock().ok())
        .flatten()
        .and_then(|mut spare| {
            let at = spare.iter().rposition(|pixels| pixels.len() == len)?;
            Some(spare.swap_remove(at))
        });
    reused.unwrap_or_else(|| vec![0; len])
}

/// Keeps `pixels` for a later `take`, unless it's small, or another thread is in the
/// middle of taking or giving: freeing is never worth a wait.
pub(crate) fn give(pixels: Vec<u8>) {
    if pixels.len() < SMALLEST {
        return;
    }
    let Ok(mut spare) = SPARE.try_lock() else {
        return;
    };
    spare.push(pixels);
    let mut held: usize = spare.iter().map(Vec::len).sum();
    while held > MOST_BYTES {
        held -= spare.remove(0).len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_given_picture_is_taken_back_only_at_its_size_and_the_oldest_go_first() {
        // A size no other test's pictures have, as the spares are shared.
        let len = SMALLEST * 3 + 4;
        let marked = |mark: usize| {
            let mut pixels = vec![0; len];
            pixels[0] = u8::try_from(mark).unwrap();
            pixels
        };
        give(marked(7));
        assert_eq!(take(len + 4)[0], 0);
        assert_eq!(take(len)[0], 7);

        let count = MOST_BYTES / len + 2;
        (1..=count).for_each(|mark| give(marked(mark)));
        let kept: Vec<u8> =
            std::iter::from_fn(|| Some(take(len)[0]).filter(|&mark| mark > 0)).collect();
        assert_eq!(kept.len(), MOST_BYTES / len);
        assert!(!kept.contains(&1));
    }
}
