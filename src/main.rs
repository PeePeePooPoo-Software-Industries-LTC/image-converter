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

enum OptMode {
    Basic,
    Optimised,
}

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
		.add_filter("basic", &["bsc"])
		.set_title("Select a save location")
		.save_file() else { panic!("No save location selected.") };

    // let files = vec![std::path::PathBuf::from(
    //     "\\\\wsl.localhost\\Ubuntu-20.04\\home\\jaschutte\\School\\croaker\\images\\frogge.bmp",
    // )];
    // let output = std::path::PathBuf::from(
    //     "\\\\wsl.localhost\\Ubuntu-20.04\\home\\jaschutte\\School\\croaker\\images\\output.opt",
    // );

    let opt_mode = match output.extension() {
        Some(ext) => match ext.to_str() {
            Some("bsc") => OptMode::Basic,
            Some("opt") => OptMode::Optimised,
            _ => {
                println!("Invalid file extension selected, resorting to 'opt'.");
                OptMode::Optimised
            }
        },
        None => {
            println!("No file extension selected, resorting to 'opt'.");
            OptMode::Optimised
        }
    };

    let mut all_content = String::new();
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

        if let OptMode::Basic = opt_mode {
            all_content += &format!(
                "PROGMEM const uint16_t {name}[{len}] = {{\n",
                len = rgb.len() / 3,
                name = file_stem
            );
            // Just loop through every pixel and write it down
            for (_, row) in rgb.enumerate_rows() {
                let mut sum = String::from("\t");
                for (_, _, pixel) in row {
                    let pixel = pixel.0;
                    // Convert [0..255] to [0..31] for RB and [0..63] for G
                    let (r, g, b) = rgb_to_565!(pixel[0], pixel[1], pixel[2]);
                    // Paste
                    sum = format!("{}{}, ", &sum, rgb565_to_u16!(r, g, b));
                }
                sum.pop();
                sum.pop();
                all_content += &format!("{}\n", sum);
            }
            all_content += "}};\n";
        } else {
            const WINDOW_SIZE: usize = 4;
            let mut color_palette: [Option<u16>; OPT_MAX_COLORS] = [None; OPT_MAX_COLORS];
            let mut window = [IMPOSSIBLE_INDEX; WINDOW_SIZE]; // We init at 8, an impossible index to search as we only allow 8 colors
            let mut matching_pixels = 1;
            let mut pixel_idx = -4;
            let mut pixel_converter = |pixel| {
                pixel_idx += 1;
                // Not worth writing a loop for
                window[0] = window[1];
                window[1] = window[2];
                window[2] = window[3];
                window[3] = pixel;

                // println!("#{:?} {}", &window, matching_pixels);
                if window[0] == window[1] {
                    matching_pixels += 1;
                    if matching_pixels == 15 {
                        // RANGE BYTE
                        // [ RESV | RAN1 | RAN2 | RAN3 | RAN4 | COL1 | COL2 | COL3 ]
                        let char = RANGE_SIG_BIT
                            + (matching_pixels << RANGE_RANGE_OFFSET)
                            + (window[0] << RANGE_COL_OFFSET);
                        matching_pixels = 0;
                        Some(char)
                    } else if (matching_pixels == 2 || matching_pixels == 1)
                        && window[1] != window[2]
                    {
                        // COLOR BYTE
                        // [ RESV | COL1 | COL2 | COL3 | COPY | COL1 | COL2 | COL3 ]
                        let char = COLOR_SIG_BIT
                            + (window[0] << COLOR_FIRST_COL_OFFSET)
                            + (window[1] << COLOR_SECOND_COL_OFFSET);
                        window[0] = IMPOSSIBLE_INDEX;
                        if matching_pixels == 1 {
                            window[1] = IMPOSSIBLE_INDEX;
                        }
                        matching_pixels = 1;
                        Some(char)
                    } else {
                        None
                    }
                } else {
                    let real_matching_count = matching_pixels + 1;
                    matching_pixels = 0;

                    if real_matching_count >= 5 {
                        // RANGE BYTE
                        // [ RESV | RAN1 | RAN2 | RAN3 | RAN4 | COL1 | COL2 | COL3 ]
                        let char = RANGE_SIG_BIT
                            + (real_matching_count << RANGE_RANGE_OFFSET)
                            + (window[0] << RANGE_COL_OFFSET);
                        window[0] = IMPOSSIBLE_INDEX;
                        Some(char)
                    } else if window[0] == window[2] && window[1] == window[3] {
                        // COLOR BYTE
                        // [ RESV | COL1 | COL2 | COL3 | COPY | COL1 | COL2 | COL3 ]
                        let char = COLOR_SIG_BIT
                            + (window[0] << COLOR_FIRST_COL_OFFSET)
                            + COPY_BIT
                            + (window[1] << COLOR_SECOND_COL_OFFSET);
                        window[0] = IMPOSSIBLE_INDEX;
                        // window = [IMPOSSIBLE_INDEX; WINDOW_SIZE];
                        Some(char)
                    } else if window[0] != IMPOSSIBLE_INDEX {
                        if real_matching_count == 4 {
                            // COLOR BYTE
                            // [ RESV | COL1 | COL2 | COL3 | COPY | COL1 | COL2 | COL3 ]
                            let char = COLOR_SIG_BIT
                                + (window[0] << COLOR_FIRST_COL_OFFSET)
                                + COPY_BIT
                                + (window[0] << COLOR_SECOND_COL_OFFSET);
                            window[0] = IMPOSSIBLE_INDEX;
                            // window[1] = IMPOSSIBLE_INDEX;
                            // window = [IMPOSSIBLE_INDEX; WINDOW_SIZE];
                            Some(char)
                        } else {
                            // COLOR BYTE
                            // [ RESV | COL1 | COL2 | COL3 | COPY | COL1 | COL2 | COL3 ]
                            let char = COLOR_SIG_BIT
                                + (window[0] << COLOR_FIRST_COL_OFFSET)
                                + (window[1] << COLOR_SECOND_COL_OFFSET);
                            window[0] = IMPOSSIBLE_INDEX;
                            window[1] = IMPOSSIBLE_INDEX;
                            Some(char)
                        }
                    } else {
                        None
                    }
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
                .chain([IMPOSSIBLE_INDEX; 4])
                .filter_map(&mut pixel_converter)
                .collect::<Vec<u8>>();

            // Make sure we empty the entire buffer
            // for pixel in [IMPOSSIBLE_INDEX; 4] {
            //     if let Some(char) = &mut pixel_converter(pixel) {
            //         image_data.push(*char);
            //     }
            // }

            // Convert the image data into a C array
            let (w, h) = rgb.dimensions();
            all_content += &format!("// Image dimensions: {}, {}\n", w, h);
            all_content += &format!(
                "#define {}_SIZE {}\n",
                file_stem.to_uppercase(),
                image_data.len() + 16
            );
            all_content += &format!(
                "PROGMEM const char {name}[{upper}_SIZE] = {{\n\t",
                upper = file_stem.to_uppercase(),
                name = file_stem
            );
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

            // VERBOSE OUTPUT
            // fora
        }
    }

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
