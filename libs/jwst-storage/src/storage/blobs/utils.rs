use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{
    stream::{iter, StreamExt},
    Stream,
};
use image::{load_from_memory, ImageOutputFormat, ImageResult};
use jwst::{Base64Engine, BlobMetadata, URL_SAFE_ENGINE};
use sea_orm::FromQueryResult;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io::Cursor};

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

    fn output_format(&self) -> ImageOutputFormat {
        match self.format {
            ImageFormat::Jpeg => ImageOutputFormat::Jpeg(80),
            ImageFormat::WebP => ImageOutputFormat::WebP,
        }
    }

    pub fn optimize_image(&self, data: &[u8]) -> ImageResult<Vec<u8>> {
        let mut buffer = Cursor::new(vec![]);
        let image = load_from_memory(data)?;
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
                return Ok(Self {
                    format,
                    width,
                    height,
                });
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

pub async fn get_hash(stream: impl Stream<Item = Bytes> + Send) -> (String, Vec<u8>) {
    let mut hasher = Sha256::new();

    let buffer = stream
        .flat_map(|buffer| {
            hasher.update(&buffer);
            iter(buffer)
        })
        .collect()
        .await;

    let hash = URL_SAFE_ENGINE.encode(hasher.finalize());
    (hash, buffer)
}

#[derive(FromQueryResult)]
pub(super) struct InternalBlobMetadata {
    pub(super) size: i64,
    pub(super) created_at: DateTime<Utc>,
}

impl From<InternalBlobMetadata> for BlobMetadata {
    fn from(val: InternalBlobMetadata) -> Self {
        BlobMetadata {
            content_type: "application/octet-stream".into(),
            last_modified: val.created_at.naive_local(),
            size: val.size as u64,
        }
    }
}
