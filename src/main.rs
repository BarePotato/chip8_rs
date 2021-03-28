// http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
// http://multigesture.net/articles/how-to-write-an-emulator-chip-8-interpreter/
// https://chip-8.github.io/links/
#![allow(dead_code)]
#![allow(unused_variables)]
use sfml::graphics::{Color, RenderTarget, RenderWindow};
use sfml::window::{Style, VideoMode};

use rand::{rngs::ThreadRng, thread_rng, Rng};

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
    clear: bool,
    random: ThreadRng,
    wait_key: bool,
}

impl Catpeasant {
    // emulates a cycle
    fn do_cycle(&mut self) {
        // get opcode
        let opcode = self.read_opcode();

        // decode && exec
        self.decode_exec_opcode(opcode);

        // update timers
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }

        if self.sound_timer > 0 {
            self.sound_timer -= 1;
            if self.sound_timer == 0 {
                // This should play a sound now; or flag one to be played?
            }
        }
    }

    fn read_opcode(&self) -> u16 {
        (self.memory[self.pc as usize] as u16) << 8 | self.memory[(self.pc + 1) as usize] as u16
    }

    fn decode_exec_opcode(&mut self, opcode: u16) {
        // TODO: for funsies, explore union usage instead
        let (n1, n2, n3, n4) = (
            (opcode & 0xf000) >> 12,
            (opcode & 0x0f00) >> 8,
            (opcode & 0x00f0) >> 4,
            opcode & 0x000f,
        );

        let address = opcode & 0x0fff;
        let vx = self.v[n2 as usize];
        let vy = self.v[n3 as usize];
        let kk = (opcode & 0x00ff) as u8;

        match opcode {
            // Clear Screen
            0x00e0 => self.clear = true,
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
                self.stack[self.sp] = self.pc;
                self.sp += 1; // FIXME: make sure we aren't bigger than the stack
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
            0x7000..=0x7FFF => self.v[n2 as usize] += kk,
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
                //TODO: Do the SFML bits so we can figure this out
            }
            // Keyboard
            0xE000..=0xEFFF if kk == 0x9E || kk == 0xA1 => match kk {
                0x9e => {
                    if self.keyboard[n2 as usize] {
                        self.pc += 2;
                    }
                }
                0xa1 => {
                    if !self.keyboard[n2 as usize] {
                        self.pc += 2;
                    }
                }
                _ => {}
            },
            // timers, memory, bcd
            0xF000..=0xFFFF => match kk {
                0x07 => self.v[n2 as usize] = self.delay_timer,
                0x0A => {
                    // FIXME: This is bool crap code just to show it
                    // once SFML is connected, this needs to be a blocking key_read
                    // or a simulation of that.
                    let key_press = 'C';
                    self.v[n2 as usize] = key_press as u8;
                }
                0x15 => self.delay_timer = vx,
                0x18 => self.sound_timer = vx,
                0x1E => self.i += vx as u16,
                0x29 => self.i = vx as u16,
                0x33 => {
                    self.memory[self.i as usize] = vx / 100;
                    self.memory[(self.i + 1) as usize] = (vx / 10) % 10;
                    self.memory[(self.i + 2) as usize] = vx % 10;
                }
                // TODO: If something blows up drop the inclusive or go +1 in the right places
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

fn main() {
    let mode = VideoMode::new(640, 320, VideoMode::desktop_mode().bits_per_pixel);
    let mut togglebit = RenderWindow::new(mode, "Chippy Boi", Style::NONE, &Default::default());

    let mut chip8 = Catpeasant {
        memory: [0; 4096],
        i: 0,
        v: [0; 16],
        delay_timer: 0,
        sound_timer: 0,
        pc: 0,
        sp: 0,
        stack: [0; 16],
        keyboard: [false; 16],
        display: [0; 2048],
        draw: false,
        clear: true,
        random: thread_rng(),
        wait_key: false,
    };

    while togglebit.is_open() {
        // emulate cycle
        if !chip8.wait_key {
            chip8.do_cycle();

            // draw if ready
            if chip8.clear && !chip8.draw {
                // this is stupid
                togglebit.clear(Color::rgb(8, 8, 8));
            }
            if chip8.draw {
                togglebit.clear(Color::rgb(8, 8, 8));
                togglebit.display();
            }
        }

        // key states
        while let Some(event) = togglebit.poll_event() {
            use sfml::window::Key;
            match event {
                sfml::window::Event::Closed => togglebit.close(),
                sfml::window::Event::KeyPressed {
                    code: Key::Escape, ..
                } => togglebit.close(),

                sfml::window::Event::KeyPressed { code, .. } => {}
                sfml::window::Event::KeyReleased { code, .. } => {}
                _ => {}
            }
        }
    }
}
