use nu_plugin::{serve_plugin, MsgPackSerializer};
use nu_plugin_xlsx::XlsxPlugin;

fn main() {
    serve_plugin(&XlsxPlugin, MsgPackSerializer);
}
