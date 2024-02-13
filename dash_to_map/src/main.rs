extern crate sdl2;
use clap::Parser;
use std::f64::consts::PI;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use sdl2::render::Canvas;
use sdl2::pixels::{Color,PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::image::LoadTexture;
use std::str::Split;
use curl::easy::Easy;
use std::fs::{DirEntry, File};
use sdl2::surface::Surface;
use std::io::Write;
use chrono::{NaiveDate, NaiveDateTime};
use substring::Substring;
use std::str;
use ffprobe;
use crossterm::{execute,cursor::Hide,cursor::Show};
use std::io;

const MAP_WIDTH: u32 = 256_u32;
const MAP_HEIGHT: u32 = 256_u32;
const TILE_W: i16 = 256;
const TILE_H: i16 = 256;
const BASE_URL: &str = "https://maps.geoapify.com/v1/";
const MAP_ZOOM: u16 = 15;
const PAD_TOKEN: &str = "PADDING";
const EMPTY_GPS_NUMBERS: [i32; 9] = [0,0,0,0,0,0,0,0,0];

/// 70mai Dash Cam Lite 2 map image sequence maker
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Dashcam normal front camera dir (for calculating map time offset)
    #[arg(short, long)]
    frontvideo: String,

    /// Output dir for the rendered image sequence
    #[arg(short, long)]
    outputdir: String,

    /// Directory to read and write cache tile map
    #[arg(short, long, default_value_t = String::from("~./cache/dashmap"))]
    mapcachedir: String,

    /// Output expected framerate
    #[arg(long)]
    fps: u32,

    /// Original time factor multiplier for timelapse (Must be greater than 0 and less or equal than 1) (defaults to 1, no timelapse)"
    #[arg(long)]
    originaltimefactor: f32,

    /// GPS data file
    #[arg(long)]
    gpsdatafile: String,

    // GeoAPIfy key for map images
    #[arg(long)]
    geoapifykey: String,

    // Difference between GPS timestamp and video timestamp in seconds
    #[arg(long, short, default_value_t = -17987_i64)] // -5 * 3600 + 13
    timedifference: i64
}

struct GPSData {
    timestamp: u32,
    latitude: f64,
    longitude: f64,
    letter: String,
    numbers: [i32;9]
}

struct VideoData{
    start_timestamp: u32,
    end_timestamp: u32,
    filename: String
}

fn main() {
    let _ = execute!(
        io::stdout(),
        Hide
    );
    let args = Args::parse();

    let front_video_path = args.frontvideo;
    let output_dir_path = args.outputdir;
    let map_cache_dir = args.mapcachedir;
    let fps = args.fps;
    let time_multiplication = 1_f32 / args.originaltimefactor;
    let gps_data_file_path = args.gpsdatafile;
    let geoapifykey = args.geoapifykey;

    let raw_gps_data = extract_gps_data(&gps_data_file_path);
    let video_timestamps = extract_timestamps(&front_video_path, args.timedifference);

    let mut canvas = init_canvas(MAP_WIDTH, MAP_HEIGHT);

    build_animation(
        &video_timestamps,
        &raw_gps_data,
        time_multiplication,
        fps,
        &geoapifykey,
        &mut canvas,
        &map_cache_dir,
        &output_dir_path
    );

    let _ = execute!(
        io::stdout(),
        Show
    );
}

