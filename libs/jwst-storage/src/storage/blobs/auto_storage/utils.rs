use std::collections::HashMap;

enum ImageFormat {
    Jpeg,
    WebP,
}

pub struct ImageParams {
    format: ImageFormat,
    width: Option<usize>,
    height: Option<usize>,
}

impl ImageParams {
    #[inline]
    fn check_size(w: Option<usize>, h: Option<usize>) -> bool {
        if let Some(w) = w {
            if w % 320 != 0 || w > 1920 {
                return false;
            }
        }
        if let Some(h) = h {
            if h % 180 != 0 || h > 1080 {
                return false;
            }
        }
        true
    }

    pub(super) fn format(&self) -> String {
        match self.format {
            ImageFormat::Jpeg => "jpeg".to_string(),
            ImageFormat::WebP => "webp".to_string(),
        }
    }

    fn output_format(&self) -> image::ImageOutputFormat {
        match self.format {
            ImageFormat::Jpeg => image::ImageOutputFormat::Jpeg(80),
            ImageFormat::WebP => image::ImageOutputFormat::WebP,
        }
    }

    pub fn optimize_image(&self, data: &[u8]) -> image::ImageResult<Vec<u8>> {
        let mut buffer = std::io::Cursor::new(vec![]);
        let image = image::load_from_memory(data)?;
        image.write_to(&mut buffer, self.output_format())?;
        Ok(buffer.into_inner())
    }
}

impl TryFrom<&HashMap<String, String>> for ImageParams {
    type Error = ();

    fn try_from(value: &HashMap<String, String>) -> Result<Self, Self::Error> {
        let mut format = None;
        let mut width = None;
        let mut height = None;
        for (key, value) in value {
            match key.as_str() {
                "format" => {
                    format = match value.as_str() {
                        "jpeg" => Some(ImageFormat::Jpeg),
                        "webp" => Some(ImageFormat::WebP),
                        _ => return Err(()),
                    }
                }
                "width" => width = value.parse().ok(),
                "height" => height = value.parse().ok(),
                _ => return Err(()),
            }
        }

        if let Some(format) = format {
            if Self::check_size(width, height) {
                return Ok(Self { format, width, height });
            }
        }
        Err(())
    }
}

impl ToString for ImageParams {
    fn to_string(&self) -> String {
        let mut params = String::new();

        params.push_str(&format!("format={}", self.format()));
        if let Some(width) = &self.width {
            params.push_str(&format!("width={}", width));
        }
        if let Some(height) = &self.height {
            params.push_str(&format!("height={}", height));
        }
        params
    }
}
