extern crate ansi_term;
extern crate alto;
#[macro_use]
extern crate error_chain;
extern crate rand;
extern crate num;


error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }
    foreign_links {
        Io(::std::io::Error) #[cfg(unix)];
        Alto(alto::AltoError);
    }
    errors {
    }
}

use std::f64;
use std::time::Duration;
use num::Float;
use std::ops::Range;

fn _value(f:(u64, u64)) -> f64 {
    f.0 as f64 / f.1 as f64
}
fn median(f0:(u64, u64), f1:(u64, u64)) -> (u64, u64) {
    (f0.0 + f1.0, f0.1 + f1.1)
}
fn multiply(f0:(u64, u64), f1:(u64, u64)) -> (u64, u64) {
    (f0.0 * f1.0, f0.1 * f1.1)
}


/// Marches through a Stern-Brocot tree using a vec of booleans
/// as predicate for branching. 
fn iterate_on_sb_tree(vec:&Vec<bool>) -> (u64, u64) {
    let mut low = (0, 1);
    let mut high = (1, 0);
    let mut med = median(low, high);

    for &b in vec  {
        if b {
            high = med;
        } else {
            low = med;
        }
        med = median(low, high);
    }
    med
}

use std::sync::Arc;
use alto::*;
use std::io;
use std::io::Read;

fn main() {
	let mut p = Player::new().unwrap();
    let base = ROOT_NOTE;
    let fr = (1, 1);
    //let mut rng = rand::thread_rng();
    let mut v = Vec::<bool>::new();
    let mut notes = Vec::<Note>::new();
    let mut skipping = false;

    println!("starting");

    /*
    match get char
        get key
        a push left
        b push right
        space apply then play sound
        r reset
    */

    for c in io::stdin().bytes() {
        let c = c.unwrap();
        print!("{}", c);
        if skipping {
            if c as char == '\n' {
                skipping = false;
            }
            continue
        }
        match c as char {
            'a' => v.push(false),
            'q' => v.push(true),
            'A' => v.extend_from_slice(&[false, false]),
            'Q' => v.extend_from_slice(&[true, true]),
            'z' | 'Z' => v.extend_from_slice(&[false, true]),
            's' | 'S' => v.extend_from_slice(&[true, false]),
            ' ' => {
                let fr = multiply(fr, iterate_on_sb_tree(&v));
                v.clear();
                let hz = base * fr.0 as f64 / fr.1 as f64;
                notes.push(Note {hz, duration:0.6f64, profile:&profiles::sqrt});
            }
            '\n' => {
                let fr = multiply(fr, iterate_on_sb_tree(&v));
                v.clear();
                let hz = base * fr.0 as f64 / fr.1 as f64;
                notes.push(Note {hz, duration:0.6f64, profile:&profiles::sqrt});

                p.play(notes.clone(), Duration::from_millis(200));
                notes.clear();
            }
            '#' => skipping = true,
            _ => ()
        }
    }
}

const SIN_LEN:usize = 44_000;
const ROOT_NOTE:f64 = 440.0;

mod profiles {
    pub fn _sqrt2(x:f64) -> f64 {
        let x = x.min(1f64).max(0f64);
        let x = x.powf(0.10) - x.powf(5.0) + (x * 100.0 * ::std::f64::consts::PI).sin() / 10.0;
        x * (x.sqrt() - 1.0)
    }
    pub fn _sqrt3(x:f64) -> f64 {
        let mut x:f64 = x;// / ::std::f64::consts::PI;
        if x > 1. {
           x = 1.;
        }
        (x.powf(0.1) - x.sqrt()).powf(4.) * 10. + 0.3 - 0.3 * (x/2.).powf(10.)

        //1. - (x * ::std::f64::consts::PI).powf(1. / 50.) * 3. + 0.1
    }
    pub fn sqrt(x:f64) -> f64 {
        if x < 0.0 {
            0.0
        } else if x < 0.5 {
            x
        } else if x < 1.0 {
            1.0 - x
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy)]
struct Note {
    duration:f64,
    hz:f64,
    profile:&'static Fn(f64)->f64,
}

use std::fmt;

impl fmt::Debug for Note {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Note {{ duration: {}, hz: {}, ratio: {}, profile_id: {:?} }}",
            self.duration,
            self.hz,
            self.hz / ROOT_NOTE,
            self.profile as *const _)
    }
}
use num::clamp;

