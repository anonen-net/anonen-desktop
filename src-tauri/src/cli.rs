use clap::Parser;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "anonen", about = "あのねん (Anonen) - Speech to Text")]
pub struct CliArgs {
    #[arg(long)]
    pub start_hidden: bool,

    #[arg(long)]
    pub no_tray: bool,

    #[arg(long)]
    pub toggle_transcription: bool,

    #[arg(long)]
    pub toggle_post_process: bool,

    #[arg(long)]
    pub cancel: bool,

    #[arg(long)]
    pub debug: bool,
}
