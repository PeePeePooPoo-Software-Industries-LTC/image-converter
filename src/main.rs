
use std::io::{stdin, Read};

use image::io::Reader;
use rfd::FileDialog;

macro_rules! rgb565_to_u16 {
	($r:expr, $g:expr, $b:expr) => {
		($r  * 2048 + $g * 32 + $b) as u16
	};
}

macro_rules! rgb_to_565 {
	($r:expr, $g:expr, $b:expr) => {
		(($r as f32 / 255.0 * 31.0) as u16, ($g as f32 / 255.0 * 63.0) as u16, ($b as f32 / 255.0 * 31.0) as u16)
	};
}

fn main() {
    let Some(files) = FileDialog::new()
		.add_filter("images", &["png", "bmp", "gif"])
		.set_title("Select images to convert")
		.pick_files() else { panic!("No file selected.") };

	let Some(output) = FileDialog::new()
		.set_title("Select a save location")
		.save_file() else { panic!("No save location selected.") };

	let mut all_content = String::new();
    for file in files {
		// That's gotta be the worst rust code I've ever seen
		// So it would seem
		// *HE'S A PIRATE STARTS PLAYING*
        let file_stem = { let Some(file_stem) = file.file_stem() else { panic!("Invalid name") }; file_stem.to_string_lossy() };
        let file_name = { let Some(file_name) = file.file_name() else { panic!("Invalid name") }; file_name.to_string_lossy() };
        let Ok(raw_reader) = Reader::open(&file) else { panic!("Invalid file") };
        let Ok(file_reader) = raw_reader.with_guessed_format() else { panic!("Invalid file format") };
        let Ok(decoded) = file_reader.decode() else { panic!("Invalid file format") };
        let Some(rgb) = decoded.as_rgb8() else { panic!("Invalid color codec") };

        all_content += &format!("\n// AUTO-GENERATED IMAGE CONVERTED FROM: {}\n", file_name);
        all_content += &format!("PROGMEM const uint16_t {name}[{len}] = {{\n", len = rgb.len() / 3, name = file_stem);
        for (_, row) in rgb.enumerate_rows() {
            let mut sum = String::from("\t");
            for (_, _, pixel) in row {
				let pixel = pixel.0;
				let (r, g, b) = rgb_to_565!(pixel[0], pixel[1], pixel[2]);
                sum = format!("{}{}, ", &sum, rgb565_to_u16!(r, g, b));
            }
            sum.pop();
            all_content += &format!("{}\n", sum);
        }
        all_content += "}};\n";
    }

	match std::fs::write(&output, &all_content) {
		Ok(_) => {
			println!("Saved succesfully to {:?}", &output);
			let _ = stdin().read(&mut [0u8]).unwrap();
		},
		Err(_) => {
			println!("Failed to save. Resorting to outputting to console (press something to continue)");
			let _ = stdin().read(&mut [0u8]).unwrap();
			println!("{}", &all_content);
			let _ = stdin().read(&mut [0u8]).unwrap();
		},
	};
}
