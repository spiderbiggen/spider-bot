use crate::commands::CommandError;
use crate::{GifCache, SpiderBot};

pub(crate) type Context<'a, 'tenor_config> =
    poise::Context<'a, SpiderBot<'tenor_config>, CommandError>;

pub(crate) trait GifCacheExt {
    fn gif_cache(&self) -> &GifCache;
}

pub(crate) trait TenorExt<'tenor_config> {
    fn tenor(&self) -> &tenor::Client<'tenor_config>;
}

pub(crate) trait GifContextExt<'tenor_config>:
    TenorExt<'tenor_config> + GifCacheExt
{
    fn gif_context(&self) -> (&tenor::Client<'tenor_config>, &GifCache);
}

impl<'tenor_config> TenorExt<'tenor_config> for Context<'_, 'tenor_config> {
    fn tenor(&self) -> &tenor::Client<'tenor_config> {
        &self.framework().user_data.tenor
    }
}

impl GifCacheExt for Context<'_, '_> {
    fn gif_cache(&self) -> &GifCache {
        &self.framework().user_data.gif_cache
    }
}

impl<'tenor_config> GifContextExt<'tenor_config> for Context<'_, 'tenor_config> {
    fn gif_context(&self) -> (&tenor::Client<'tenor_config>, &GifCache) {
        let context = self.framework().user_data;
        (&context.tenor, &context.gif_cache)
    }
}

impl<'tenor_config, T> TenorExt<'tenor_config> for (tenor::Client<'tenor_config>, T) {
    fn tenor(&self) -> &tenor::Client<'tenor_config> {
        &self.0
    }
}

impl<T> GifCacheExt for (T, GifCache) {
    fn gif_cache(&self) -> &GifCache {
        &self.1
    }
}

impl<'tenor_config> GifContextExt<'tenor_config> for (tenor::Client<'tenor_config>, GifCache) {
    fn gif_context(&self) -> (&tenor::Client<'tenor_config>, &GifCache) {
        (&self.0, &self.1)
    }
}
