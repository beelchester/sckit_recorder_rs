mod encoder;

use cidre::{
    arc::Retained,
    cf,
    cm::{self},
    cv, define_obj_type, dispatch, objc,
    sc::{
        self,
        stream::{Output, OutputImpl},
    },
};
use encoder::AVAssetWriterEncoder;
use futures::executor::block_on;
use std::{ path::Path, sync::{atomic::{AtomicBool, Ordering}, mpsc::Sender, Arc}, thread, time::Duration};

struct StreamOutputInner {
    sender: Sender<Retained<cm::SampleBuf>>,
}

impl StreamOutputInner {
    fn handle_video(&mut self, sample_buf: &mut cm::SampleBuf) {
        let sample_buf = sample_buf.retained();
        self.sender.send(sample_buf).unwrap();
    }
}

define_obj_type!(StreamOutput + OutputImpl, StreamOutputInner, STREAM_OUTPUT);

impl Output for StreamOutput {}

#[objc::add_methods]
impl OutputImpl for StreamOutput {
    extern "C" fn impl_stream_did_output_sample_buf(
        &mut self,
        _cmd: Option<&cidre::objc::Sel>,
        _stream: &sc::Stream,
        sample_buf: &mut cm::SampleBuf,
        kind: sc::OutputType,
    ) {
        match kind {
            sc::OutputType::Screen => self.inner_mut().handle_video(sample_buf),
            sc::OutputType::Audio => {}
            sc::OutputType::Mic => {}
        }
    }
}

#[tokio::main]
async fn main() {
    let content = block_on(sc::ShareableContent::current());
    if let Ok(content) = content {
        let displays = content.displays().clone();
        let display = displays.first().unwrap();
        let scale_factor = 2;

        let width = display.width() as usize * scale_factor;
        let height = display.height() as usize * scale_factor;

        let mut cfg = sc::StreamCfg::new();
        cfg.set_width(width);
        cfg.set_height(height);
        cfg.set_pixel_format(cv::PixelFormat::_32_BGRA);
        let color_space = cf::String::from_str("kCGColorSpaceSRGB");
        cfg.set_color_space_name(&color_space);
        cfg.set_minimum_frame_interval(cm::Time {
            value: 1,
            scale: 60,
            ..Default::default()
        });

        let filter =
            sc::ContentFilter::with_display_excluding_windows(display, &cidre::ns::Array::new());

        let queue = dispatch::Queue::serial_with_ar_pool();

        let stream = sc::Stream::new(&filter, &cfg);
        let (tx, rx) = std::sync::mpsc::channel();
        let inner = StreamOutputInner { sender: tx };
        let delegate = StreamOutput::with(inner);
        stream
            .add_stream_output(delegate.as_ref(), sc::OutputType::Screen, Some(&queue))
            .unwrap();
        stream.start().await.unwrap();

        let mut encoder =
            AVAssetWriterEncoder::init(Path::new("output.mp4")).unwrap();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        thread::spawn(move || {
            thread::sleep(Duration::from_secs(5));
            stop_flag_clone.store(true, Ordering::SeqCst);
        });

        while let Ok(sample_buf) = rx.recv() {
            if stop_flag.load(Ordering::SeqCst) {
                break;
            }
            encoder.encode(&sample_buf).unwrap();
        }

        encoder.stop().unwrap();
        println!("Done");

        _ = stream.stop().await;
    }
}
