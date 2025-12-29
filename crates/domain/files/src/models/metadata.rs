use serde::{Deserialize, Serialize};

pub use super::image_metadata::ImageMetadata;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksums: Option<FileChecksums>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_specific: Option<TypeSpecificMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TypeSpecificMetadata {
    Image(ImageMetadata),
    Document(DocumentMetadata),
    Audio(AudioMetadata),
    Video(VideoMetadata),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AudioMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct VideoMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileChecksums {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

impl FileMetadata {
    pub const fn new() -> Self {
        Self {
            checksums: None,
            type_specific: None,
        }
    }

    pub fn with_image(mut self, image: ImageMetadata) -> Self {
        self.type_specific = Some(TypeSpecificMetadata::Image(image));
        self
    }

    pub fn with_document(mut self, doc: DocumentMetadata) -> Self {
        self.type_specific = Some(TypeSpecificMetadata::Document(doc));
        self
    }

    pub fn with_audio(mut self, audio: AudioMetadata) -> Self {
        self.type_specific = Some(TypeSpecificMetadata::Audio(audio));
        self
    }

    pub fn with_video(mut self, video: VideoMetadata) -> Self {
        self.type_specific = Some(TypeSpecificMetadata::Video(video));
        self
    }

    pub fn with_checksums(mut self, checksums: FileChecksums) -> Self {
        self.checksums = Some(checksums);
        self
    }
}

impl DocumentMetadata {
    pub const fn new() -> Self {
        Self {
            title: None,
            author: None,
            page_count: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub const fn with_page_count(mut self, page_count: u32) -> Self {
        self.page_count = Some(page_count);
        self
    }
}

impl AudioMetadata {
    pub const fn new() -> Self {
        Self {
            duration_seconds: None,
            sample_rate: None,
            channels: None,
        }
    }

    pub const fn with_duration_seconds(mut self, duration: f32) -> Self {
        self.duration_seconds = Some(duration);
        self
    }

    pub const fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = Some(sample_rate);
        self
    }

    pub const fn with_channels(mut self, channels: u8) -> Self {
        self.channels = Some(channels);
        self
    }
}

impl VideoMetadata {
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            duration_seconds: None,
            frame_rate: None,
        }
    }

    pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub const fn with_duration_seconds(mut self, duration: f32) -> Self {
        self.duration_seconds = Some(duration);
        self
    }

    pub const fn with_frame_rate(mut self, frame_rate: f32) -> Self {
        self.frame_rate = Some(frame_rate);
        self
    }
}

impl FileChecksums {
    pub const fn new() -> Self {
        Self {
            md5: None,
            sha256: None,
        }
    }

    pub fn with_md5(mut self, md5: impl Into<String>) -> Self {
        self.md5 = Some(md5.into());
        self
    }

    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = Some(sha256.into());
        self
    }
}
