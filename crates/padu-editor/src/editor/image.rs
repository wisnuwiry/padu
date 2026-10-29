//! Image management, drop handling, and image store integration.

use std::path::Path;

use gpui::{App, Entity, Global};

use crate::editor::Editor;
use crate::model::is_image;

/// Source of an image payload: bytes (e.g. pasted screenshot) or file on disk.
pub enum Source<'a> {
    Bytes(&'a [u8]),
    File(&'a Path),
}

/// Handler for persisting and accepting images.
#[derive(Clone, Copy)]
pub struct ImageStore {
    pub keep: fn(
        source: Source,
        editor: &Entity<Editor>,
        base: Option<&Path>,
        cx: &App,
    ) -> Option<String>,
    pub accepts: fn(path: &Path) -> bool,
}

fn default_accepts(path: &Path) -> bool {
    path.to_str().is_some_and(is_image)
}

fn default_keep(
    source: Source,
    _editor: &Entity<Editor>,
    _base: Option<&Path>,
    _cx: &App,
) -> Option<String> {
    match source {
        Source::File(path) => Some(path.to_string_lossy().to_string()),
        Source::Bytes(_) => None,
    }
}

impl Default for ImageStore {
    fn default() -> Self {
        Self {
            keep: default_keep,
            accepts: default_accepts,
        }
    }
}

impl ImageStore {
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }
}

struct Installed(ImageStore);
impl Global for Installed {}

/// Register the global image store for saving and locating images.
pub fn set_image_store(cx: &mut App, store: ImageStore) {
    cx.set_global(Installed(store));
}
