extern crate sdl2;
use clap::Parser;
use sdl2::video::Window;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;
use sdl2::render::Canvas;
use sdl2::pixels::{Color,PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::image::LoadTexture;
use std::str::Split;
use curl::easy::Easy;
use std::fs::File;
use sdl2::surface::Surface;
use std::io::Write;
use std::time::Duration;
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};

const MAP_WIDTH: u32 = 256_u32;
const MAP_HEIGHT: u32 = 256_u32;
const TILE_W: i16 = 256;
const TILE_H: i16 = 256;
const BASE_URL: &str = "https://maps.geoapify.com/v1/";
const MAP_ZOOM: u16 = 15;

static ALL_FRAMES_DUMPED: AtomicBool = AtomicBool::new(false);

/// 70mai Dash Cam Lite 2 map image sequence maker
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output dir for the rendered image sequence
    #[arg(short, long)]
    outputdir: String,

    /// Directory to read and write cache tile map
    #[arg(short, long, default_value_t = String::from("./dash2MapCache"))]
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

    #[arg(long)]
    geoapifykey: String
}

struct GPSData {
    timestamp: u32,
    latitude: f64,
    longitude: f64,
    letter: String,
    numbers: [i32;9]
}

fn main() {
    let args = Args::parse();

    let output_dir_path = args.outputdir;
    let map_cache_dir = args.mapcachedir;
    let fps = args.fps;
    let time_multiplication = 1_f32 / args.originaltimefactor;
    let gps_data_file_path = args.gpsdatafile;
    let geoapifykey = args.geoapifykey;

    let gps_trips = extract_gps_data(&gps_data_file_path);

    let mut canvas = init_canvas(MAP_WIDTH, MAP_HEIGHT);

    let mut window_canvas = init_ui(
        MAP_WIDTH,
        MAP_HEIGHT
    );

    let last_trip_index = gps_trips.len()-1;
    for (index, gps_data) in gps_trips.into_iter().enumerate() {
        let trip_outpur_dir = format!("{}/{}", output_dir_path, gps_data.first().unwrap().timestamp);
        std::fs::create_dir_all(&trip_outpur_dir).unwrap();
        build_animation(
            &mut canvas,
            time_multiplication,
            &gps_data,
            fps,
            &geoapifykey,
            &map_cache_dir,
            &trip_outpur_dir,
            index == last_trip_index,
            &mut window_canvas
        );    
    }

    println!("Waiting for remaining frames to be dumped into files");
    while ALL_FRAMES_DUMPED.load(Ordering::SeqCst) == false{
        thread::sleep(Duration::from_millis(1)); 
    }
}

fn init_canvas<'a>(image_width: u32, image_height: u32)-> Canvas<Surface<'a>>{
    let surface = Surface::new(image_width, image_height, PixelFormatEnum::ARGB8888).unwrap();
    let canvas = Surface::into_canvas(surface).unwrap();
    return canvas;
}

fn init_ui(width: u32, height: u32)-> Canvas<Window>{
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();
    let window = video_subsystem
        .window("Dash2Map", width, height)
        .build()
        .unwrap();

    let mut canvas : Canvas<Window> = window
        .into_canvas()
        .present_vsync()
        .build()
        .unwrap();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    return canvas;
}

fn find_gps_record(gps_data: &Vec<GPSData>, target_timestamp: u32)-> Option<&GPSData>{
    let mut i = 0;

    if !gps_data.is_empty(){
        loop{
            let gps = gps_data.get(i).unwrap();

            if gps.timestamp > target_timestamp{
                let found_record: &GPSData;
                if i > 0 {
                    found_record = gps_data.get(i-1).unwrap();
                }else{
                    found_record = gps_data.get(0).unwrap();
                }
                return Option::Some(found_record);
            }

            i+=1;

            if i == gps_data.len() {
                break;
            }
        }
    }

    return Option::None;
}

fn build_animation(
    canvas: &mut Canvas<Surface>,
    time_multiplication: f32,
    gps_data: &Vec<GPSData>,
    fps: u32,
    geoapifykey: &str,
    map_cache_dir: &str,
    output_dir: &str,
    is_last_trip: bool,
    window_canvas: &mut Canvas<Window>,
){
    let window_canvas_trg = Rect::new(
        0,
        0,
        MAP_WIDTH,
        MAP_HEIGHT
    );

    if !gps_data.is_empty() {
        let mut frame_count = 0_u32;
        let star_timestamp = gps_data.get(0).unwrap().timestamp;
        let multiplied_frame_time = (1_f64 / f64::from(fps)) * f64::from(time_multiplication);

        loop{
            println!("Frame {}", frame_count);
            let frame_output_path = format!("{}/{:0>6}.png", output_dir, frame_count);
            let next_target_delta = f64::from(frame_count) * multiplied_frame_time;
            let next_target_timestamp = (f64::from(star_timestamp) + next_target_delta).floor();
            frame_count+=1;

            println!("Start Timestamp: {}", star_timestamp);
            println!("Next target delta {}", next_target_delta);
            println!("Next target timestamp {}", next_target_timestamp);

            let next_gps_record = find_gps_record(gps_data, next_target_timestamp as u32);


            match  next_gps_record{ 
                Some(gps_record) => {
                    println!("Next GPS record timestamp {}", gps_record.timestamp);
                    build_map_frame(geoapifykey, gps_record, canvas, map_cache_dir);

                    write_canvas_to_file(
                        canvas,
                        &frame_output_path,
                        gps_data.last().unwrap().timestamp == gps_record.timestamp && is_last_trip
                    );

                    present_canvas_to_window(
                        window_canvas,
                        canvas,
                        window_canvas_trg
                    );
                }
                None => {
                    break;
                }
            }
        }
    }
}

fn write_canvas_to_file(
    canvas: &Canvas<Surface>,
    output_path: &str,
    is_last_frame: bool
){
    let (width, height) = canvas.surface().size();
    let canvas_pixels = canvas.read_pixels(None, PixelFormatEnum::ABGR8888).unwrap();

    let path = String::from(output_path);
    thread::spawn(move || {
        image::save_buffer(&path, &canvas_pixels, width, height, image::ColorType::Rgba8).unwrap();
        // println!("Written frame {}", path);
        if is_last_frame{
            ALL_FRAMES_DUMPED.store(true, Ordering::SeqCst);
        }
    });
}

fn present_canvas_to_window(
    canvas_trg: &mut Canvas<Window>,
    canvas_src: &mut Canvas<Surface>,
    trg: Rect
){

    let texture_creator = canvas_trg.texture_creator();
    let src_texture = texture_creator.create_texture_from_surface(canvas_src.surface()).unwrap();
    let _ = canvas_trg.copy(&src_texture, None, trg);
    canvas_trg.present();
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
    // canvas.present();
}

fn extract_gps_data(gps_data_file_path: &str)->Vec<Vec<GPSData>>{
    let content = fs::read_to_string(gps_data_file_path).expect("Could not read GPS data file");
    let lines: Split<&str> = content.split("\n");
    let mut gps_trips: Vec<Vec<GPSData>> = Vec::new();
    let mut first_line = true;
    let mut gps_trip_index = 0;
    gps_trips.push(Vec::new());

    for line in lines {
        let parts = line.split(",").collect::<Vec<&str>>();

        if parts.len() == 1 {
            if !first_line {
                gps_trip_index+=1;
                gps_trips.push(Vec::new());
            }
        } else if parts.len() > 1{
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

            gps_trips[gps_trip_index].push(gps_line_data);
        }

        first_line = false
    }

    gps_trips
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

        println!("Downloading tile file {}", tile_url);
        
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
