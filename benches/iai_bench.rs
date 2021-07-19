mod spsc;

use spsc::iai_bench::{bounded_enqueue_dequeue, unbounded_enqueue_dequeue};

iai::main!(unbounded_enqueue_dequeue, bounded_enqueue_dequeue);
