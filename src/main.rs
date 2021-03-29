// http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
// http://multigesture.net/articles/how-to-write-an-emulator-chip-8-interpreter/
// https://chip-8.github.io/links/
use std::{env::args, fs::File, thread, time::Instant};
use std::{io::Read, time::Duration};

use rand::{rngs::ThreadRng, thread_rng, Rng};

use sfml::audio::{self, SoundSource};
use sfml::graphics::{
    Color, Drawable, RectangleShape, RenderTarget, RenderWindow, Shape, Transformable,
};
use sfml::system::Vector2f;
use sfml::window::{Event, Key, Style, VideoMode};

const PIXEL: usize = 10;
const D_WIDTH: usize = 64;
const D_HEIGHT: usize = 32;
const FONT_SPRITES: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

struct Catpeasant {
    memory: [u8; 4096], // guess what this is
    i: u16,             // index register/address
    v: [u8; 16],        // general purose registers
    delay_timer: u8,    // delay timer register
    sound_timer: u8,    // sound timer register
    pc: u16,            //program counter
    sp: usize,          // stack pointer
    stack: [u16; 16],
    keyboard: [bool; 16],
    display: [u8; 2048],
    draw: bool,
    random: ThreadRng,
    freaky: Instant,
    key_wait: bool,
    sound: bool,
}

