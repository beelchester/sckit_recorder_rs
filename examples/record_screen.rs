use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use sckit_recorder_rs::{capturer::Capturer, encoder::AVAssetWriterEncoder};

#[tokio::main]
async fn main() {
    let (tx, rx) = std::sync::mpsc::channel();
    let stream = Capturer::init().unwrap();
    stream.start(tx).await;
    let (width, height) = stream.get_dimensions();

    let output = Path::new("output.mp4");
    let mut encoder = AVAssetWriterEncoder::init(width, height, output).unwrap();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);
    println!("Recording started");
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        stop_flag_clone.store(true, Ordering::SeqCst);
    });

    while let Ok(sample_buf) = rx.recv() {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }
        encoder.append_buf(&sample_buf).unwrap();
    }

    encoder.finish().unwrap();
    println!("Done");

    _ = stream.stop().await;
}