impl Note {

    //Take the aplitude of a note, using time since start of note
    pub fn value(&self, cursor:f64) -> f64 {
        let t = clamp(cursor / self.duration, 0.0, 1.0);
        self.private_value(cursor, 30) * Note::adjusted_volume(self.hz) * (*self.profile)(t)
    }

    fn private_value(&self, cursor:f64, iter:u32) -> f64 {

        let mut val = 0.;
        for i in 1 .. iter { 
            val += (self.hz * (cursor as f64 + (i as f64)) * i as f64).sin() / (3.0).powf((i - 1) as f64);
            val += (self.hz * (cursor as f64 + (i as f64)) / i as f64).sin() / (3.0).powf((i - 1) as f64);
        }
        val
    }
    /// Maps a float inside from one interval to another
    /// If the input doesn't lie inside the first interval, it will be clamped
    /// Output is garanteed to be in second interval
    /// Behavior undefined for non-finite ranges or inputs
    fn map_interval_clamped(f:f64, int_a:Range<f64>, int_b:Range<f64>) -> f64 {
        let f = clamp(f, int_a.start, int_a.end);
        let f = (f  - int_a.start) / (int_a.end - int_a.start);
        f * (int_b.end - int_b.start) + int_b.start
    }

    fn adjusted_volume(hz:f64) -> f64 {
        Note::map_interval_clamped(hz.ln(), 4.6 .. 8.6, 1.5 .. 0.5)
    }
}

impl Player {

    fn get_reverb_slot(ctx:&Context) -> Result<efx::AuxEffectSlot> {
        let mut slot = ctx.new_aux_effect_slot()?;
        let mut reverb: efx::EaxReverbEffect = ctx.new_effect()?;
        reverb.set_preset(&efx::REVERB_PRESET_GENERIC)?;
        slot.set_effect(&reverb)?;
        Ok(slot)
    }

    fn new() -> Result<Player> {
        let alto = Alto::load_default()?;
        println!("Using output: {:?}", alto.default_output());
        let dev = alto.open(None)?;
        let ctx = dev.new_context(None)?;
        
        let slot = if dev.is_extension_present(alto::ext::Alc::Efx) {
            println!("Using EFX reverb");
            Player::get_reverb_slot(&ctx).map_err(|e| println!("{:?}", e)).ok()
        } else {
            println!("EFX not present");
            None
        };
        Ok(Player{ctx, slot, wave:SinWave::default()})
    }

    fn play(&mut self, hz:Vec<Note>, duration:Duration) {

        println!("{:?}hz", hz);
        self.wave.notes = hz.iter().map(|&note| (note, self.wave.cursor)).collect::<Vec<_>>();

        let buf = self.wave.by_ref().take(SIN_LEN as usize).collect::<Vec<_>>();
        let buf = self.ctx.new_buffer(buf, SIN_LEN as i32).unwrap();
        let buf = Arc::new(buf);

        let mut src = self.ctx.new_static_source().unwrap();
        src.set_buffer(buf).unwrap();
        src.set_looping(true);
        if let Some(ref mut slot) = self.slot {
            src.set_aux_send(0, slot).unwrap();
        }

        src.play();

        std::thread::sleep(duration);
    }
}

struct Player {
    ctx:Context,
    slot:Option<efx::AuxEffectSlot>,
    wave:SinWave,
}


struct SinWave {
    notes: Vec<(Note, f64)>,
	vol: f64,
    cursor: f64,
}


impl SinWave {
    pub fn default() -> SinWave {
        SinWave{notes:Vec::new(), vol:0.2, cursor:0.0}
    }
}


impl Iterator for SinWave {
	type Item = Mono<i16>;

	fn next(&mut self) -> Option<Mono<i16>> {


        let v:f64 = self.notes
            .iter()
            .map(|&(ref note, ref start)| note.value(self.cursor - start))
            .sum();
        
        self.cursor += 2.0 * std::f64::consts::PI / SIN_LEN as f64;

        let v = v * self.vol / (self.notes.len() as f64);

		Some(Mono{center: (v * std::i16::MAX as f64) as i16})
	}
}


