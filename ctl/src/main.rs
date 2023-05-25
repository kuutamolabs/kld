mod system_info;
use system_info::system_info;

use clap::Parser;

#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
enum Command {
    SystemInfo(SystemInfoArgs),
}

#[derive(clap::Args)]
/// Show system info
struct SystemInfoArgs {
    /// Display system info with inline format
    #[clap(long)]
    inline: bool,
}

fn main() {
    match Command::parse() {
        Command::SystemInfo(arg) => system_info(arg.inline),
    }
}