impl Catpeasant {
    // emulates a cycle
    fn do_cycle(&mut self) {
        // get opcode
        let opcode = self.read_opcode();

        // decode && exec
        self.decode_exec_opcode(opcode);

        // update timers
        if self.freaky.elapsed() > Duration::from_nanos(1000000000 / 60) {
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }

            if self.sound_timer > 0 {
                self.sound_timer -= 1;
                if self.sound_timer == 0 {
                    self.sound = true;
                }
            }

            self.freaky = Instant::now();
        }
    }

    fn load_rom(&mut self, rome: std::path::PathBuf) {
        let mut rom = File::open(rome).unwrap();
        let _ = rom.read(&mut self.memory[512..]);
    }

    fn read_opcode(&self) -> u16 {
        (self.memory[self.pc as usize] as u16) << 8 | self.memory[(self.pc + 1) as usize] as u16
    }

    fn decode_exec_opcode(&mut self, opcode: u16) {
        let (_toggles_bits, n2, n3, n4) = (
            (opcode & 0xf000) >> 12,
            (opcode & 0x0f00) >> 8,
            (opcode & 0x00f0) >> 4,
            opcode & 0x000f,
        );

        let address = opcode & 0x0fff;
        let vx = self.v[n2 as usize];
        let vy = self.v[n3 as usize];
        let kk = (opcode & 0x00ff) as u8;

        // println!("{:04x}", opcode);
        match opcode {
            // Clear Screen
            0x00e0 => {
                self.display = [0; 2048];
            }
            // Return
            0x00ee => {
                self.pc = self.stack[self.sp];
                self.sp -= 1;
            }
            0x0000..=0x0FFF => {} // Calls things for COSMAC, not common in regular roms.
            // Jump
            0x1000..=0x1FFF => {
                self.pc = address;
                return;
            }
            // Call Subroutine
            0x2000..=0x2FFF => {
                self.sp += 1; // FIXME: No fix let panic
                self.stack[self.sp] = self.pc;
                self.pc = address;
                return;
            }
            // Skip Next if Vx == NN
            0x3000..=0x3FFF => {
                if vx == kk {
                    self.pc += 2;
                }
            }
            // Skip Next if Vx != NN
            0x4000..=0x4FFF => {
                if vx != kk {
                    self.pc += 2;
                }
            }
            // Skip Next if Vx == Vy
            0x5000..=0x5FFF if n4 == 0 => {
                if vx == vy {
                    self.pc += 2;
                }
            }
            // Sets Vx to NN
            0x6000..=0x6FFF => self.v[n2 as usize] = kk,
            // Adds NN to Vx
            0x7000..=0x7FFF => self.v[n2 as usize] = vx.wrapping_add(kk),
            // Assignment, BitOps, Math. More matching inside =)
            0x8000..=0x8FFF => {
                self.v[n2 as usize] = match n4 {
                    0 => vy,
                    1 => vx | vy,
                    2 => vx & vy,
                    3 => vx ^ vy,
                    4 => {
                        let (v, b) = vx.overflowing_add(vy);
                        self.v[15] = match b {
                            true => 1,
                            false => 0,
                        };
                        v
                    }
                    5 => {
                        self.v[15] = if vx > vy { 1 } else { 0 };
                        vx.wrapping_sub(vy)
                    }
                    6 => {
                        self.v[15] = vx & 0x1;
                        vx >> 1
                    }
                    7 => {
                        self.v[15] = if vy > vx { 1 } else { 0 };
                        vy.wrapping_sub(vx)
                    }
                    0xE => {
                        self.v[15] = vx & 0x80;
                        vx << 1
                    }
                    _ => 0,
                }
            }
            // Skip Next if Vx != Vy
            0x9000..=0x9FFF if n4 == 0 => {
                if vx != vy {
                    self.pc += 2;
                }
            }
            // Sets I to NNN
            0xA000..=0xAFFF => self.i = address,
            // Jumps to NNN + V0
            0xB000..=0xBFFF => {
                self.pc = address + self.v[0] as u16;
                return;
            }
            // Random
            0xC000..=0xCFFF => self.v[n2 as usize] = self.random.gen::<u8>() & kk,
            // Display
            0xD000..=0xDFFF => {
                self.v[15] = 0;

                let height = n4;
                let mut pixel;

                for y in 0..height {
                    pixel = self.memory[(self.i + y) as usize];
                    for x in 0..8 {
                        if (pixel & (0x80 >> x)) != 0 {
                            let mut index =
                                vx as usize + x as usize + ((vy as usize + y as usize) * 64);
                            while index >= 2048 {
                                index -= 2048;
                            }

                            if self.display[index] == 1 {
                                self.v[15] = 1;
                            }

                            self.display[index] ^= 1;
                        }
                    }
                }

                self.draw = true;
            }
            // Keyboard
            0xE000..=0xEFFF if kk == 0x9E || kk == 0xA1 => match kk {
                0x9e => {
                    if self.keyboard[self.v[n2 as usize] as usize] {
                        self.pc += 2;
                    }
                }
                0xa1 => {
                    if !self.keyboard[self.v[n2 as usize] as usize] {
                        self.pc += 2;
                    }
                }
                _ => {}
            },
            // timers, memory, bcd
            0xF000..=0xFFFF => match kk {
                0x07 => self.v[n2 as usize] = self.delay_timer,
                0x0A => {
                    if !self.key_wait {
                        self.key_wait = true;
                        self.keyboard = [false; 16];
                        return;
                    }

                    if self.key_wait {
                        let mut index = 42;
                        for (i, b) in self.keyboard.iter().enumerate() {
                            if *b {
                                index = i;
                                break;
                            }
                        }
                        if index != 42 {
                            self.key_wait = false;
                            self.v[n2 as usize] = index as u8;
                            self.pc += 2;
                        }
                    }

                    return;
                }
                0x15 => self.delay_timer = vx,
                0x18 => self.sound_timer = vx,
                0x1E => self.i += vx as u16,
                0x29 => self.i = (vx * 5) as u16,
                0x33 => {
                    self.memory[self.i as usize] = vx / 100;
                    self.memory[(self.i + 1) as usize] = (vx / 10) % 10;
                    self.memory[(self.i + 2) as usize] = vx % 10;
                }
                0x55 => self.memory[(self.i as usize)..=((self.i + n2) as usize)]
                    .copy_from_slice(&self.v[0..=(n2 as usize)]),
                0x65 => self.v[0..=(n2 as usize)]
                    .copy_from_slice(&self.memory[(self.i as usize)..=((self.i + n2) as usize)]),
                _ => {}
            },
            _ => {}
        }

        self.pc += 2;
    }
}

