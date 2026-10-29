//! swscale's flag bits, spelled out because FFmpeg 8 turned the `#define`s into the
//! `SwsFlags` enum, which the generated bindings expose under a different path. The
//! values are part of the ABI and are the same in both.

use std::ffi::c_int;

pub(super) const SWS_BILINEAR: c_int = 1 << 1;
pub(super) const SWS_BICUBIC: c_int = 1 << 2;
pub(super) const SWS_FULL_CHR_H_INT: c_int = 1 << 13;
pub(super) const SWS_FULL_CHR_H_INP: c_int = 1 << 14;
pub(super) const SWS_ACCURATE_RND: c_int = 1 << 18;
