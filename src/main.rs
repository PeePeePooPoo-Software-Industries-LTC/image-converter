use std::io::{stdin, Read};

use image::{io::Reader, Rgb};
use rfd::FileDialog;

// An index not reached by the 'opt' level
const OPT_MAX_COLORS: usize = 8;
const IMPOSSIBLE_INDEX: u8 = OPT_MAX_COLORS as u8;
// Format 'opt' bit signatures
const COPY_BIT: u8 = 0x08;
const COLOR_SIG_BIT: u8 = 0x00;
const COLOR_FIRST_COL_OFFSET: u8 = 4;
const COLOR_SECOND_COL_OFFSET: u8 = 0;
const RANGE_SIG_BIT: u8 = 0x80;
const RANGE_COL_OFFSET: u8 = 0;
const RANGE_RANGE_OFFSET: u8 = 3;

macro_rules! rgb565_to_u16 {
    ($r:expr, $g:expr, $b:expr) => {
        ($r * 2048 + $g * 32 + $b) as u16
    };
}

macro_rules! rgb_to_565 {
    ($r:expr, $g:expr, $b:expr) => {
        (
            ($r as f32 / 255.0 * 31.0) as u16,
            ($g as f32 / 255.0 * 63.0) as u16,
            ($b as f32 / 255.0 * 31.0) as u16,
        )
    };
}

#[derive(Clone, Copy)]
struct PixelRange {
    range: u8,
    color: u8,
}

impl PixelRange {
    fn empty() -> Self {
        PixelRange { range: 0, color: 0 }
    }
}

fn get_palette_index(color_palette: &mut [Option<u16>; 8], pixel_color: u16) -> Option<usize> {
    for (idx, palette_color) in color_palette.iter_mut().enumerate() {
        match palette_color {
            Some(palette_color) => {
                if *palette_color == pixel_color {
                    return Some(idx);
                }
            } // Oh, it's already in the palette, we're done here
            None => {
                // New color, add!
                *palette_color = Some(pixel_color);
                return Some(idx);
            }
        }
    }
    None
}

