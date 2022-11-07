extern crate sdl2;
use clap::Parser;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;
use std::ffi::OsStr;
use std::time;
use sdl2::video::{Window, WindowContext};
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::image::LoadTexture;
use sdl2::pixels::PixelFormatEnum;
use std::str::Split;
use curl::easy::Easy;
use std::fs::File;
use std::io::Write;

const ASPECT_RATIO: f32 = 16_f32/9_f32;
const WINDOW_SCALE: f32 = 0.5;
const MAP_MARGIN_RIGHT: f32 = 50_f32;
const MAP_MARGIN_BOTTOM: f32 = 50_f32;
const BACK_MARGIN_TOP: f32 = 50_f32;
const MAP_WIDTH: u32 = 256_u32;
const MAP_HEIGHT: u32 = 256_u32;
const TILE_W: i16 = 256;
const TILE_H: i16 = 256;
const BASE_URL: &str = "https://maps.geoapify.com/v1/";
const API_KEY: &str = "1e1542ea1d34493a881901530d5f4831";
const MAP_ZOOM: u16 = 15;
const GPS_DATA_TIMEZONE: i32 = -3;

/// 70mai Dash Cam Lite 2 timelapse and hud map video builder tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {

    /// Directory containing an image sequence from the front camera
    #[arg(long)]
    frontdir: String,

    /// Directory containing an image sequence from the back camera
    #[arg(long)]
    backdir: String,

    /// Frame offset for the front input image sequence 
    #[arg(long, default_value_t = 0)]
    frontoffset: u32,

    /// Frame offset for the back input image sequence 
    #[arg(long, default_value_t = 0)]
    backoffset: u32,

    /// Output width size in pixels
    #[arg(short, long, default_value_t = 1920)]
    width: u32,

    /// Output dir for the rendered image sequence
    #[arg(short, long)]
    outputdir: String,

    /// Directory to read and write cache tile map
    #[arg(short, long, default_value_t = String::from("./dash2MapCache"))]
    mapcachedir: String,

    /// Original video framerate
    #[arg(long)]
    originalfps: u32,

    /// Original time factor multiplier for timelapse (Must be greater than 0 and less or equal than 1) (defaults to 1, no timelapse)"
    #[arg(long)]
    originaltimefactor: f32,

    /// GPS data file
    #[arg(long)]
    gpsdatafile: String
}

struct ImageSizeData {
    original_width: u32,
    original_height: u32,
    original_cropped_height: u32,
    new_width: u32,
    new_height: u32,
    cropped_height: u32
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

    let front_dir_path = args.frontdir;
    let back_dir_path = args.backdir;
    let front_offset = args.frontoffset;
    let back_offset = args.backoffset;
    let output_dir_path = args.outputdir;
    let output_width = args.width;
    let map_cache_dir = args.mapcachedir;
    let original_video_framerate = args.originalfps;
    let original_video_time_factor = args.originaltimefactor;
    let gps_data_file_path = args.gpsdatafile;

    let front_images = read_dir(&front_dir_path);
    let back_images = read_dir(&back_dir_path);
    let first_front_image = &front_images[0];
    let front_image_size = calculate_size(first_front_image, output_width);
    let back_image_size = calculate_size(first_front_image, output_width);

    let start_timestamp = extract_timestamp(&front_dir_path);
    let gps_data = extract_gps_data(&gps_data_file_path);

    // for d in &gps_data {
    //     println!(
    //         "Time: {}\nPos: {},{}\nLetter: {}\nNumbers: {:?}\n",
    //         d.timestamp,
    //         d.latitude,
    //         d.longitude,
    //         d.letter,
    //         d.numbers
    //     );
    // }

    let mut canvas = init_ui(front_image_size.new_width, front_image_size.cropped_height);

    build_animation(
        &mut canvas,
        &front_images,
        &back_images,
        &front_image_size,
        &back_image_size,
        start_timestamp,
        original_video_framerate,
        original_video_time_factor,
        &gps_data,
        &map_cache_dir
    );
}

