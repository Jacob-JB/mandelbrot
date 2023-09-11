
use std::{num::NonZeroU64, time::Instant};

use eframe::egui;


#[derive(Clone, Copy)]
struct Complex {
    r: f64,
    i: f64,
}

impl std::ops::Add for Complex {
    type Output = Complex;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Complex {
            r: self.r + rhs.r,
            i: self.i + rhs.i,
        }
    }
}

impl Complex {
    const ZERO: Self = Complex { r: 0., i: 0. };

    fn new(r: f64, i: f64) -> Complex {
        Complex { r, i }
    }

    #[inline]
    fn square(self) -> Complex {
        Complex {
            r: self.r * self.r - self.i * self.i,
            i: 2. * self.r * self.i,
        }
    }

    fn has_diverged(self) -> bool {
        (self.r * self.r + self.i * self.i) > 4.
    }

    fn compute_mandelbrot(self) -> Option<NonZeroU64> {
        let mut z = Complex::ZERO;

        for i in 1..500 {
            z = z.square() + self;

            if z.has_diverged() {
                return Some(NonZeroU64::new(i).unwrap());
            }
        }

        None
    }
}


fn main() {
    const SIZE: usize = 2usize.pow(14);
    let thread_count = num_cpus::get();

    let log_interval = SIZE / 100;

    let mut render = Box::new(vec![vec![None; SIZE]; SIZE]);

    println!("spawning {} worker threads", thread_count);
    let mut threads: Vec<_> = (0..thread_count).map(|_| {
        let (tx_row, rx_row) = std::sync::mpsc::channel();
        let (tx_result, rx_result) = std::sync::mpsc::channel();

        (
            tx_row,
            rx_result,
            std::thread::spawn(move || {
                while let Ok(Some(y)) = rx_row.recv() {
                    let mut result = vec![None; SIZE];

                    for x in 0..SIZE {
                        result[x] = Complex::new(
                            (x as f64 / SIZE as f64) * 4. - 2.,
                            (y as f64 / SIZE as f64) * 4. - 2.,
                        ).compute_mandelbrot();
                    }

                    // println!("{}: completed {}", thread_num, y);
                    tx_result.send(result).unwrap();
                }
            }),
            None,
        )
    }).collect();

    let mut next_row = 0;

    let mut completed_start_index = 0;
    let mut completed = std::collections::VecDeque::new();

    println!("starting");
    let start = Instant::now();
    loop {
        // receive results from threads
        for (
            _,
            rx,
            _,
            working
        ) in threads.iter_mut() {
            match rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("worker thread disconnected"),
                Err(std::sync::mpsc::TryRecvError::Empty) => (),
                Ok(result) => {
                    let row: usize = working.take().unwrap();
                    render[row] = result;

                    // add completed rows to memory
                    loop {
                        match completed.get_mut(row - completed_start_index) {
                            None => completed.push_back(false),
                            Some(entry) => {
                                *entry = true;
                                break entry;
                            },
                        }
                    };

                    // clear memory
                    while let Some(true) = completed.front() {
                        completed.pop_front();
                        completed_start_index += 1;
                    }
                }
            }
        }

        // dispatch work to threads
        threads.retain_mut(|(
            tx,
            _,
            _,
            working
        )| {
            if working.is_some() {
                true
            } else {
                if next_row < SIZE {
                    tx.send(Some(next_row)).unwrap();
                    *working = Some(next_row);
                    next_row += 1;
                    if next_row % log_interval == 0 {
                        println!("done {} / {} rows", next_row, SIZE);
                    }
                    true
                } else {
                    tx.send(None).unwrap();
                    false
                }
            }
        });

        // break when done
        if completed_start_index == SIZE {
            break;
        }
    }

    println!("done in {:#?}", start.elapsed());


    let mut texture: Option<egui::TextureHandle> = None;

    let options = eframe::NativeOptions::default();
    eframe::run_simple_native("ProgSoc 2023 Rust Ripoff", options, move |ctx, _frame| {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(0.0))
            .show(ctx, |ui| {

                let texture = texture.get_or_insert_with(|| {
                    let mut image = egui::ColorImage::new([SIZE, SIZE], egui::Color32::WHITE);
                    let width = image.width();

                    for (y, row) in image.pixels.chunks_mut(width).enumerate() {
                        for (x, pixel) in row.iter_mut().enumerate() {
                            if let Some(escape_time) = render[y][x] {
                                *pixel = egui::Color32::from_rgb(((Into::<u64>::into(escape_time) - 1) * 2u64.pow(5) % u8::MAX as u64) as u8, 0, 0);
                            } else {
                                *pixel = egui::Color32::BLACK
                            }
                        }
                    }

                    ctx.load_texture("colour-square", image, Default::default())
                });

                ui.centered_and_justified(|ui| {
                    ui.image(texture, egui::Vec2::splat(ui.available_size().min_elem()));
                });
            });
    }).unwrap();
}
