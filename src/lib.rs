mod to_xlsx;

use nu_plugin::{Plugin, PluginCommand};

pub struct XlsxPlugin;

impl Plugin for XlsxPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![Box::new(to_xlsx::ToXlsx)]
    }
}
