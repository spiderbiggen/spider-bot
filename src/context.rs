use crate::SpiderBot;
use crate::cache::{GifCacheReader, GifCacheWriter};
use crate::interactions::CommandError;
use klipy::Klipy;

pub(crate) type Context<'a> = poise::Context<'a, SpiderBot, CommandError>;

pub(crate) trait GifCacheExt {
    fn gif_cache(&self) -> &GifCacheReader;
}

pub(crate) trait GifCacheWriterExt {
    fn gif_cache_writer(&self) -> &GifCacheWriter;
}

pub(crate) trait KlipyExt {
    fn klipy(&self) -> &Klipy<'static>;
}

pub(crate) trait GifContextExt: KlipyExt + GifCacheExt + GifCacheWriterExt {
    fn gif_context(&self) -> (&Klipy<'static>, &GifCacheReader, &GifCacheWriter) {
        (self.klipy(), self.gif_cache(), self.gif_cache_writer())
    }
}

impl KlipyExt for Context<'_> {
    fn klipy(&self) -> &Klipy<'static> {
        &self.serenity_context().data_ref::<SpiderBot>().klipy
    }
}

impl GifCacheExt for Context<'_> {
    fn gif_cache(&self) -> &GifCacheReader {
        &self.serenity_context().data_ref::<SpiderBot>().gif_cache
    }
}

impl GifCacheWriterExt for Context<'_> {
    fn gif_cache_writer(&self) -> &GifCacheWriter {
        &self
            .serenity_context()
            .data_ref::<SpiderBot>()
            .gif_cache_writer
    }
}

impl GifContextExt for Context<'_> {
    fn gif_context(&self) -> (&Klipy<'static>, &GifCacheReader, &GifCacheWriter) {
        let data = self.serenity_context().data_ref::<SpiderBot>();
        (&data.klipy, &data.gif_cache, &data.gif_cache_writer)
    }
}
