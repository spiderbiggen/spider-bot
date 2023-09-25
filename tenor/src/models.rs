use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use strum_macros::{EnumString, IntoStaticStr};
use url::Url;

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    pub results: T,
    pub next: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MediaFormat {
    pub url: Url,
}

#[derive(Deserialize, Clone)]
pub struct Gif {
    pub id: String,
    pub title: String,
    pub url: Url,
    #[serde(rename = "itemurl")]
    pub item_url: Url,
    pub media_formats: HashMap<MediaFilter, MediaFormat>,
}

impl Debug for Gif {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gif")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("url", &self.url.as_str())
            .field("item_url", &self.item_url.as_str())
            .field("media_formats", &self.media_formats)
            .finish()
    }
}

/// Tenor supports filtering content based on ratings that map to the Motion Picture Association (MPA)
/// It's important to note that tenor doesn't surface the type of nudity that can be found in R-rated films.
/// If you become aware of such content, inform Tenor immediately by contacting support@tenor.com.
#[derive(Debug, Copy, Clone, PartialEq, EnumString, IntoStaticStr)]
pub enum ContentFilter {
    /// Rated G
    #[strum(serialize = "high")]
    High,
    /// Rated G and PG
    #[strum(serialize = "medium")]
    Medium,
    /// Rated G, PG, and PG-13
    #[strum(serialize = "low")]
    Low,
    /// Rated G, PG, PG-13 and R (no nudity)
    #[strum(serialize = "off")]
    Off,
}

