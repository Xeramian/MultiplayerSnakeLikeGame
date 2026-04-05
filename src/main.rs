use std::collections::HashSet;

use macroquad::{prelude::*, rand::rand, ui::root_ui};

fn get_conf() -> Conf {
    Conf {
        window_title: "High FPS Chat".to_owned(),
        platform: miniquad::conf::Platform {
            blocking_event_loop: false,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn ip_to_6digit(ip: std::net::Ipv4Addr) -> String {
    let mut num = u32::from(ip);
    let charset = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".as_bytes();
    let mut code = String::new();

    // Generate 6 characters
    for _ in 0..6 {
        code.push(charset[(num % 62) as usize] as char);
        num /= 62;
    }
    // Reverse because we calculated from the "ones" place up
    code.chars().rev().collect()
}

fn code_to_ip(code: &str) -> Option<std::net::Ipv4Addr> {
    if code.len() != 6 { return None; }
    
    let charset = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut num: u64 = 0; // Use u64 to prevent overflow during calculation

    for c in code.chars() {
        let val = charset.find(c)? as u64;
        num = num * 62 + val;
    }
    
    // Convert the resulting number back to an IPv4 address
    Some(std::net::Ipv4Addr::from(num as u32))
}

enum GameScenes {
    MainMenu,
    Playing,
    Setup2Player,
    Playing2Player
}

#[macroquad::main(get_conf)]
async fn main() {
    let padding = 50f32;
    let mut snake = std::collections::vec_deque::VecDeque::from([(10.0, 10.0)]);
    let mut cur_dir = (0f32, 0f32);
    let mut next_dir = (1f32, 0f32);
    let mut time = std::time::Instant::now();
    let mut time_skip = 250;
    let grid_size = (12, 12);
    let gen_apple = move |snakes: &[&std::collections::VecDeque<(f32, f32)>], others: &[(f32, f32)]| {
        let mut options = HashSet::new();
        for i in 0..grid_size.0 {
            for j in 0..grid_size.1 {
                options.insert((i, j));
            }
        }
        for snake in snakes {
            for sn in *snake {
                options.remove(&(sn.0 as u8, sn.1 as u8));
            }
        }
        for other in others {
            options.remove(&(other.0 as u8, other.1 as u8));
        }
        let options_vec = options.iter().collect::<Vec<_>>();
        let selected = *options_vec[rand() as usize % options_vec.len()];
        return (selected.0 as f32, selected.1 as f32);
    };
    let mut apple = gen_apple(&[&snake], &[]);
    let mut scene = GameScenes::MainMenu;
    let font = load_ttf_font("assets/PressStart2P-Regular.ttf").await.unwrap();
    let mut code = String::from("3wK8a7");
    let mut code_text = String::new();
    let mut socket = None;
    let mut enemy_snake = std::collections::vec_deque::VecDeque::from([(10.0, 10.0)]);
    let mut buf = [0u8; 1024];
    let mut powerup_time = std::time::Instant::now();
    let mut powerup_type = 2;
    let mut powerup = (-1f32, -1f32);
    let mut speed_factor = 1f32;
    let mut enemy_dir = (1f32, 0f32);
    let mut cooldown_timer = 0;
    loop {
        clear_background(BLACK);
        match scene {
            GameScenes::MainMenu => {
                let text_dimensions = measure_text("Snake Game!", Some(&font), 48, 1f32);
                draw_text_ex("Snake Game!", (screen_width() - text_dimensions.width) / 2f32, (screen_height() - text_dimensions.height) / 2f32, TextParams { font: Some(&font), font_size: 48, ..Default::default() });
                root_ui().button(Some(Vec2 { x: screen_width() / 2f32 - 140f32, y: screen_height() / 2f32 + 40f32 }), "Start Game").then(|| {
                    snake = std::collections::vec_deque::VecDeque::from([(10.0, 10.0)]);
                    cur_dir = (0f32, 0f32);
                    next_dir = (1f32, 0f32);
                    time = std::time::Instant::now();
                    powerup_time = std::time::Instant::now();
                    time_skip = 250;
                    apple = gen_apple(&[&snake], &[]);
                    scene = GameScenes::Playing;
                });
                root_ui().button(Some(Vec2 { x: screen_width() / 2f32 + 60f32, y: screen_height() / 2f32 + 40f32 }), "Start 2P Game").then(|| {
                    scene = GameScenes::Setup2Player;
                });
            },
            GameScenes::Setup2Player => {
                if is_key_down(KeyCode::Escape) {
                    scene = GameScenes::MainMenu;
                }
                root_ui().button(Some(Vec2 { x: screen_width() / 2f32 - 140f32, y: screen_height() / 2f32 + 40f32 }), "Host Game").then(|| {
                    socket = None;
                    if let Ok(_socket) = std::net::UdpSocket::bind("0.0.0.0:8080") {
                        if let Ok(std::net::IpAddr::V4(my_ip)) = local_ip_address::local_ip() {
                            code_text = format!("Your Code: {}", ip_to_6digit(my_ip));
                        }
                        println!("Socket Started at {:?}", _socket);
                        _socket.set_nonblocking(true).ok();
                        socket = Some(_socket);
                    }
                });
                let text_dimensions = measure_text(&code_text, Some(&font), 24, 1f32);
                draw_text_ex(&code_text, (screen_width() - text_dimensions.width) / 2f32 , (screen_height() - text_dimensions.height) / 2f32, TextParams { font: Some(&font), font_size: 24, ..Default::default() });
                root_ui().input_text(0, "Code", &mut code);
                root_ui().button(Some(Vec2 { x: screen_width() / 2f32 + 60f32, y: screen_height() / 2f32 + 40f32 }), "Join Game").then(|| {
                    socket = None;
                    code_text = String::new();
                    code = code.trim().to_string();
                    if let Some(target_ip) = code_to_ip(code.trim()) {
                        let target_addr = format!("{}:8080", target_ip);
                        if let Ok(_socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
                            println!("Socket Connected to {:?}", _socket);
                            _socket.connect(target_addr).ok();
                            if let Ok(_) = _socket.send(b"Start!") {
                                if let Ok(cnt) = _socket.recv(&mut buf) {
                                    let msg = String::from_utf8_lossy(&buf[..cnt]);
                                    println!("{:?}", msg);
                                    if msg.trim() == "Ready!" {
                                        _socket.set_nonblocking(true).ok();
                                        socket = Some(_socket);
                                        cur_dir = (0f32, 0f32);
                                        next_dir = (1f32, 0f32);
                                        time = std::time::Instant::now();
                                        powerup_time = std::time::Instant::now();
                                        time_skip = 250;
                                        snake = std::collections::vec_deque::VecDeque::from([((grid_size.0 * 3 / 4) as f32, (grid_size.1 / 2) as f32)]);
                                        enemy_snake = std::collections::vec_deque::VecDeque::from([((grid_size.0 * 1 / 4) as f32, (grid_size.1 / 2) as f32)]);
                                        apple = ((grid_size.0 / 2) as f32, (grid_size.1 / 2) as f32);
                                        scene = GameScenes::Playing2Player;
                                    }
                                }
                            }
                        }
                    } else {
                        println!("Invalid 6-digit code!");
                    }
                });
                if let Some(_socket) = &mut socket {
                    match _socket.recv_from(&mut buf) {
                        Ok((amt, src)) => {
                            let msg = String::from_utf8_lossy(&buf[..amt]);
                            println!("{:?}", msg);
                            if msg.trim() == "Start!" {
                                _socket.connect(src).ok();
                                _socket.send(b"Ready!").ok();
                                cur_dir = (0f32, 0f32);
                                next_dir = (-1f32, 0f32);
                                time = std::time::Instant::now();
                                powerup_time = std::time::Instant::now();
                                time_skip = 250;
                                snake = std::collections::vec_deque::VecDeque::from([((grid_size.0 * 1 / 4) as f32, (grid_size.1 / 2) as f32)]);
                                enemy_snake = std::collections::vec_deque::VecDeque::from([((grid_size.0 * 3 / 4) as f32, (grid_size.1 / 2) as f32)]);
                                apple = ((grid_size.0 / 2) as f32, (grid_size.1 / 2) as f32);
                                scene = GameScenes::Playing2Player;
                            }
                        },
                        Err(_) => {

                        }
                    }
                }
            },
            GameScenes::Playing => {
                let interval_width = (screen_width() - 2f32 * padding) / grid_size.0 as f32;
                let interval_height = (screen_height() - 2f32 * padding) / grid_size.1 as f32;
                for i in 0..=grid_size.0 {
                    draw_line(padding + i as f32 * (interval_width), padding, padding + i as f32 * (interval_width), screen_height() - padding, 2f32, WHITE);
                }
                for j in 0..=grid_size.1 {
                    draw_line(padding, padding + j as f32 * (interval_height), screen_width() - padding, padding + j as f32 * (interval_height), 2f32, WHITE);
                }
                if powerup != (-1f32, -1f32) {
                    draw_rectangle(padding + powerup.0 * interval_width, padding + powerup.1 * interval_height, interval_width, interval_height, RED);
                }
                draw_rectangle(padding + apple.0 * interval_width, padding + apple.1 * interval_height, interval_width, interval_height, ORANGE);
                for s in snake.iter() {
                    draw_rectangle(padding + s.0 * interval_width - 2f32, padding + s.1 * interval_height - 2f32, interval_width + 4f32, interval_height + 4f32, BLACK);
                }
                for s in snake.iter() {
                    draw_rectangle(padding + s.0 * interval_width, padding + s.1 * interval_height, interval_width, interval_height, GREEN);
                }
                let keys_pressed = get_keys_pressed();
                if keys_pressed.contains(&KeyCode::A) {
                    if cur_dir != (1.0, 0.0) {
                        next_dir = (-1.0, 0.0);
                    }
                } else if keys_pressed.contains(&KeyCode::D) {
                    if cur_dir != (-1.0, 0.0) {
                        next_dir = (1.0, 0.0);
                    }
                } else if keys_pressed.contains(&KeyCode::W) {
                    if cur_dir != (0.0, 1.0) {
                        next_dir = (0.0, -1.0);
                    }
                } else if keys_pressed.contains(&KeyCode::S) {
                    if cur_dir != (0.0, -1.0) {
                        next_dir = (0.0, 1.0);
                    }
                }
                if is_key_down(KeyCode::Space) {
                    time_skip = (100f32 * speed_factor) as u128;
                } else {
                    time_skip = (250f32 * speed_factor) as u128;
                }
                if is_key_down(KeyCode::Escape) {
                    scene = GameScenes::MainMenu;
                }
                if powerup_time.elapsed().as_millis() > 7500 && speed_factor != 1f32 {
                    speed_factor = 1f32;
                }
                if powerup == (-1f32, -1f32) && powerup_time.elapsed().as_millis() > 12500 {
                    powerup = gen_apple(&[&snake], &[apple]);
                    powerup_type = 1;
                }
                if time.elapsed().as_millis() > time_skip {
                    let next_pos = (((snake.back().unwrap().0 as i8 + next_dir.0 as i8 + grid_size.0 as i8) % grid_size.0 as i8) as f32, ((snake.back().unwrap().1 as i8 + next_dir.1 as i8 + grid_size.1 as i8) % grid_size.1 as i8) as f32);
                    snake.push_back(next_pos);
                    if next_pos != apple {
                        snake.pop_front();
                    } else {
                        apple = gen_apple(&[&snake], &[powerup]);
                    }
                    if next_pos != powerup {

                    } else {
                        match powerup_type {
                            1 => {
                                speed_factor = 0.7f32;
                            }
                            _ => {}
                        }
                        powerup = (-1f32, -1f32);
                        powerup_time = std::time::Instant::now();
                    }
                    cur_dir = next_dir;
                    time = std::time::Instant::now();
                }
            },
            GameScenes::Playing2Player => {
                let interval_width = (screen_width() - 2f32 * padding) / grid_size.0 as f32;
                let interval_height = (screen_height() - 2f32 * padding) / grid_size.1 as f32;
                for i in 0..=grid_size.0 {
                    draw_line(padding + i as f32 * (interval_width), padding, padding + i as f32 * (interval_width), screen_height() - padding, 2f32, WHITE);
                }
                for j in 0..=grid_size.1 {
                    draw_line(padding, padding + j as f32 * (interval_height), screen_width() - padding, padding + j as f32 * (interval_height), 2f32, WHITE);
                }
                if powerup != (-1f32, -1f32) {
                    draw_rectangle(padding + powerup.0 * interval_width, padding + powerup.1 * interval_height, interval_width, interval_height, match powerup_type { 1 => Color { r: 1f32, g: 0.27f32, b: 0f32, a: 1f32 }, 2 => YELLOW, 3 => Color { r: 0f32, g: 1f32, b: 1f32, a: 1f32 }, _ => BLACK });
                }
                draw_rectangle(padding + apple.0 * interval_width, padding + apple.1 * interval_height, interval_width, interval_height, ORANGE);
                for s in enemy_snake.iter() {
                    draw_rectangle(padding + s.0 * interval_width - 2f32, padding + s.1 * interval_height - 2f32, interval_width + 4f32, interval_height + 4f32, BLACK);
                }
                for s in &enemy_snake {
                    draw_rectangle(padding + s.0 * interval_width, padding + s.1 * interval_height, interval_width, interval_height, BLUE);
                }
                for s in snake.iter() {
                    draw_rectangle(padding + s.0 * interval_width - 2f32, padding + s.1 * interval_height - 2f32, interval_width + 4f32, interval_height + 4f32, BLACK);
                }
                for s in &snake {
                    draw_rectangle(padding + s.0 * interval_width, padding + s.1 * interval_height, interval_width, interval_height, GREEN);
                }
                draw_text_ex(&format!("P Score: {}", snake.len()), 100f32, 40f32, TextParams { font: Some(&font), color: GREEN, font_size: 18, ..Default::default() });
                draw_text_ex(&format!("E Score: {}", enemy_snake.len()), screen_width() - measure_text(&format!("E Score: {}", enemy_snake.len()), Some(&font), 18, 1f32).width - 100f32, 40f32, TextParams { font: Some(&font), color: BLUE, font_size: 18, ..Default::default() });
                let keys_pressed = get_keys_pressed();
                if keys_pressed.contains(&KeyCode::A) {
                    if cur_dir != (1.0, 0.0) {
                        next_dir = (-1.0, 0.0);
                    }
                } else if keys_pressed.contains(&KeyCode::D) {
                    if cur_dir != (-1.0, 0.0) {
                        next_dir = (1.0, 0.0);
                    }
                } else if keys_pressed.contains(&KeyCode::W) {
                    if cur_dir != (0.0, 1.0) {
                        next_dir = (0.0, -1.0);
                    }
                } else if keys_pressed.contains(&KeyCode::S) {
                    if cur_dir != (0.0, -1.0) {
                        next_dir = (0.0, 1.0);
                    }
                }
                if is_key_down(KeyCode::Space) {
                    time_skip = (100f32 * speed_factor) as u128;
                } else {
                    time_skip = (250f32 * speed_factor) as u128;
                }
                if is_key_down(KeyCode::Escape) {
                    scene = GameScenes::MainMenu;
                }
                if powerup_time.elapsed().as_millis() > 7500 && speed_factor != 1f32 {
                    speed_factor = 1f32;
                }
                if powerup == (-1f32, -1f32) && powerup_time.elapsed().as_millis() > 12500 {
                    powerup = gen_apple(&[&snake], &[apple]);
                    powerup_type = [1, 1, 1, 1, 1, 1, 1, 3, 3, 3, 2][rand() as usize % 11];
                    if let Some(_socket) = &mut socket {
                        _socket.send(format!("{} {}\n{} {} {}\n{}", apple.0, apple.1, powerup.0, powerup.1, powerup_type, snake.iter().map(|x| format!("{} {}", x.0, x.1)).collect::<Vec<_>>().join("\n")).as_bytes()).ok();
                    }
                }
                if let Some(_socket) = &mut socket {
                    match _socket.recv(&mut buf) {
                        Ok(cnt) => {
                            let mut powerup_consumed = false;
                            let msg = String::from_utf8_lossy(&buf[..cnt]);
                            enemy_snake.clear();
                            let mut lines = msg.lines();
                            if let Some(apple_line) = lines.next() {
                                for (i, s) in apple_line.split(' ').into_iter().enumerate() {
                                    if i == 0 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {apple.0 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                    if i == 1 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {apple.1 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                }
                            }
                            if let Some(powerup_line) = lines.next() {
                                for (i, s) in powerup_line.split(' ').into_iter().enumerate() {
                                    if i == 0 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {powerup.0 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                    if i == 1 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {powerup.1 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                    if i == 2 {
                                        match s.parse::<u32>() {
                                            Ok(val) => {powerup_type = val;}
                                            Err(_) => {}
                                        }
                                    }
                                    if i == 3 {
                                        match s.parse::<u32>() {
                                            Ok(val) => {
                                                if val == 1 {
                                                    powerup_consumed = true;
                                                    powerup_time = std::time::Instant::now().checked_sub(std::time::Duration::from_millis(1000)).expect("err with times?");
                                                }
                                            }
                                            Err(_) => {}
                                        }
                                    }
                                }
                            }
                            if let Some(enemy_dir_line) = lines.next() {
                                for (i, s) in enemy_dir_line.split(' ').into_iter().enumerate() {
                                    if i == 0 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {enemy_dir.0 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                    if i == 1 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {enemy_dir.1 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                }
                            }
                            for line in lines {
                                let mut cur_enemy_pos = (0f32, 0f32);
                                for (i, s) in line.split(' ').into_iter().enumerate() {
                                    if i == 0 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {cur_enemy_pos.0 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                    if i == 1 {
                                        match s.parse::<f32>() {
                                            Ok(val) => {cur_enemy_pos.1 = val;}
                                            Err(_) => {}
                                        }
                                    }
                                }
                                enemy_snake.push_back(cur_enemy_pos);
                            }
                            if powerup_consumed && powerup_type == 2 {
                                std::mem::swap(&mut snake, &mut enemy_snake);
                                cur_dir = enemy_dir;
                                next_dir = enemy_dir;
                            }
                            if powerup_consumed && powerup_type == 3 {
                                cooldown_timer = 5000;
                            }
                        }
                        Err(_) => {

                        }
                    }
                }
                if time.elapsed().as_millis() > time_skip + cooldown_timer {
                    cooldown_timer = 0;
                    let mut powerup_consumed = false;
                    let next_pos = (((snake.back().unwrap().0 as i8 + next_dir.0 as i8 + grid_size.0 as i8) % grid_size.0 as i8) as f32, ((snake.back().unwrap().1 as i8 + next_dir.1 as i8 + grid_size.1 as i8) % grid_size.1 as i8) as f32);
                    snake.push_back(next_pos);
                    if next_pos != apple {
                        snake.pop_front();
                    } else {
                        apple = gen_apple(&[&snake, &enemy_snake], &[]);
                    }
                    if next_pos == powerup {
                        powerup = (-1f32, -1f32);
                        powerup_consumed = true;
                    }
                    if let Some(_socket) = &mut socket {
                        _socket.send(format!("{} {}\n{} {} {} {}\n{} {}\n{}", apple.0, apple.1, powerup.0, powerup.1, powerup_type, if powerup_consumed {1} else {0}, next_dir.0, next_dir.1, snake.iter().map(|x| format!("{} {}", x.0, x.1)).collect::<Vec<_>>().join("\n")).as_bytes()).ok();
                    }
                    cur_dir = next_dir;
                    if powerup_consumed {
                        match powerup_type {
                            1 => {
                                speed_factor = 0.7f32;
                            }
                            2 => {
                                std::mem::swap(&mut snake, &mut enemy_snake);
                                cur_dir = enemy_dir;
                                next_dir = enemy_dir;
                            }
                            _ => {}
                        }
                        powerup_time = std::time::Instant::now();
                    }
                    time = std::time::Instant::now();
                }
            }
        }
        next_frame().await
    }
}