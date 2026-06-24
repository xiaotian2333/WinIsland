use std::cell::RefCell;
use std::time::Instant;

use skia_safe::{
    AlphaType, Color, ColorType, Data, ISize, Image, ImageInfo, Paint, image_filters, images,
    surfaces,
};

use crate::utils::gdi_capture::{ScreenCaptureRequest, capture_screen_bgra};

type GlassCacheEntry = (Image, Instant, i32, i32, u32, u32);

thread_local! {
    static GLASS_CACHE: RefCell<Option<GlassCacheEntry>> = const { RefCell::new(None) };
}

/// Frosted dark glass backdrop: captures the island region + margin from the
/// desktop, then applies a heavy blur (sigma ~40). A strong darkening blend
/// (Multiply + dark base) guarantees the signature dark glass look.
///
/// Note: WDA_EXCLUDEFROMCAPTURE is intentionally NOT set. It was previously
/// used to black out the island window during GDI capture, preventing
/// self-feedback. However, it introduced a one-frame lag on window transitions
/// (screenshot tools couldn't capture the island, and every GDI capture had to
/// toggle the affinity flag). The dark Multiply blend layer already masks any
/// residual self-capture artifacts, making WDA unnecessary for glass style.
pub fn get_glass_background(
    screen_x: i32,
    screen_y: i32,
    w: u32,
    h: u32,
    blur_sigma: f32,
) -> Option<Image> {
    if w == 0 || h == 0 {
        return None;
    }

    let cached = GLASS_CACHE.with(|cell| {
        let cache = cell.borrow();
        if let Some((img, time, cx, cy, cw, ch)) = cache.as_ref()
            && time.elapsed().as_millis() < 500
            && *cx == screen_x
            && *cy == screen_y
            && *cw == w
            && *ch == h
        {
            return Some(img.clone());
        }
        None
    });
    if let Some(img) = cached {
        return Some(img);
    }

    let result = capture_and_blur(screen_x, screen_y, w, h, blur_sigma);

    if let Some(ref img) = result {
        GLASS_CACHE.with(|cell| {
            *cell.borrow_mut() = Some((img.clone(), Instant::now(), screen_x, screen_y, w, h));
        });
    }

    result
}

/// Captures the island region + margin from the desktop, heavily blurs,
/// crops to the island area, then blends with a dark base colour to
/// guarantee the signature dark frosted-glass look.
fn capture_and_blur(sx: i32, sy: i32, w: u32, h: u32, blur_sigma: f32) -> Option<Image> {
    let downscale = 4u32;
    // Margin is wide enough that after heavy blur the blacked-out island
    // centre gets diluted by surrounding desktop content, producing a dark
    // tint instead of solid black.
    let margin = (w.max(h) / downscale) as i32;
    let cap_full_w = (w as i32 + 2 * margin).max(1);
    let cap_full_h = (h as i32 + 2 * margin).max(1);
    let cap_w = (cap_full_w / downscale as i32).max(1);
    let cap_h = (cap_full_h / downscale as i32).max(1);

    let capture = capture_screen_bgra(ScreenCaptureRequest {
        src_x: sx - margin,
        src_y: sy - margin,
        src_w: cap_full_w,
        src_h: cap_full_h,
        dst_w: cap_w,
        dst_h: cap_h,
        use_halftone: true,
        force_opaque_alpha: true,
    })?;

    let info = ImageInfo::new(
        ISize::new(capture.width, capture.height),
        ColorType::BGRA8888,
        AlphaType::Opaque,
        None,
    );
    let data = Data::new_copy(&capture.pixels);
    let src_img = images::raster_from_data(&info, data, (capture.width * 4) as usize)?;

    // Frosted glass: heavy blur (sigma ~40, stronger than Mica's ~6).
    let scaled_sigma = blur_sigma / downscale as f32;
    let mut blur_surface = surfaces::raster_n32_premul(ISize::new(capture.width, capture.height))?;
    let blur_canvas = blur_surface.canvas();
    let mut paint = Paint::default();
    if let Some(filter) = image_filters::blur((scaled_sigma, scaled_sigma), None, None, None) {
        paint.set_image_filter(filter);
    }
    blur_canvas.draw_image(&src_img, (0, 0), Some(&paint));
    let blurred = blur_surface.image_snapshot();

    let crop_x = (margin / downscale as i32) as f32;
    let crop_y = (margin / downscale as i32) as f32;
    let crop_w = (w / downscale).max(1) as i32;
    let crop_h = (h / downscale).max(1) as i32;

    let mut final_surface = surfaces::raster_n32_premul(ISize::new(crop_w, crop_h))?;
    let final_canvas = final_surface.canvas();
    final_canvas.draw_image(&blurred, (-crop_x, -crop_y), None);

    // Blend with a very dark base to guarantee the signature black glass
    // look even when WDA_EXCLUDEFROMCAPTURE doesn't fully black out the
    // island area on the current system.
    let mut darken = Paint::default();
    darken.set_color(Color::from_argb(195, 8, 8, 12));
    darken.set_anti_alias(true);
    darken.set_blend_mode(skia_safe::BlendMode::Multiply);
    final_canvas.draw_paint(&darken);

    Some(final_surface.image_snapshot())
}
