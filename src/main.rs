use std::{ops::Div, thread};

use lockfree::queues::mpsc::jiffy;

const ITERATIONS: usize = 1000000;
const SENDER_THREADS: usize = 10;

pub fn main() {
    let (mut rx, tx) = jiffy::queue();

    let ths: Vec<_> = (0..SENDER_THREADS)
        .map(|_| {
            let mut sender = tx.clone();
            thread::spawn(move || {
                let start = std::time::Instant::now();
                for i in 0..ITERATIONS {
                    sender.enqueue(i as u64);
                }
                let duration = start.elapsed();

                let per_insert = duration.div(ITERATIONS as u32);

                println!("Duration: {:?} / {}", duration, ITERATIONS);
                println!("Duration-Per-Enqueue: {:?}", per_insert);
            })
        })
        .collect();

    let receiver = thread::spawn(move || {
        let mut received = 0;
        loop {
            match rx.dequeue() {
                Some(_) => {
                    received += 1;
                }
                None => {}
            };

            if received != SENDER_THREADS * ITERATIONS {
                return;
            }
        }
    });

    for th in ths {
        th.join().unwrap();
    }
    receiver.join().unwrap();
}