fn init_ui(image_width: u32, image_height: u32)-> Canvas<Window>{
    // let frame_time = time::Duration::from_millis(10);
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();
    let window = video_subsystem
        .window(
            "Dash2Map",
            (image_width as f32 * WINDOW_SCALE) as u32,
            (image_height as f32 * WINDOW_SCALE) as u32
        )
        .build()
        .unwrap();

    let mut canvas : Canvas<Window> = window
        .into_canvas()
        .present_vsync()
        .build()
        .unwrap();

    let _scale_result = canvas.set_scale(WINDOW_SCALE, WINDOW_SCALE);
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    canvas
}

fn calculate_size(input_image: &str, output_width: u32)->ImageSizeData{
    let first_front_image_size = get_image_size(input_image);
    let original_image_aspect_ratio = first_front_image_size.0 as f64 / first_front_image_size.1 as f64;
    let new_width = output_width;
    let new_height = (new_width as f64 / original_image_aspect_ratio) as u32;
    let cropped_height = (new_width as f32 / ASPECT_RATIO) as u32;
    let original_cropped_height = (first_front_image_size.0 as f32 / ASPECT_RATIO) as u32;

    ImageSizeData{
        original_width: first_front_image_size.0,
        original_height: first_front_image_size.1,
        original_cropped_height: original_cropped_height,
        new_width: new_width,
        new_height: new_height,
        cropped_height: cropped_height
    }
}

fn build_animation(
    canvas: &mut Canvas<Window>,
    front_images: &Vec<String>,
    back_images: &Vec<String>,
    front_image_size: &ImageSizeData,
    back_image_size: &ImageSizeData,
    start_timestamp: u32,
    original_video_framerate: u32,
    original_video_time_factor: f32,
    gps_data: &Vec<GPSData>,
    map_cache_dir: &str
){

    let front_src = Rect::new(
        0,
        0,
        front_image_size.original_width,
        front_image_size.original_cropped_height
    );

    let front_trg = Rect::new(
        0,
        0,
        front_image_size.new_width,
        front_image_size.new_height
    );

    let back_src = Rect::new(
        0,
        0,
        back_image_size.original_width,
        back_image_size.original_cropped_height
    );

    let back_trg = Rect::new(
        0,
        0,
        back_image_size.new_width,
        back_image_size.new_height
    );

    let map_trg = Rect::new(
        front_image_size.new_width as i32 - MAP_MARGIN_RIGHT as i32 - MAP_WIDTH as i32,
        front_image_size.cropped_height as i32 - MAP_MARGIN_BOTTOM as i32 - MAP_HEIGHT as i32,
        MAP_WIDTH,
        MAP_HEIGHT
    );

    let fps = original_video_framerate as f32 * original_video_time_factor;

    for idx in 0..front_images.len() {
        render_img(
            canvas,
            &front_images[idx],
            front_src,
            front_trg
        );

        let delta_timestamp = idx as f32 / fps;
        
        render_map_frame(
            start_timestamp + delta_timestamp as u32,
            gps_data,
            canvas,
            map_trg,
            map_cache_dir
        );

        if back_images.len() > idx {
            render_img(
                canvas,
                &back_images[idx],
                back_src,
                back_trg
            );
        }

        canvas.present();
    }
}

fn render_img(
    canvas: &mut Canvas<Window>,
    image_path: &str,
    src: Rect,
    trg: Rect
){
    let texture_creator = canvas.texture_creator();
    let texture = load_image_into_texture(&texture_creator, image_path);
    let _canvas_render_result = canvas.copy(
        &texture,
        src,
        trg
    );
}

fn solve_gps_line(timestamp: u32, gps_data: &Vec<GPSData>)-> Option<&GPSData>{
    for gps_line in gps_data {
        let utc_gps = (gps_line.timestamp as i32 + 8 * 3600) as u32;
        if timestamp <= utc_gps {
            println!("Timestamp: {} -- GPSUTC: {}",timestamp, utc_gps);
            return Option::Some(gps_line);
        }
    }

    Option::None
}

