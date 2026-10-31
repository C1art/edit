use vst3::Steinberg::{
    kInvalidArgument, kResultTrue, tresult, IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewTrait,
    ViewRect,
};
use vst3::{Class, ComRef};

use super::window;

/// Lets the editor ask for its window to be resized.
pub(super) struct Frame {
    window: isize,
    style: u32,
}

impl Frame {
    pub(super) fn new(window: isize, style: u32) -> Self {
        Self { window, style }
    }
}

impl Class for Frame {
    type Interfaces = (IPlugFrame,);
}

impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        if new_size.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null pointer to a ViewRect the plugin supplies for this call.
        let size = unsafe { *new_size };
        window::resize_for_view(self.window, self.style, &size);
        // SAFETY: the plugin passes its own view, which is alive for this call.
        if let Some(view) = unsafe { ComRef::from_raw(view) } {
            // SAFETY: `new_size` is the plugin's valid rect; the spec has the host confirm
            // a resize it asked for with onSize.
            unsafe { view.onSize(new_size) };
        }
        kResultTrue
    }
}