fn main() {
    let Some(files) = FileDialog::new()
    	.add_filter("images", &["png", "bmp", "gif"])
    	.set_title("Select images to convert")
    	.pick_files() else { panic!("No file selected.") };

    let Some(output) = FileDialog::new()
    	.add_filter("optimised", &["opt"])
    	.set_title("Select a save location")
    	.save_file() else { panic!("No save location selected.") };

    let mut all_content = String::new();
    let mut all_content_cpp = String::new();
    for file in files {
        // That's gotta be the worst rust code I've ever seen
        // So it would seem
        // *HE'S A PIRATE STARTS PLAYING*
        let file_stem = {
            let Some(file_stem) = file.file_stem() else { panic!("Invalid name") };
            file_stem.to_string_lossy()
        };
        let file_name = {
            let Some(file_name) = file.file_name() else { panic!("Invalid name") };
            file_name.to_string_lossy()
        };
        let Ok(raw_reader) = Reader::open(&file) else { panic!("Invalid file") };
        let Ok(file_reader) = raw_reader.with_guessed_format() else { panic!("Invalid file format") };
        let Ok(decoded) = file_reader.decode() else { panic!("Invalid file format") };
        let Some(rgb) = decoded.as_rgb8() else { panic!("Invalid color codec") };

        // Basic note
        all_content += &format!("\n// AUTO-GENERATED IMAGE CONVERTED FROM: {}\n", file_name);

        const WINDOW_SIZE: usize = 4;
        let mut color_palette: [Option<u16>; OPT_MAX_COLORS] = [None; OPT_MAX_COLORS];

        let mut prev_pixel = IMPOSSIBLE_INDEX;
        let mut matching_pixels = 1;
        let mut pixel_converter = |pixel| {
            // Count the pixels in a row, starting from 1 (the current pixel)
            let different = prev_pixel != pixel;

            let return_val =
                if (different || matching_pixels == 15) && prev_pixel != IMPOSSIBLE_INDEX {
                    Some(PixelRange {
                        range: matching_pixels,
                        color: prev_pixel,
                    })
                } else {
                    None
                };

            matching_pixels = if different || matching_pixels == 15 {
                1
            } else {
                matching_pixels + 1
            };

            prev_pixel = pixel;
            return_val
        };

        // Compact some bytes into color bytes
        let mut window = [PixelRange::empty(); WINDOW_SIZE];
        let mut pixel_compactor = |range_byte| {
            // We look FORWARDS not BACKWARDS
            window[0] = window[1];
            window[1] = window[2];
            window[2] = window[3];
            window[3] = range_byte;

            // Check for an xoxo pattern
            if window[0].range == 1
                && window[1].range == 1
                && window[2].range == 1
                && window[3].range == 1
                && window[0].color == window[2].color
                && window[1].color == window[3].color
            {
                let color0 = window[0].color;
                let color1 = window[1].color;
                window = [PixelRange::empty(); WINDOW_SIZE];
                Some([COLOR_SIG_BIT
                    + (color0 << COLOR_FIRST_COL_OFFSET)
                    + COPY_BIT
                    + (color1 << COLOR_SECOND_COL_OFFSET)])
            // Handle inefficient range(1) bytes
            } else if window[0].range == 1 && window[1].range >= 1 {
                let color0 = window[0].color;
                let color1 = window[1].color;
                // Since we draw the next pixel, remove one from it's counter
                window[1].range -= 1;
                Some([COLOR_SIG_BIT
                    + (color0 << COLOR_FIRST_COL_OFFSET)
                    + (color1 << COLOR_SECOND_COL_OFFSET)])
            // Copy paste efficient range(x) bytes
            } else if window[0].range != 0 {
                Some([RANGE_SIG_BIT
                    + (window[0].range << RANGE_RANGE_OFFSET)
                    + (window[0].color << RANGE_COL_OFFSET)])
            } else {
                None
            }
        };

        // Get a color palette from the raw image
        let get_color_palette = |(pixel_x, pixel_y, pixel): (u32, u32, &Rgb<u8>)| {
            let pixel = pixel.0;
            let (r, g, b) = rgb_to_565!(pixel[0], pixel[1], pixel[2]);
            let color = rgb565_to_u16!(r, g, b);

            match get_palette_index(&mut color_palette, color) {
                Some(idx) => idx as u8,
                None => {
                    println!(
                        "WARNING: A 9th pixel color located at ({}, {}). Using random color.",
                        pixel_x, pixel_y
                    );
                    0
                }
            }
        };

        // Function programming power ftw!
        let image_data = rgb
            .enumerate_pixels()
            .map(get_color_palette)
            // Make sure we have enough to for the last pixel to also get the prev_pixel
            .chain([IMPOSSIBLE_INDEX; 1])
            .filter_map(&mut pixel_converter)
            // Since the buffer needs to look into the future, we append 4 to the end
            .chain([PixelRange::empty(); 4])
            .filter_map(&mut pixel_compactor)
            .flatten()
            .collect::<Vec<u8>>();

        // Convert the image data into a C array
        let (w, h) = rgb.dimensions();
        let image_len = image_data.len() + 16;
        all_content_cpp += &format!(
            "// AUTO GENERATED IMAGE, PUT ME IN images.cpp
RawImage image_{file_stem} {{
	GET_IMAGE({file_stem}),
	{image_len},
	Vector2 {{ {w}, {h} }}
}};
",
        );
        all_content += "// AUTO GENERATED IMAGE, PUT ME IN images.h\n";
        all_content += &format!("extern RawImage image_{};\n", file_stem,);
        all_content += &format!("PROGMEM const char __raw_{file_stem}_p[{image_len}] = {{\n\t",);
        for color in color_palette {
            all_content += &format!(
                "0x{:02x}, 0x{:02x}, ",
                match color {
                    Some(c) => c >> 8,
                    None => 0,
                },
                match color {
                    Some(c) => c & 0xFF,
                    None => 0,
                },
            );
        }
        all_content.pop();
        all_content += "\n\t";
        for encoded_pixel in &image_data {
            all_content += &format!("0x{:02x}, ", *encoded_pixel);
        }
        all_content.pop();
        all_content.pop();
        all_content += "\n};\n";
    }
    all_content += all_content_cpp.as_str();

    match std::fs::write(&output, &all_content) {
        Ok(_) => {
            println!("Saved succesfully to {:?}", &output);
            // let _ = stdin().read(&mut [0u8]).unwrap();
        }
        Err(_) => {
            println!(
                "Failed to save. Resorting to outputting to console (press something to continue)"
            );
            let _ = stdin().read(&mut [0u8]).unwrap();
            println!("{}", &all_content);
            let _ = stdin().read(&mut [0u8]).unwrap();
        }
    };
}