/*

approximates desired frequencies as steps through the stern brocot tree


use std::cmp::Ordering;
use ansi_term::Colour::*;
fn main() {

    play_synth();
    //println!("notes");
    //print_approximations(2f64.powf(0f64 / 12f64), 30);
    //print_approximations(2f64.powf(1f64 / 12f64), 30);
    //print_approximations(2f64.powf(2f64 / 12f64), 30);
    //print_approximations(2f64.powf(3f64 / 12f64), 30);
    //print_approximations(2f64.powf(4f64 / 12f64), 30);
    //print_approximations(2f64.powf(5f64 / 12f64), 30);
    //print_approximations(2f64.powf(6f64 / 12f64), 30);
    //print_approximations(2f64.powf(7f64 / 12f64), 30);
    //print_approximations(2f64.powf(8f64 / 12f64), 30);
    //print_approximations(2f64.powf(9f64 / 12f64), 30);
    //print_approximations(2f64.powf(10f64 / 12f64), 30);
    //print_approximations(2f64.powf(11f64 / 12f64), 30);
    //print_approximations(2f64.powf(12f64 / 12f64), 30);

}

fn palette(t:f64) -> (u8, u8, u8)
{
    let a = (0.5,0.5,0.5);
    let b = (0.5,0.5,0.5);
    let c = (1.0,1.0,1.0);
    let d = (0.0,0.10,0.20);

    let r =(a.0 + b.0 * f64::cos(6.28318 * (c.0 * t + d.0)),
            a.1 + b.1 * f64::cos(6.28318 * (c.1 * t + d.1)),
            a.2 + b.2 * f64::cos(6.28318 * (c.2 * t + d.2)));

    ((r.0 * 255f64) as u8, (r.1 * 255f64) as u8, (r.2 * 255f64) as u8)
}

fn print_approximations(f:f64, n:u64) -> (u64, u64) {
    assert!(f >= 0f64);

    let mut low = (0, 1);
    let mut high = (f64::round(f) as u64, 0);

    let mut best_diff = f64::INFINITY;
    let mut best_median = (1, 0);

    //println!("approximating {} to the {}th iteration", f, n);

    for i in 0..n {
        let median = (low.0 + high.0, low.1 + high.1);

        //first definition of the diophantine approximation
        //let current_diff = f64::abs(value(median) - f);
        //second definition
        let current_diff = f64::abs(median.1 as f64 * f - median.0 as f64);

        if current_diff < best_diff {
            best_diff = current_diff;
            best_median = median;
            let p = palette(f64::sqrt(best_diff));
            print!("{}", RGB(p.0, p.1, p.2).paint(format!("{}/{} ", best_median.0, best_median.1)));
        } else {
            let p = palette(f64::sqrt(i as f64 / 10f64));
            print!("{}", RGB(p.0, p.1, p.2).paint("|"));
        }
        
        match f.partial_cmp(&value(median)).unwrap() {
            Ordering::Greater => low = median,
            Ordering::Less=> high = median,
            Ordering::Equal => break,
        }
    }
    println!("");
    best_median
    //println!("\t{:?} : {}", best_median, best_diff);
}

use rand::Rng;
use rand::distributions::Range as RandRange;
use rand::distributions::IndependentSample;

old algo for random play

for _ in 0 .. {
    let mut r_p = Range::new(1u32, 5).ind_sample(&mut rng);
    let mut r_q = Range::new(1u32, 6).ind_sample(&mut rng);
    if f < 220f64 {
        r_p += 1
    }
    if f > 880f64 {
        r_q += 1;
    }
    if r_q == r_p {
        continue
    }
    println!("{}/{}", r_p, r_q);

    f *= r_p as f64 / r_q as f64;

    let mut tempo = 50_000 *  Range::new(2u32, 10).ind_sample(&mut rng);
    if rng.gen() {
        tempo = prev_tempo;
    }
    prev_tempo = tempo;
    println!("{}/{} for {}", r_p, r_q, tempo);
    
    p.play(f, Duration::from_micros(tempo as u64));
}*/
