use crate::SpiderBot;
use crate::cache::{GifCacheReader, GifCacheWriter};
use crate::commands::CommandError;
use klipy::Klipy;

pub(crate) type Context<'a, 'klipy_config> =
    poise::Context<'a, SpiderBot<'klipy_config>, CommandError>;

pub(crate) trait GifCacheExt {
    fn gif_cache(&self) -> &GifCacheReader;
}

pub(crate) trait GifCacheWriterExt {
    fn gif_cache_writer(&self) -> &GifCacheWriter;
}

pub(crate) trait KlipyExt<'klipy_config> {
    fn klipy(&self) -> &Klipy<'klipy_config>;
}

pub(crate) trait GifContextExt<'klipy_config>:
    KlipyExt<'klipy_config> + GifCacheExt + GifCacheWriterExt
{
    fn gif_context(&self) -> (&Klipy<'klipy_config>, &GifCacheReader, &GifCacheWriter) {
        (self.klipy(), self.gif_cache(), self.gif_cache_writer())
    }
}

impl<'klipy_config> KlipyExt<'klipy_config> for Context<'_, 'klipy_config> {
    fn klipy(&self) -> &Klipy<'klipy_config> {
        &self.framework().user_data.klipy
    }
}

impl GifCacheExt for Context<'_, '_> {
    fn gif_cache(&self) -> &GifCacheReader {
        &self.framework().user_data.gif_cache
    }
}

impl GifCacheWriterExt for Context<'_, '_> {
    fn gif_cache_writer(&self) -> &GifCacheWriter {
        &self.framework().user_data.gif_cache_writer
    }
}

impl<'klipy_config> GifContextExt<'klipy_config> for Context<'_, 'klipy_config> {
    fn gif_context(&self) -> (&Klipy<'klipy_config>, &GifCacheReader, &GifCacheWriter) {
        let data = self.framework().user_data;
        (&data.klipy, &data.gif_cache, &data.gif_cache_writer)
    }
}