fn render_map_frame(
    timestamp: u32,
    gps_data: &Vec<GPSData>,
    canvas: &mut Canvas<Window>,
    map_area: Rect,
    map_cache_dir: &str
){
    match solve_gps_line(timestamp, gps_data){
        Some(gps_line) => {
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
        
            let tile_file = download_or_get(map_cache_dir, MAP_ZOOM, tile_x, tile_y);
            let adj_tile_file1 = download_or_get(map_cache_dir, MAP_ZOOM, adj_tile1_x, adj_tile1_y);
            let adj_tile_file2 = download_or_get(map_cache_dir, MAP_ZOOM, adj_tile2_x, adj_tile2_y);
            let adj_tile_file3 = download_or_get(map_cache_dir, MAP_ZOOM, adj_tile3_x, adj_tile3_y);
        
            let texture_creator = canvas.texture_creator();
            
            let x_offset = (half_w-px) as i32;
            let y_offset = (half_h-py) as i32;
            let x_offset_adj = x_offset + (TILE_W * tile_offset_x) as i32;
            let y_offset_adj = y_offset + (TILE_H * tile_offset_y) as i32;

            let center_tile_texture = texture_creator.load_texture(tile_file).unwrap();
            let adj_tile1_texture = texture_creator.load_texture(adj_tile_file1).unwrap();
            let adj_tile2_texture = texture_creator.load_texture(adj_tile_file2).unwrap();
            let adj_tile3_texture = texture_creator.load_texture(adj_tile_file3).unwrap();

            let center_tile_src_x = if x_offset < 0 { -x_offset } else { 0 };
            let center_tile_src_y = if y_offset < 0 { -y_offset } else { 0 };
            let center_tile_uncapped_l = map_area.left() + x_offset;
            let center_tile_uncapped_t = map_area.top() + y_offset;
            let center_tile_uncapped_r = center_tile_uncapped_l + TILE_W as i32;
            let center_tile_uncapped_b = center_tile_uncapped_t + TILE_H as i32;
            let center_tile_w = map_area.width()  - (if center_tile_uncapped_r > map_area.right() { center_tile_uncapped_r - map_area.right()} else { map_area.left() - center_tile_uncapped_l}) as u32;
            let center_tile_h = map_area.height() - (if center_tile_uncapped_b > map_area.bottom() { center_tile_uncapped_b - map_area.bottom()} else { map_area.top() - center_tile_uncapped_t}) as u32;

            let center_tile_src_rect = Rect::new(center_tile_src_x, center_tile_src_y, center_tile_w, center_tile_h);

            let center_tile_trg_rect_x = if x_offset < 0 { map_area.left() } else { map_area.left() + x_offset };
            let center_tile_trg_rect_y = if y_offset < 0 { map_area.top() } else { map_area.top() + y_offset };

            let center_tile_trg_rect = Rect::new(
                center_tile_trg_rect_x,
                center_tile_trg_rect_y,
                center_tile_w,
                center_tile_h
            );

            let adj_tile1_src_rect = Rect::new(0,0,0,0);
            let adj_tile1_trg_rect = Rect::new(0,0,0,0);
            let adj_tile2_src_rect = Rect::new(0,0,0,0);
            let adj_tile2_trg_rect = Rect::new(0,0,0,0);
            let adj_tile3_src_rect = Rect::new(0,0,0,0);
            let adj_tile3_trg_rect = Rect::new(0,0,0,0);
            let indicator_rect = Rect::new(
                map_area.left() + MAP_WIDTH as i32 / 2 - 5,
                map_area.top() + MAP_HEIGHT as i32 / 2 - 5,
                10,
                10
            );

            let _ = canvas.copy(&center_tile_texture, center_tile_src_rect, center_tile_trg_rect);
            // let _ = canvas.copy(&adj_tile1_texture, adj_tile1_src_rect, adj_tile1_trg_rect);
            // let _ = canvas.copy(&adj_tile2_texture, adj_tile2_src_rect, adj_tile2_trg_rect);
            // let _ = canvas.copy(&adj_tile3_texture, adj_tile3_src_rect, adj_tile3_trg_rect);
            canvas.set_draw_color(Color::RGB(255, 0, 0));
            let _ = canvas.fill_rect(indicator_rect);




            // let _canvas_render_result1 = 
        
        
            // let _canvas_render_result2 = canvas.copy(&texture1, None, Rect::new(x_offset1,y_offset,256,256));
        
            // let _canvas_render_result3 = canvas.copy(&texture2, None, Rect::new(x_offset1,y_offset1,256,256));
        
            // let _canvas_render_result4 = canvas.copy(&texture3, None, Rect::new(x_offset,y_offset1,256,256));
        
            // canvas.set_draw_color(Color::RGB(255, 0, 0));
            // let _canvas_render_result5 = canvas.fill_rect(Rect::new((half_w-2) as i32, (half_h-2) as i32, 4,4));
        }
        None => {}
    }
}