impl Drawable for Catpeasant {
    fn draw<'a: 'shader, 'texture, 'shader, 'shader_texture>(
        &'a self,
        target: &mut dyn RenderTarget,
        _states: sfml::graphics::RenderStates<'texture, 'shader, 'shader_texture>,
    ) {
        let mut pixel = RectangleShape::new();
        pixel.set_size(Vector2f::new(PIXEL as f32, PIXEL as f32));
        pixel.set_fill_color(Color::GREEN);

        for (y, scanline) in self.display.chunks_exact(D_WIDTH).enumerate() {
            for (x, p) in scanline.iter().enumerate() {
                if *p > 0 {
                    pixel.set_position(Vector2f::new((x * 10) as f32, (y * 10) as f32));
                    target.draw(&pixel);
                }
            }
        }
    }
}

fn clear(w: &mut RenderWindow) {
    w.clear(Color::rgb(8, 8, 8));
}

fn main() {
    let mode = VideoMode::new(
        (D_WIDTH * PIXEL) as u32,
        (D_HEIGHT * PIXEL) as u32,
        VideoMode::desktop_mode().bits_per_pixel,
    );
    let mut togglebit = RenderWindow::new(mode, "Chippy Boi", Style::NONE, &Default::default());
    togglebit.set_position(
        (
            ((1920 - mode.width) / 2) as i32,
            (((1080 - mode.height) / 2) + 1080) as i32,
        )
            .into(),
    );

    // initial clear
    clear(&mut togglebit);
    togglebit.display();

    let snd_buf = audio::SoundBuffer::from_file("sound.wav").unwrap();
    let mut chip_sound = audio::Sound::with_buffer(&snd_buf);
    chip_sound.set_volume(10.0);

    let mut chip8 = Catpeasant {
        memory: [0; 4096],
        i: 0,
        v: [0; 16],
        delay_timer: 0,
        sound_timer: 0,
        pc: 512,
        sp: 0,
        stack: [0; 16],
        keyboard: [false; 16],
        display: [0; 2048],
        draw: false,
        random: thread_rng(),
        freaky: Instant::now(),
        key_wait: false,
        sound: false,
    };

    chip8.memory[0..FONT_SPRITES.len()].copy_from_slice(&FONT_SPRITES);

    let args = args().skip(1).collect::<Vec<_>>();
    let rom_path = if !args.is_empty() {
        args[0].clone()
    } else {
        "roms/MERLIN".into()
    };

    chip8.load_rom(rom_path.into());

    while togglebit.is_open() {
        // emulate cycle
        chip8.do_cycle();

        // draw if ready
        if chip8.draw {
            chip8.draw = false;
            clear(&mut togglebit);
            togglebit.draw(&chip8);
            togglebit.display();
        }

        if chip8.sound {
            chip8.sound = false;
            chip_sound.play();
        }

        // key states
        chip8.keyboard[0x0] = Key::X.is_pressed();
        chip8.keyboard[0x1] = Key::Num1.is_pressed();
        chip8.keyboard[0x2] = Key::Num2.is_pressed();
        chip8.keyboard[0x3] = Key::Num3.is_pressed();
        chip8.keyboard[0x4] = Key::Q.is_pressed();
        chip8.keyboard[0x5] = Key::W.is_pressed();
        chip8.keyboard[0x6] = Key::E.is_pressed();
        chip8.keyboard[0x7] = Key::A.is_pressed();
        chip8.keyboard[0x8] = Key::S.is_pressed();
        chip8.keyboard[0x9] = Key::D.is_pressed();
        chip8.keyboard[0xa] = Key::Z.is_pressed();
        chip8.keyboard[0xb] = Key::C.is_pressed();
        chip8.keyboard[0xc] = Key::Num4.is_pressed();
        chip8.keyboard[0xd] = Key::R.is_pressed();
        chip8.keyboard[0xe] = Key::F.is_pressed();
        chip8.keyboard[0xf] = Key::V.is_pressed();

        // all the other events
        while let Some(event) = togglebit.poll_event() {
            match event {
                Event::Closed => togglebit.close(),
                Event::KeyPressed {
                    code: Key::Escape, ..
                } => togglebit.close(),
                _ => {}
            }
        }

        thread::sleep(Duration::from_millis(2));
    }
}
