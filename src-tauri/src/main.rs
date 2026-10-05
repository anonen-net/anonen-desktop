#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anonen_app_lib::CliArgs;
use clap::Parser;

fn main() {
    let cli_args = CliArgs::parse();

    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    anonen_app_lib::run(cli_args)
}