fn load_image_into_texture<'a>(texture_creator: &'a TextureCreator<WindowContext>, img_path: &str)-> Texture<'a>{
    let texture = texture_creator.load_texture(img_path).unwrap();
    texture
}

fn get_image_size(img_path: &str)->(u32,u32){
    let decoder = png::Decoder::new(fs::File::open(img_path).unwrap());
    let info_reader = decoder.read_info().unwrap();
    let info = info_reader.info();
    let width = info.width;
    let height = info.height;
    (width, height)
}

fn extract_timestamp(dir: &str)->u32{
    let meta_data_file = format!("{}/metadata.txt", dir);
    let meta_data_file_path = Path::new(&meta_data_file);
    let meta_data_file_content = fs::read_to_string(meta_data_file_path).expect("Could not read metadata.txt file");

    meta_data_file_content
        .trim()
        .parse()
        .expect("Could not parse data from metadata.txt file")
}

//1666068470,A,-38.115060,-57.597886,0,0,2,84,47,0,0,0,0
fn extract_gps_data(gps_data_file_path: &str)->Vec<GPSData>{
    let content = fs::read_to_string(gps_data_file_path).expect("Could not read GPS data file");
    let lines: Split<&str> = content.split("\n");
    let mut gps_data = Vec::new();

    for line in lines {
        let parts = line.split(",").collect::<Vec<&str>>();

        if parts.len() > 1{
            let time = parts[0].parse::<u32>().unwrap();
            let letter = parts[1];
            let lat = parts[2].parse::<f64>().unwrap();
            let lon = parts[3].parse::<f64>().unwrap();
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

fn read_dir(dir: &str)->Vec<String>{
    let mut images: Vec<String> = Vec::new();
    let path_dir = Path::new(dir);

    if path_dir.is_dir(){
        for file in fs::read_dir(path_dir).expect("Could not open") {
            let file = file.expect("unable to get entry");
            let file_path = file.path();

            match file_path.extension().and_then(OsStr::to_str) {
                None => println!("Could not read extenstion"),
                Some(ext) => {
                    let mut clean_extension = String::from(ext);
                    clean_extension.make_ascii_lowercase();
                    if clean_extension == "png" {
                        match file_path.to_str(){
                            None => println!("Could not read extension"),
                            Some(file_name) => {
                                images.push(String::from(file_name));
                            }
                        }
                    }
                }
            }
        }
    }

    images.sort();
    images
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

fn download_or_get(cache_dir: &str, zoom: u16, tile_x: i16, tile_y: i16)-> String{
    let tile_file_path = format!("{}/{}/{}/{}.png", cache_dir, zoom, tile_x, tile_y);
    let tile_path = Path::new(&tile_file_path);
    let tile_file_path_string = String::from(&tile_file_path);

    let file_exists = tile_path.exists();
    
    if !file_exists{
        let tile_url = format!("{}tile/osm-carto/{}/{}/{}.png?apiKey={}", BASE_URL, zoom, tile_x, tile_y, API_KEY);
        let prefix = tile_path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();

        println!("Downloading tile file {}", tile_url);
        
        let mut curl = Easy::new();
        curl.url(&tile_url).unwrap();
        curl.write_function(move |data| {
            let mut tile_file = File::create(&tile_file_path_string).expect("Unable to create tile cache file");
            tile_file.write_all(data).unwrap();
            Ok(data.len())
        }).unwrap();
        curl.perform().unwrap();
    }

    return tile_file_path;
}
