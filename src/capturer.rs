use anyhow::Error;
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
use futures::executor::block_on;
use std::
    sync::
        mpsc::Sender
    
;
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

pub struct Capturer {
    stream: Retained<sc::Stream>,
    dimensions: (u32, u32),
}

impl Capturer {
    //TODO: allow config input
    pub fn init() -> Result<Self, Error> {
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

            let filter = sc::ContentFilter::with_display_excluding_windows(
                display,
                &cidre::ns::Array::new(),
            );

            let stream = sc::Stream::new(&filter, &cfg);

            return Ok(Self {
                stream,
                dimensions: (width as u32, height as u32),
            });
        }
        Err(Error::msg("Failed to get content"))
    }
    pub async fn start(&self, tx: Sender<Retained<cm::SampleBuf>>) {
        let queue = dispatch::Queue::serial_with_ar_pool();
        let inner = StreamOutputInner { sender: tx };
        let delegate = StreamOutput::with(inner);
        self.stream
            .add_stream_output(delegate.as_ref(), sc::OutputType::Screen, Some(&queue))
            .unwrap();
        self.stream.start().await.unwrap();
    }
    pub async fn stop(&self) {
        self.stream.stop().await.unwrap();
    }
    pub fn get_dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
}
