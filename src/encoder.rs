use std::path::Path;

use anyhow::Error;
use cidre::{arc::Retained, av, cf, cm, ns};

const SCFRAMESTATUSCOMPLETE: isize = 0;

// External AVFoundation framework constants
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    // Video encoding keys
    // static AVVideoAverageBitRateKey: &'static cidre::ns::String;
    // static AVVideoProfileLevelKey: &'static cidre::ns::String;
    // static AVVideoProfileLevelH264HighAutoLevel: &'static cidre::ns::String;
    // static AVVideoExpectedSourceFrameRateKey: &'static cidre::ns::String;
    static AVVideoCodecTypeH264: &'static cidre::ns::String;

    // Color space keys
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
    input: Option<Retained<av::AssetWriterInput>>,
    start_time: Option<cm::Time>,
}

impl AVAssetWriterEncoder {
    pub fn init(output_path: &Path) -> Result<Self, Error> {
        let writer = av::AssetWriter::with_url_and_file_type(
            cf::Url::with_path(output_path, false)
                .unwrap()
                .as_ns(),
            av::FileType::mp4(),
        )?;

        Ok(Self {
            writer,
            input: None,
            start_time: None,
        })
    }

    pub fn encode(&mut self, sample_buf: &Retained<cm::SampleBuf>) -> Result<(), Error> {
        // Validate frame status
        let attachment_array = sample_buf.attaches(true).unwrap();
        let attachment = attachment_array.iter().next().unwrap();
        let status_raw_val = attachment.get(unsafe { 
            SCStreamFrameInfoStatus.as_ref()
        }).unwrap();
        let status_num = status_raw_val.as_number().as_ns().as_integer();

        // Skip frames with incomplete status
        if status_num != SCFRAMESTATUSCOMPLETE {
            return Ok(());
        }

        if self.input.is_none() {
        let dimensions = sample_buf.format_desc().unwrap().dimensions();
        let start_time = sample_buf.pts();
            self.start_time = Some(start_time);

        let mut dict = ns::DictionaryMut::new();

        dict.insert(
             unsafe { av::video_settings_keys::width().unwrap() },
            ns::Number::with_u32(dimensions.width as u32).as_id_ref(),
        );
        dict.insert(
            unsafe { av::video_settings_keys::height().unwrap() },
            ns::Number::with_u32(dimensions.height as u32).as_id_ref(),
        );
        dict.insert(
            av::video_settings_keys::codec(),
            unsafe { AVVideoCodecTypeH264 }.as_id_ref(),
        );

        let mut color_props = ns::DictionaryMut::new();
        color_props.insert(
            unsafe { AVVideoColorPrimariesKey },
            unsafe { AVVideoTransferFunction_ITU_R_709_2 },
        );
        color_props.insert(
            unsafe { AVVideoYCbCrMatrixKey },
            unsafe { AVVideoYCbCrMatrix_ITU_R_709_2 },
        );
        color_props.insert(
            unsafe { AVVideoTransferFunctionKey },
            unsafe { AVVideoTransferFunction_ITU_R_709_2 },
        );

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

        self.writer
            .add_input(&input)
            .map_err(|e| Error::msg(format!("Failed to add asset writer input: {}", e)))?;

        self.writer.start_writing();
        self.writer.start_session_at_src_time(self.start_time.unwrap());
        }
        if let Some(input) = &mut self.input {

        if input.is_ready_for_more_media_data() {

        input.append_sample_buf(sample_buf).unwrap();
        }
    }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        self.writer.finish_writing();
        Ok(())
    }
}
