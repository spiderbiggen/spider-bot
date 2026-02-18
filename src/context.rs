use crate::commands::CommandError;
use crate::{GifCache, SpiderBot};
use klipy::Klipy;

pub(crate) type Context<'a, 'klipy_config> =
    poise::Context<'a, SpiderBot<'klipy_config>, CommandError>;

pub(crate) trait GifCacheExt {
    fn gif_cache(&self) -> &GifCache;
}

pub(crate) trait KlipyExt<'klipy_config> {
    fn klipy(&self) -> &Klipy<'klipy_config>;
}

pub(crate) trait GifContextExt<'klipy_config>:
    KlipyExt<'klipy_config> + GifCacheExt
{
    fn gif_context(&self) -> (&Klipy<'klipy_config>, &GifCache);
}

impl<'klipy_config> KlipyExt<'klipy_config> for Context<'_, 'klipy_config> {
    fn klipy(&self) -> &Klipy<'klipy_config> {
        &self.framework().user_data.klipy
    }
}

impl GifCacheExt for Context<'_, '_> {
    fn gif_cache(&self) -> &GifCache {
        &self.framework().user_data.gif_cache
    }
}

impl<'klipy_config> GifContextExt<'klipy_config> for Context<'_, 'klipy_config> {
    fn gif_context(&self) -> (&Klipy<'klipy_config>, &GifCache) {
        let context = self.framework().user_data;
        (&context.klipy, &context.gif_cache)
    }
}

impl<'klipy_config, T> KlipyExt<'klipy_config> for (Klipy<'klipy_config>, T) {
    fn klipy(&self) -> &Klipy<'klipy_config> {
        &self.0
    }
}

impl<T> GifCacheExt for (T, GifCache) {
    fn gif_cache(&self) -> &GifCache {
        &self.1
    }
}

impl<'klipy_config> GifContextExt<'klipy_config> for (Klipy<'klipy_config>, GifCache) {
    fn gif_context(&self) -> (&Klipy<'klipy_config>, &GifCache) {
        (&self.0, &self.1)
    }
}
