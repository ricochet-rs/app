//! Phosphor icons embedded in the app, with gpui-component's own icons behind them.

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/phosphor/*.svg"]
struct Phosphor;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = Phosphor::get(path) {
            return Ok(Some(file.data));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut found: Vec<SharedString> = Phosphor::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| p.to_string().into())
            .collect();
        found.extend(gpui_component_assets::Assets.list(path)?);
        Ok(found)
    }
}
