use std::env;
use std::fs;
use std::path::Path;
use execute::Execute;
use toml::Table;
use chrono::prelude::Local;
use execute::shell;
use rand::seq::SliceRandom;
use std::{ thread, time };

const DAYPARTS: &[&str] = &["morning", "day", "evening", "night"];

fn path_exists(path: &String) -> bool {
	return Path::new(path).exists();
}

//Ensures that the config directory exists and is set up properly
fn configure(path: &String) -> (Table, [Vec<String>; 4], [Vec<String>; 24]) {
	let dayparts_path = "".to_owned() + path + "/dayparts";
	let hours_path = "".to_owned() + path + "/hours";
	let config_path = "".to_owned() + path + "/config.toml";

	let mut config: Table = Default::default();

	if !path_exists(path) {
		fs::create_dir(path).expect("Failed to create config folder!");
	}
	if !path_exists(&dayparts_path) {
		fs::create_dir(&dayparts_path).expect("Failed to create dayparts folder!");
		for i in DAYPARTS {
			fs::create_dir(format!("{}/{}", dayparts_path, i)).expect(
				&format!("Failed to create dayparts subfolder: {}", i)
			);
		}
	}
	if !path_exists(&hours_path) {
		fs::create_dir(&hours_path).expect("Failed to create hours folder!");
		for i in 0..24 {
			fs::create_dir(format!("{}/{}", &hours_path, i)).expect(
				&format!("Failed to create hours subfolder: {:?}", i)
			);
		}
	}
	if !path_exists(&config_path) {
		fs::copy("./default.config.toml", &config_path).expect(
			"Failed to create default config file!"
		);
	} else {
		let file = fs::read_to_string(&config_path).expect("Failed to read config file!");
		config = file.parse::<Table>().expect("Failed to parse config.toml");
	}

	let (daypart_images, hour_images) = register_images(&path);

	return (config, daypart_images, hour_images);
}

// Search through config folder for images in either the Daypart subfolder or the Hour subfolder, and returns them as a vec
fn register_images(config_path: &String) -> ([Vec<String>; 4], [Vec<String>; 24]) {
	let mut daypart_images: [Vec<String>; 4] = Default::default();
	let mut hour_images: [Vec<String>; 24] = Default::default();

	for (i, daypart) in DAYPARTS.iter().enumerate() {
		let daypart_paths = fs
			::read_dir(format!("{}/dayparts/{}", config_path, *daypart))
			.expect("Failed to read dayparts folder!");
		for path in daypart_paths {
			daypart_images[i].push(
				path
					.unwrap()
					.path()
					.into_os_string()
					.into_string()
					.expect("File path is invalid Unicode!")
			);
		}
	}

	for hour in 0..24 {
		let hour_paths = fs
			::read_dir(format!("{}/hours/{:?}", config_path, hour))
			.expect("Failed to read hours folder!");
		for path in hour_paths {
			hour_images[hour].push(
				path
					.unwrap()
					.path()
					.into_os_string()
					.into_string()
					.expect("File path is not valid unicode!")
			);
		}
	}

	return (daypart_images, hour_images);
}

fn get_hour() -> usize {
	let now = Local::now();
	let hour = now.format("%H").to_string().parse::<i32>().unwrap() as usize;
	return hour;
}

fn update_loop(
	config: Table,
	daypart_images: [Vec<String>; 4],
	hour_images: [Vec<String>; 24]
) -> ! {
	let mut last_image: &String = &String::new();
	loop {
		let hour = get_hour();
		let mut chosen_image: &String;
		let mut total_images = 0;

		let morning_start = config.get("morning").unwrap().as_integer().unwrap_or(6) as usize;
		let day_start = config.get("day").unwrap().as_integer().unwrap_or(12) as usize;
		let evening_start = config.get("evening").unwrap().as_integer().unwrap_or(18) as usize;
		let night_start = config.get("night").unwrap().as_integer().unwrap_or(24) as usize;

		//Determine which daypart we're in
		let daypart_index: usize;
		if hour >= morning_start && hour < day_start {
			daypart_index = 0;
		} else if hour >= day_start && hour < evening_start {
			daypart_index = 1;
		} else if hour >= evening_start && hour < night_start {
			daypart_index = 2;
		} else if hour >= night_start || hour < morning_start {
			daypart_index = 3;
		} else {
			panic!(
				"Hour doesn't match rules set in config. Ensure that your dayparts are in order and don't overlap."
			);
		}

		let daypart = &daypart_images[daypart_index];
		let mut rng = rand::thread_rng();
		chosen_image = daypart.choose(&mut rng).unwrap();

		total_images += daypart.len();

		// Overwrite with more specific hour if it exists
		if hour_images[hour].len() > 0 {
			chosen_image = hour_images[hour].choose(&mut rand::thread_rng()).unwrap();
		}

		total_images += hour_images[hour].len();

		let command_str = config
			.get("wallpaper_command")
			.unwrap()
			.as_str()
			.expect("Wallpaper command is invalid.");
		let always_change = config
			.get("always_change")
			.expect("always_change config key is missing")
			.as_bool()
			.expect("always_change must be a boolean");
		let update_interval = config
			.get("update_interval")
			.expect("update_interval config key is missing")
			.as_integer()
			.expect("update_interval must be an integer") as u64;

		// Make sure the image is a new one if always_change is true, if not, loop again
		if !always_change || (always_change && chosen_image != last_image && total_images > 1) {
			if chosen_image.len() > 0 {
				let parsed_command = command_str.replace("{{image}}", chosen_image);
				println!("Running: {}", parsed_command);
				let mut command = shell(parsed_command);
				command.execute().err();

				last_image = chosen_image;
				let duration = time::Duration::from_secs(60 * update_interval);
				thread::sleep(duration);
			} else {
				// TODO: Add system for choosing another image to fall back to
				println!(
					"No image could be found for daypart {} and hour {}",
					DAYPARTS[daypart_index],
					hour
				);

				let duration = time::Duration::from_secs(60 * update_interval);
				thread::sleep(duration);
			}
		}

		// Sleep for an hour until next loop
	}
}

fn main() {
	let home_path = env::var("HOME").expect("User home directory not set!");
	let config_path = env::var("XDG_CONFIG_HOME").unwrap_or(home_path + "/.config/wallhelper");

	let (config, daypart_images, hour_images) = configure(&config_path);

	update_loop(config, daypart_images, hour_images);
}