impl Default for ContentFilter {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, EnumString, IntoStaticStr, Deserialize)]
pub enum MediaFilter {
    /// - Resolution and size: High quality single frame GIF format; smaller in size than the GIF format
    /// - Dimensions: Original upload dimensions (no limits)
    /// - Usage notes: Make this the first frame of the content. It's intended for use as a thumbnail preview.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "preview")]
    #[serde(rename = "preview")]
    Preview,
    /// - Resolution and size: High-quality GIF format; largest file size available
    /// - Dimensions: Original upload dimensions (no limits)
    /// - Usage notes: Use this size for GIF shares on desktop.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "gif")]
    #[serde(rename = "gif")]
    Gif,
    /// - Resolution and size: Small reduction in size of the GIF format
    /// - Dimensions: Original upload dimensions (no limits) but much higher compression rate
    /// - Usage notes: Use this size for GIF previews on desktop.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "mediumgif")]
    #[serde(rename = "mediumgif")]
    MediumGif,
    /// - Resolution and size: Reduced size of the GIF format
    /// - Dimensions: Up to 220 pixels wide. Height scaled to preserve the aspect ratio.
    /// - Usage notes: Use this size for GIF previews and shares on mobile.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "tinygif")]
    #[serde(rename = "tinygif")]
    TinyGif,
    /// - Resolution and size: Smallest size of the GIF format
    /// - Dimensions: Up to 90 pixels tall. Width scaled to preserve the aspect ratio.
    /// - Usage notes: Use this size for GIF previews on mobile.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "nanogif")]
    #[serde(rename = "nanogif")]
    NanoGif,
    /// - Resolution and size: Highest quality video format; largest of the video formats, but smaller than GIF
    /// - Dimensions: Similar to GIF but padded to fit video container specifications, which are usually 8-pixel increments.
    /// - Usage notes: Use this size for MP4 previews and shares on desktop.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "mp4")]
    #[serde(rename = "mp4")]
    Mp4,
    /// - Resolution and size: Highest quality video format; larger in size than MP4
    /// - Dimensions: Similar to GIF but padded to fit video container specifications, which are usually 8-pixel increments.
    /// - Usage notes: Use this size for MP4 shares when you want the video clip to run a few times rather than only once.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "loopedmp4")]
    #[serde(rename = "loopedmp4")]
    LoopedMp4,
    /// - Resolution and size: Reduced size of the MP4 format
    /// - Dimensions: Variable width and height, with a maximum bounding box of 320x320 pixels
    /// - Usage notes: Use this size for MP4 previews and shares on mobile.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "tinymp4")]
    #[serde(rename = "tinymp4")]
    TinyMp4,
    /// - Resolution and size: Smallest size of the MP4 format
    /// - Dimensions: Variable width and height, with a maximum bounding box of 150x150 pixels
    /// - Usage notes: Use this size for MP4 previews on mobile.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "nanomp4")]
    #[serde(rename = "nanomp4")]
    NanoMp4,
    /// - Resolution and size: Lower quality video format; smaller in size than MP4
    /// - Dimensions: Similar to GIF but padded to fit video container specifications, which are usually 8-pixel increments.
    /// - Usage notes: Use this size for WebM previews and shares on desktop.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "webm")]
    #[serde(rename = "webm")]
    Webm,
    /// - Resolution and size: Reduced size of the WebM format
    /// - Dimensions: Variable width and height, with a maximum bounding box of 320x320 pixels
    /// - Usage notes: Use this size for GIF shares on mobile.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "tinywebm")]
    #[serde(rename = "tinywebm")]
    TinyWebm,
    /// - Resolution and size: Smallest size of the WebM format
    /// - Dimensions: Variable width and height, with a maximum bounding box of 150x150 pixels
    /// - Usage notes: Use this size for GIF previews on mobile.
    ///
    /// This format is supported for GIFs and stickers.
    #[strum(serialize = "nanowebm")]
    #[serde(rename = "nanowebm")]
    NanoWebm,
    /// - Resolution and size: High-quality WebP sticker format; largest file size available
    /// - Dimensions: Original upload dimensions (no limits)
    /// - Usage notes: Use this size for sticker shares for high-bandwidth users.
    ///
    /// This format is supported for stickers.
    #[strum(serialize = "webp_transparent")]
    #[serde(rename = "webp_transparent")]
    WebpTransparent,
    /// - Resolution and size: Reduced size of the WebP sticker format; maximum size of 500 KB
    /// - Dimensions: Up to 220x220 pixels, height scaled to preserve the aspect ratio.
    /// - Usage notes: Use this size for sticker previews for high-bandwidth users
    ///                and shares for low-bandwidth users.
    ///
    /// This format is supported for stickers.
    #[strum(serialize = "tinywebp_transparent")]
    #[serde(rename = "tinywebp_transparent")]
    TinyWebpTransparent,
    /// - Resolution and size: Smallest size of the WebP sticker format; maximum size of 100 KB
    /// - Dimensions: Up to 90x90 pixels, with the width scaled to preserve the aspect ratio.
    /// - Usage notes: Use this size for sticker previews for low-bandwidth users.
    ///
    /// This format is supported for stickers.
    #[strum(serialize = "nanowebp_transparent")]
    #[serde(rename = "nanowebp_transparent")]
    NanoWebpTransparent,
    /// - Resolution and size: High-quality GIF sticker format; largest file size available
    /// - Dimensions: Original upload dimensions (no limits)
    /// - Usage notes: Use this size for sticker shares for high-bandwidth users.
    ///
    /// This format is supported for stickers.
    #[strum(serialize = "gif_transparent")]
    #[serde(rename = "gif_transparent")]
    GifTransparent,
    /// - Resolution and size: Reduced size of the GIF sticker format; maximum size of 500 KB
    /// - Dimensions: Up to 220x220 pixels, with the height scaled to preserve the aspect ratio.
    /// - Usage notes: Use this size for sticker previews for high-bandwidth users
    ///                and shares for low-bandwidth users.
    ///
    /// This format is supported for stickers.
    #[strum(serialize = "tinygif_transparent")]
    #[serde(rename = "tinygif_transparent")]
    TinyGifTransparent,
    /// - Resolution and size: Smallest size of the GIF sticker format; maximum size of 100 KB
    /// - Dimensions: Up to 90x90 pixels, with the width scaled to preserve the aspect ratio.
    /// - Usage notes: Use this size for sticker previews for low-bandwidth users.
    ///
    /// This format is supported for sticker.
    #[strum(serialize = "nanogif_transparent")]
    #[serde(rename = "nanogif_transparent")]
    NanoGifTransparent,
}