fn extract_timestamps(front_video_path: &String, timezonedifference: i64) -> Vec<VideoData>{
    println!("Extracting timestamps and duration from front videos...");
    let mut raw_timestamp_posts: Vec<VideoData> = Vec::new();
    let path_dir = Path::new(front_video_path);
    
    if path_dir.is_dir(){
        let dir_iterator = fs::read_dir(path_dir).unwrap();
        let mut files: Vec<DirEntry> = Vec::new();

        for path in dir_iterator {
            files.push(path.unwrap());
        }

        files.sort_by(|a, b| a.path().cmp(&b.path()));

        for file in files {
            let file_path = file.path();

            match file_path.extension().and_then(OsStr::to_str) {
                None => println!("Could not read extenstion on file from front camera directory"),
                Some(ext) => {
                    let mut clean_extension = String::from(ext);
                    clean_extension.make_ascii_lowercase();
                    if clean_extension == "mp4" {
                        match file_path.to_str(){
                            None => println!("Could not read extenstion on file from front camera directory"),
                            Some(file_full_path) => {
                                let file_name = file_full_path.split("/").last().unwrap();

                                let file_name_parts: Vec<&str> = file_name
                                    .strip_prefix("NO").unwrap()
                                    .strip_suffix("F.MP4").unwrap()
                                    .split("-").collect();

                                let date = file_name_parts[0];
                                let time = file_name_parts[1];

                                let year = date.substring(0,4).parse::<i32>().unwrap();
                                let month = date.substring(4,6).parse::<u32>().unwrap();
                                let day = date.substring(6,8).parse::<u32>().unwrap();

                                let hour = time.substring(0,2).parse::<u32>().unwrap();
                                let minute = time.substring(2, 4).parse::<u32>().unwrap();
                                let second = time.substring(4, 6).parse::<u32>().unwrap();

                                let datetime: NaiveDateTime = NaiveDate
                                    ::from_ymd_opt(year, month, day)
                                    .unwrap()
                                    .and_hms_opt(hour, minute, second)
                                    .unwrap();

                                let start_timestamp = datetime.timestamp() + timezonedifference;
                                
                                println!("Reading video duration for {}", file_name);
                                let video_duration = ffprobe::ffprobe(file_full_path)
                                    .unwrap()
                                    .streams
                                    .first()
                                    .unwrap()
                                    .duration
                                    .as_ref()
                                    .unwrap()
                                    .parse::<f64>()
                                    .unwrap();

                                let end_timestamp = start_timestamp + video_duration.floor() as i64;

                                raw_timestamp_posts.push(VideoData{
                                    start_timestamp: start_timestamp as u32,
                                    end_timestamp: end_timestamp as u32,
                                    filename: file_name.to_string()
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    
    println!("[ OK ]");

    raw_timestamp_posts
}

fn extract_gps_data_for_video(video_start: u32, video_end: u32, gps_data: &Vec<GPSData>) -> Vec<GPSData>{
    let mut video_gps_data: Vec<GPSData> = Vec::new();
    for gps in gps_data {
        if gps.timestamp > video_end {
            let pad_time;

            if video_gps_data.is_empty() {
                pad_time = video_end - video_start;
            }else{
                pad_time = video_end - video_gps_data.last().unwrap().timestamp;
            }

            video_gps_data.push(GPSData{
                timestamp: pad_time,
                latitude: 0_f64,
                longitude: 0_f64,
                letter: PAD_TOKEN.to_string(),
                numbers: EMPTY_GPS_NUMBERS
            });

            break;
        }else if gps.timestamp > video_start {
            if video_gps_data.is_empty() {
                video_gps_data.push(GPSData{
                    timestamp: gps.timestamp - video_start,
                    latitude: 0_f64,
                    longitude: 0_f64,
                    letter: PAD_TOKEN.to_string(),
                    numbers: EMPTY_GPS_NUMBERS
                });
            }
            video_gps_data.push(GPSData{
                timestamp: gps.timestamp,
                latitude: gps.latitude,
                longitude: gps.longitude,
                letter: gps.letter.clone(),
                numbers: gps.numbers
            });
        }
    }

    return video_gps_data;
}

fn init_canvas<'a>(image_width: u32, image_height: u32)-> Canvas<Surface<'a>>{
    let surface = Surface::new(image_width, image_height, PixelFormatEnum::ARGB8888).unwrap();
    let canvas = Surface::into_canvas(surface).unwrap();
    return canvas;
}

fn find_gps_record(gps_data: &Vec<GPSData>, target_timestamp: u32)-> Option<&GPSData>{
    let mut found_record: Option<&GPSData>;
    found_record = Option::None;
    for gps in gps_data {

        if !gps.letter.eq(PAD_TOKEN){
            found_record = Option::Some(&gps);

            if gps.timestamp > target_timestamp {
                break;
            }
        }else {
            found_record = Option::None;
        }

    }

    return found_record;
}

fn build_animation(
    video_timestamps: &Vec<VideoData>,
    gps_data: &Vec<GPSData>,
    time_multiplication: f32,
    fps: u32,
    geoapifykey: &str,
    canvas: &mut Canvas<Surface>,
    map_cache_dir: &str,
    output_dir: &str
) {
    let mut frame_count = 0_usize;
    let multiplied_frame_time = (1_f64 / f64::from(fps)) * f64::from(time_multiplication);
    let mut video_index_plus_one = 1_usize;
    let video_count = video_timestamps.len();

    for video in video_timestamps{
        println!(
            "Building map image sequence for video {} of {} [{}] ({} - {})",
            video_index_plus_one,
            video_count,
            video.filename,
            video.start_timestamp,
            video.end_timestamp
        );
        video_index_plus_one+=1;
        let gps_data_for_video = extract_gps_data_for_video(
            video.start_timestamp,
            video.end_timestamp,
            gps_data
        );

        if !gps_data_for_video.is_empty(){
            let mut video_frame_count=0;

            match gps_data_for_video.first(){
                Some(gps_record) => {
                    let pad_frames = (gps_record.timestamp as f64 / multiplied_frame_time).floor() as i32;
                    println!("Building {} frames for start padding...", pad_frames);
                    video_frame_count=pad_frames;

                    frame_count = build_padding_animation(
                        pad_frames,
                        canvas,
                        frame_count,
                        output_dir
                    );
                }
                None => {}
            }

            println!("Building map frames...");
            loop {
                let frame_output_path = format!("{}/{:0>6}.png", output_dir, frame_count);
                let next_target_delta = video_frame_count as f64 * multiplied_frame_time;
                let next_target_timestamp = video.start_timestamp as f64 + next_target_delta;
                frame_count+=1;
                video_frame_count+=1;

                let next_gps_record = find_gps_record(&gps_data_for_video, next_target_timestamp as u32);

                match  next_gps_record{ 
                    Some(gps_record) => {
                        print!("\rFrame: {} \t Timestamp: {}", frame_count, next_target_timestamp);
                        build_map_frame(geoapifykey, gps_record, canvas, map_cache_dir);
                        write_canvas_to_file(canvas, &frame_output_path);

                        if gps_record.timestamp == gps_data_for_video.last().unwrap().timestamp {
                            break;
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
            println!("");

            if gps_data_for_video.len() > 1 {
                match gps_data_for_video.last(){
                    Some(gps_record) => {
                        if gps_record.letter == PAD_TOKEN {
                            let pad_frames = (gps_record.timestamp as f64 / multiplied_frame_time).floor() as i32;
                            println!("Building {} frames for end padding...", pad_frames);
        
                            frame_count = build_padding_animation(
                                pad_frames,
                                canvas,
                                frame_count,
                                output_dir
                            );
                        }
                    }
                    None => {}
                }
            }
        }
    }
    println!("[ OK ]");
}

fn build_padding_animation(
    pad_frames: i32,
    canvas: &mut Canvas<Surface>,
    mut frame_count: usize,
    output_dir: &str
)-> usize{
    for _ in 0..pad_frames {
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 0));
        canvas.clear();
        let frame_output_path = format!("{}/{:0>6}.png", output_dir, frame_count);
        write_canvas_to_file(canvas, &frame_output_path);
        frame_count+=1;
    }

    return frame_count;
}

fn write_canvas_to_file(
    canvas: &Canvas<Surface>,
    output_path: &str
){
    let (width, height) = canvas.surface().size();
    let canvas_pixels = canvas.read_pixels(None, PixelFormatEnum::ABGR8888).unwrap();

    let path = String::from(output_path);
    image::save_buffer(&path, &canvas_pixels, width, height, image::ColorType::Rgba8).unwrap();
}

fn build_map_frame(
    geoapifykey: &str,
    gps_line: &GPSData,
    canvas: &mut Canvas<Surface>,
    map_cache_dir: &str
){
    let tile = solve_tile(MAP_ZOOM, gps_line.latitude, gps_line.longitude);
    let tile_x = tile[0] as i16;
    let tile_y = tile[1] as i16;
    let px = tile[2] as i16;
    let py = tile[3] as i16;
    let half_w = TILE_W / 2;
    let half_h = TILE_H / 2;

    let tile_offset_x: i16;
    if px < half_w {
        tile_offset_x = -1;
    }else{
        tile_offset_x = 1;
    }

    let tile_offset_y: i16;
    if py < half_h {
        tile_offset_y = -1;
    }else{
        tile_offset_y = 1;
    }

    let adj_tile1_x = tile_x + tile_offset_x;
    let adj_tile1_y = tile_y;

    let adj_tile2_x = tile_x + tile_offset_x;
    let adj_tile2_y = tile_y + tile_offset_y;

    let adj_tile3_x = tile_x;
    let adj_tile3_y = tile_y + tile_offset_y;

    let tile_file = download_or_get(geoapifykey, map_cache_dir, MAP_ZOOM, tile_x, tile_y);
    let adj_tile_file1 = download_or_get(geoapifykey, map_cache_dir, MAP_ZOOM, adj_tile1_x, adj_tile1_y);
    let adj_tile_file2 = download_or_get(geoapifykey, map_cache_dir, MAP_ZOOM, adj_tile2_x, adj_tile2_y);
    let adj_tile_file3 = download_or_get(geoapifykey, map_cache_dir, MAP_ZOOM, adj_tile3_x, adj_tile3_y);

    let texture_creator = canvas.texture_creator();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    let x_offset = (half_w-px) as i32;
    let y_offset = (half_h-py) as i32;
    let x_offset1 = x_offset + (TILE_W * tile_offset_x) as i32;
    let y_offset1 = y_offset + (TILE_H * tile_offset_y) as i32;

    let center_tile_texture = texture_creator.load_texture(tile_file).unwrap();
    let adj_tile1_texture = texture_creator.load_texture(adj_tile_file1).unwrap();
    let adj_tile2_texture = texture_creator.load_texture(adj_tile_file2).unwrap();
    let adj_tile3_texture = texture_creator.load_texture(adj_tile_file3).unwrap();

    let _ = canvas.copy(&center_tile_texture, None, Rect::new(x_offset,y_offset,256,256));
    let _ = canvas.copy(&adj_tile1_texture, None, Rect::new(x_offset1,y_offset,256,256));
    let _ = canvas.copy(&adj_tile2_texture, None, Rect::new(x_offset1,y_offset1,256,256));
    let _ = canvas.copy(&adj_tile3_texture, None, Rect::new(x_offset,y_offset1,256,256));

    canvas.set_draw_color(Color::RGB(255, 0, 0));
    let _ = canvas.fill_rect(Rect::new((half_w-2) as i32, (half_h-2) as i32, 4,4));
}

fn extract_gps_data(gps_data_file_path: &str)->Vec<GPSData>{
    let content = fs::read_to_string(gps_data_file_path).expect("Could not read GPS data file");
    let lines: Split<&str> = content.split("\n");
    let mut gps_data: Vec<GPSData> = Vec::new();

    for line in lines {
        let parts = line.split(",").collect::<Vec<&str>>();

        if parts.len() > 1{
            let time = parts[0].parse::<u32>().unwrap();
            let letter = parts[1];

            let lat = parts[2].parse::<f64>().unwrap_or(0_f64);
            let lon = parts[3].parse::<f64>().unwrap_or(0_f64);
            
            let numbers = [
                parts[4].parse::<i32>().unwrap(),
                parts[5].parse::<i32>().unwrap(),
                parts[6].parse::<i32>().unwrap(),
                parts[7].parse::<i32>().unwrap(),
                parts[8].parse::<i32>().unwrap(),
                parts[9].parse::<i32>().unwrap(),
                parts[10].parse::<i32>().unwrap(),
                parts[11].parse::<i32>().unwrap(),
                parts[12].parse::<i32>().unwrap()
            ];

            let gps_line_data = GPSData{
                timestamp: time,
                latitude: lat,
                longitude: lon,
                letter: String::from(letter),
                numbers: numbers
            };

            gps_data.push(gps_line_data);
        }
    }

    gps_data
}

fn solve_tile(zoom: u16, lat: f64, lon: f64) -> [u16; 4] {
    let n = (2 as f64).powf(zoom as f64);
    let lat_rads = lat.to_radians();
    let lon_tile_point = n * ((lon + 180.0) / 360.0);
    let lat_tile_point = n * (1.0 - (lat_rads.tan() + 1.0 / lat_rads.cos()).ln() / PI) / 2.0;
    let lon_tile = lon_tile_point.floor();
    let lat_tile = lat_tile_point.floor();
    let xpos_factor = lon_tile_point - lon_tile;
    let ypos_factor = lat_tile_point - lat_tile;
    let xpos = (TILE_W as f64 * xpos_factor).floor();
    let ypos = (TILE_H as f64 * ypos_factor).floor();
    let mut tile = [0; 4];
    tile[0] = lon_tile as u16;
    tile[1] = lat_tile as u16;
    tile[2] = xpos as u16;
    tile[3] = ypos as u16;
    return tile;
}

fn download_or_get(geoapifykey: &str, cache_dir: &str, zoom: u16, tile_x: i16, tile_y: i16)-> String{
    let tile_file_path = format!("{}/{}/{}/{}.webp", cache_dir, zoom, tile_x, tile_y);
    let tile_path = Path::new(&tile_file_path);
    let tile_file_path_string = String::from(&tile_file_path);

    let file_exists = tile_path.exists();
    
    if !file_exists{
        let tile_url = format!("{}tile/osm-carto/{}/{}/{}.png?apiKey={}", BASE_URL, zoom, tile_x, tile_y, geoapifykey);
        let prefix = tile_path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();
        let mut curl = Easy::new();
        curl.url(&tile_url).unwrap();
        let mut tile_file = File::create(&tile_file_path_string).expect("Unable to create tile cache file");
        curl.write_function(move |data| {
            tile_file.write_all(data).unwrap();
            Ok(data.len())
        }).unwrap();
        curl.perform().unwrap();
    }

    return tile_file_path;
}
