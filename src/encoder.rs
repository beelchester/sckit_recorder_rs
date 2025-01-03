use anyhow::Error;
use arc::Retained;
use cidre::{objc::Obj, *};
use cm::SampleBuf;
use std::path::Path;

const SCFRAMESTATUSCOMPLETE: isize = 0;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    // static AVVideoAverageBitRateKey: &'static cidre::ns::String;
    // static AVVideoProfileLevelKey: &'static cidre::ns::String;
    // static AVVideoProfileLevelH264HighAutoLevel: &'static cidre::ns::String;
    static AVVideoCodecTypeH264: &'static cidre::ns::String;

    static AVVideoTransferFunctionKey: &'static cidre::ns::String;
    static AVVideoTransferFunction_ITU_R_709_2: &'static cidre::ns::String;
    static AVVideoColorPrimariesKey: &'static cidre::ns::String;
    // static AVVideoColorPrimaries_ITU_R_709_2: &'static cidre::ns::String;
    static AVVideoYCbCrMatrixKey: &'static cidre::ns::String;
    static AVVideoYCbCrMatrix_ITU_R_709_2: &'static cidre::ns::String;
    static SCStreamFrameInfoStatus: &'static cidre::ns::String;
}

pub struct AVAssetWriterEncoder {
    writer: Retained<av::AssetWriter>,
    input: Retained<av::AssetWriterInput>,
    first_ts: Option<cm::Time>,
    last_ts: Option<cm::Time>,
}

impl AVAssetWriterEncoder {
    pub fn init(width: u32, height: u32, output: &Path) -> Result<Self, Error> {
        let mut writer = av::AssetWriter::with_url_and_file_type(
            cf::Url::with_path(output, false).unwrap().as_ns(),
            av::FileType::mp4(),
        )?;

        let mut dict = ns::DictionaryMut::new();

        dict.insert(
            unsafe { av::video_settings_keys::width().unwrap() },
            ns::Number::with_u32(width).as_id_ref(),
        );
        dict.insert(
            unsafe { av::video_settings_keys::height().unwrap() },
            ns::Number::with_u32(height).as_id_ref(),
        );
        dict.insert(
            av::video_settings_keys::codec(),
            unsafe { AVVideoCodecTypeH264 }.as_id_ref(),
        );

        let mut color_props = ns::DictionaryMut::new();
        color_props.insert(unsafe { AVVideoColorPrimariesKey }, unsafe {
            AVVideoTransferFunction_ITU_R_709_2
        });
        color_props.insert(unsafe { AVVideoYCbCrMatrixKey }, unsafe {
            AVVideoYCbCrMatrix_ITU_R_709_2
        });
        color_props.insert(unsafe { AVVideoTransferFunctionKey }, unsafe {
            AVVideoTransferFunction_ITU_R_709_2
        });

        dict.insert(
            av::video_settings_keys::color_props(),
            color_props.as_id_ref(),
        );

        let mut input = av::AssetWriterInput::with_media_type_and_output_settings(
            av::MediaType::video(),
            Some(dict.as_ref()),
        )
        .map_err(|_| Error::msg("Failed to create AVAssetWriterInput"))?;
        input.set_expects_media_data_in_real_time(true);

        writer
            .add_input(&input)
            .map_err(|_| Error::msg("Failed to add asset writer input"))?;

        writer.start_writing();

        Ok(Self {
            input,
            writer,
            first_ts: None,
            last_ts: None,
        })
    }

    pub fn append_buf(&mut self, sample_buf: &Retained<SampleBuf>) -> Result<(), Error> {
        // Validate frame status
        let attachment_array = sample_buf.attaches(true).unwrap();
        let attachment = attachment_array.iter().next().unwrap();
        let status_raw_val = attachment
            .get(unsafe { SCStreamFrameInfoStatus.as_ref() })
            .unwrap();
        let status_num = status_raw_val.as_number().as_ns().as_integer();

        // Skip frames with incomplete status
        if status_num != SCFRAMESTATUSCOMPLETE {
            return Ok(());
        }

        if !self.input.is_ready_for_more_media_data() {
            println!("not ready for more data");
            return Ok(());
        }

        let time = sample_buf.pts();

        if self.first_ts.is_none() {
            self.writer.start_session_at_src_time(time);
            self.first_ts = Some(time);
        }

        self.last_ts = Some(time);

        self.input.append_sample_buf(sample_buf).ok();

        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), Error> {
        self.writer
            .end_session_at_src_time(self.last_ts.take().unwrap_or(cm::Time::zero()));
        self.input.mark_as_finished();
        self.writer.finish_writing();

        Ok(())
    }
}
