use ab_glyph::{self as ab, ScaleFont as _};
use harfbuzz_rs as hb;
use image::{GenericImageView as _, Rgba, RgbaImage};
use imageproc::drawing::Canvas as _;
use rustybuzz as rb;
use std::{ops::Add, path::Path};

const FACTOR: u32 = 4;

const MARGIN: u32 = FACTOR * 100;

// const IMG_WIDTH: u32 = FACTOR * 2000;
const LINE_HEIGHT: u32 = FACTOR * 160;

const FONT_SIZE: f32 = FACTOR as f32 * 80.0;

const _MSHQ_DEFAULT: f32 = 25.0;
const _SPAC_DEFAULT: f32 = -80.0;
macro_rules! my_file {
    () => {
        "kawthar"
    };
}
static TEXT: &str = include_str!(concat!("../texts/", my_file!(), ".txt"));

const TXT_COLOR: Rgba<u8> = Rgba([0x0A, 0x0A, 0x0A, 0xFF]);
const BKG_COLOR: Rgba<u8> = Rgba([0xFF, 0xFF, 0xF2, 0xFF]);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let full_text = TEXT.trim();
    let font_data = std::fs::read("fonts/Raqq.ttf")?;

    let mut rb_font = rb::Face::from_slice(&font_data, 0).ok_or("rustybuzz FAIL")?;
    let mut hb_font = hb::Font::new(hb::Face::from_bytes(&font_data, 0));

    let mut ab_font = ab::FontRef::try_from_slice(&font_data)?;

    let variations = [
        Variation {
            tag: *b"MSHQ",
            current_value: _MSHQ_DEFAULT,
        },
        Variation {
            tag: *b"SPAC",
            current_value: _SPAC_DEFAULT,
        },
    ];

    let canvas = write_in_image(
        full_text,
        &mut ab_font,
        &mut rb_font,
        &mut hb_font,
        variations,
    );

    canvas.save(Path::new(
        &(format!(
            "texts/rb_vs_hb__{}_MSHQ:{:.0}_SPAC:{:.0}.png",
            my_file!(),
            _MSHQ_DEFAULT,
            _SPAC_DEFAULT
        )),
    ))?;

    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct Variation {
    pub tag: [u8; 4],
    pub current_value: f32,
}

fn write_in_image(
    full_text: &str,
    ab_font: &mut (impl ab::Font + ab::VariableFont),
    rb_font: &mut rb::Face<'_>,
    hb_font: &mut hb::Owned<hb::Font<'_>>,
    variations: [Variation; 2],
) -> RgbaImage {
    for v in variations {
        ab_font.set_variation(&v.tag, v.current_value);
    }

    let ab_scale = ab_font.pt_to_px_scale(FONT_SIZE).unwrap();
    let ab_scaled_font = ab_font.as_scaled(ab_scale);

    let scale_factor = ab_scaled_font.scale_factor();
    let ascent = ab_scaled_font.ascent();

    // RUSTYBUZZ

    rb_font.set_variations(&variations.map(|v| rb::Variation {
        tag: rb::ttf_parser::Tag::from_bytes(&v.tag),
        value: v.current_value,
    }));

    let mut rb_buffer = rb::UnicodeBuffer::new();
    rb_buffer.push_str(full_text.trim());

    let rb_output = rb::shape(rb_font, &[], rb_buffer);

    // to align everything to the right. works around the weird shaping bug
    let line_width = rb_output
        .glyph_positions()
        .iter()
        .map(|p| p.x_advance as f32 * scale_factor.horizontal)
        .fold(0.0, Add::add) as u32;

    let img_width = line_width + 2 * MARGIN;
    let mut canvas = RgbaImage::from_pixel(img_width, 2 * LINE_HEIGHT + 2 * MARGIN, BKG_COLOR);

    let mut caret = 0;

    for (position, info) in rb_output
        .glyph_positions()
        .iter()
        .zip(rb_output.glyph_infos())
    {
        let gl = ab::GlyphId(info.glyph_id as u16).with_scale_and_position(
            ab_scale,
            ab::point(
                (caret + position.x_offset) as f32 * scale_factor.horizontal,
                ascent - (position.y_offset as f32 * scale_factor.vertical),
            ),
        );

        caret += position.x_advance;

        let Some(outlined_glyph) = ab_font.outline_glyph(gl) else {
            // gl is whitespace
            continue;
        };

        let bb = outlined_glyph.px_bounds();
        let bbx = bb.min.x as u32 + MARGIN;
        let bby = bb.min.y as u32 + MARGIN;

        outlined_glyph.draw(|px, py, pv| {
            let px = px + bbx;
            let py = py + bby;
            let pv = pv.clamp(0.0, 1.0);

            if canvas.in_bounds(px, py) {
                let pixel = canvas.get_pixel(px, py).to_owned();
                let weighted_color = imageproc::pixelops::interpolate(TXT_COLOR, pixel, pv);
                canvas.draw_pixel(px, py, weighted_color);
            }
        });
    }

    // HARFBUZZ

    hb_font.set_variations(
        &variations
            .iter()
            .map(|v| hb::Variation::new(&v.tag, v.current_value))
            .collect::<Vec<_>>(),
    );

    let hb_buffer = hb::UnicodeBuffer::new().add_str(full_text.trim());
    let hb_output = hb::shape(hb_font, hb_buffer, &[]);

    let mut caret = 0;

    for (position, info) in hb_output
        .get_glyph_positions()
        .iter()
        .zip(hb_output.get_glyph_infos())
    {
        let gl = ab::GlyphId(info.codepoint as u16).with_scale_and_position(
            ab_scale,
            ab::point(
                (caret + position.x_offset) as f32 * scale_factor.horizontal,
                ascent - (position.y_offset as f32 * scale_factor.vertical),
            ),
        );

        caret += position.x_advance;

        let Some(outlined_glyph) = ab_font.outline_glyph(gl) else {
            // gl is whitespace
            continue;
        };

        let bb = outlined_glyph.px_bounds();
        let bbx = bb.min.x as u32 + MARGIN;
        let bby = bb.min.y as u32 + MARGIN + LINE_HEIGHT;

        outlined_glyph.draw(|px, py, pv| {
            let px = px + bbx;
            let py = py + bby;
            let pv = pv.clamp(0.0, 1.0);

            if canvas.in_bounds(px, py) {
                let pixel = canvas.get_pixel(px, py).to_owned();
                let weighted_color = imageproc::pixelops::interpolate(TXT_COLOR, pixel, pv);
                canvas.draw_pixel(px, py, weighted_color);
            }
        });
    }

    // THE END

    canvas
}
