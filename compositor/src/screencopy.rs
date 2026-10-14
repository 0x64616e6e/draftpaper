//! wlr-screencopy (v3): a client (grim, a region picked with slurp, the wlroots screenshot portal) copies
//! an output, or a region of it, into a buffer of its own. Each copy is drawn offscreen as the Print
//! screenshot is (tty::render_pixels), with the pointer if asked for, at the backend's next frame, and
//! written into the client's shm buffer; dmabuf frames are not offered.

use std::sync::atomic::{AtomicBool, Ordering};

use smithay::{
    backend::renderer::{element::memory::MemoryRenderBuffer, gles::GlesRenderer},
    output::Output,
    reexports::{
        wayland_protocols_wlr::screencopy::v1::server::{
            zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
            zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
        },
        wayland_server::{
            protocol::{wl_buffer::WlBuffer, wl_shm},
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
        },
    },
    utils::{Logical, Physical, Point, Rectangle},
    wayland::shm::{with_buffer_contents, with_buffer_contents_mut},
};

use crate::Draftpaper;

/// What a frame copies: an output, a region of it in physical pixels, with the pointer or not.
pub struct FrameState {
    output: Option<Output>,
    region: Rectangle<i32, Physical>,
    cursor: bool,
    used: AtomicBool,
}

/// A copy waiting for the backend's next frame.
pub struct PendingCopy {
    frame: ZwlrScreencopyFrameV1,
    buffer: WlBuffer,
    output: Output,
    region: Rectangle<i32, Physical>,
    cursor: bool,
    damage: bool,
}

pub fn init(dh: &DisplayHandle) {
    dh.create_global::<Draftpaper, ZwlrScreencopyManagerV1, ()>(3, ());
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for Draftpaper {
    fn bind(_: &mut Self, _: &DisplayHandle, _: &Client, manager: New<ZwlrScreencopyManagerV1>, _: &(), init: &mut DataInit<'_, Self>) {
        init.init(manager, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for Draftpaper {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        use zwlr_screencopy_manager_v1::Request;
        let (frame, cursor, output, region) = match request {
            Request::CaptureOutput { frame, overlay_cursor, output } => (frame, overlay_cursor, output, None),
            Request::CaptureOutputRegion { frame, overlay_cursor, output, x, y, width, height } => {
                (frame, overlay_cursor, output, Some(Rectangle::<i32, Logical>::new((x, y).into(), (width, height).into())))
            }
            _ => return,
        };
        let output = Output::from_resource(&output);
        // the whole output in physical pixels, or the region asked for (logical, from the output's corner)
        let region = output.as_ref().and_then(|o| {
            let mode = o.current_mode()?;
            let whole = Rectangle::<i32, Physical>::from_size((mode.size.w, mode.size.h).into());
            match region {
                None => Some(whole),
                Some(r) => r.to_f64().to_physical(o.current_scale().fractional_scale()).to_i32_round().intersection(whole),
            }
        });
        let state = FrameState { output: output.clone(), region: region.unwrap_or_default(), cursor: cursor != 0, used: AtomicBool::new(false) };
        let frame = init.init(frame, state);
        match region.filter(|r| !r.is_empty()) {
            Some(r) => {
                frame.buffer(wl_shm::Format::Xrgb8888, r.size.w as u32, r.size.h as u32, r.size.w as u32 * 4);
                if frame.version() >= 3 {
                    frame.buffer_done();
                }
            }
            None => frame.failed(),
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, FrameState> for Draftpaper {
    fn request(
        state: &mut Self,
        _: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &FrameState,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        use zwlr_screencopy_frame_v1::Request;
        let (buffer, damage) = match request {
            Request::Copy { buffer } => (buffer, false),
            Request::CopyWithDamage { buffer } => (buffer, true),
            _ => return,
        };
        if data.used.swap(true, Ordering::SeqCst) {
            frame.post_error(zwlr_screencopy_frame_v1::Error::AlreadyUsed, "this frame was already copied");
            return;
        }
        let Some(output) = data.output.clone() else {
            frame.failed();
            return;
        };
        // a shm buffer of the size and a format offered
        let (w, h) = (data.region.size.w, data.region.size.h);
        let fits = with_buffer_contents(&buffer, |_, _, b| {
            b.width == w && b.height == h && b.stride >= w * 4 && matches!(b.format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888)
        });
        if !matches!(fits, Ok(true)) {
            frame.post_error(zwlr_screencopy_frame_v1::Error::InvalidBuffer, "the buffer is not the shm buffer offered");
            return;
        }
        state.screencopies.push(PendingCopy { frame: frame.clone(), buffer, output, region: data.region, cursor: data.cursor, damage });
    }
}

/// Fill the waiting copies (the backend calls this with its renderer before drawing a frame).
pub fn finish(data: &mut Draftpaper, renderer: &mut GlesRenderer, pointer: Option<(&MemoryRenderBuffer, Point<i32, Logical>)>) {
    let copies = std::mem::take(&mut data.screencopies);
    for copy in copies {
        if !copy.frame.is_alive() {
            continue;
        }
        let drawn = crate::tty::render_pixels(data, renderer, &copy.output, pointer.filter(|_| copy.cursor), false);
        let Ok((pixels, width, _)) = drawn else {
            copy.frame.failed();
            continue;
        };
        let r = copy.region;
        // RGBA rows of the whole output into the client's XRGB (little endian: B, G, R, X) rows
        let written = with_buffer_contents_mut(&copy.buffer, |ptr, len, b| {
            let dst = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
            for y in 0..r.size.h {
                let from = (((r.loc.y + y) * width + r.loc.x) * 4) as usize;
                let to = (b.offset + y * b.stride) as usize;
                let (src, out) = (&pixels[from..from + r.size.w as usize * 4], &mut dst[to..to + r.size.w as usize * 4]);
                for (s, d) in src.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
                    d.copy_from_slice(&[s[2], s[1], s[0], 255]);
                }
            }
        });
        if written.is_err() {
            copy.frame.failed();
            continue;
        }
        copy.frame.flags(zwlr_screencopy_frame_v1::Flags::empty());
        if copy.damage {
            copy.frame.damage(0, 0, r.size.w as u32, r.size.h as u32);
        }
        let now = std::time::Duration::from(smithay::utils::Clock::<smithay::utils::Monotonic>::new().now());
        copy.frame.ready((now.as_secs() >> 32) as u32, now.as_secs() as u32, now.subsec_nanos());
    }
}
