//! `soundcraft-cli`: SoundCraft without a window.
//!
//! ```text
//! soundcraft-cli info FILE                       audio file or .scraft summary
//! soundcraft-cli convert IN OUT [--bit-depth 16|24|32f] [--rate HZ] [--normalize] [--dither | --no-dither]
//!                                                 dithers only when the output loses resolution, unless told
//! soundcraft-cli run [--in S.scraft | --demo] [--cmd ID[=JSON]]… [--bounce OUT] [--start T --end T] [--save S.scraft] [--inspect]
//! soundcraft-cli script FILE [--in S | --demo] [--save S] [--bounce OUT [--start T --end T]]   (one `ID JSON` per line;
//!                                                 without --start/--end, --bounce covers the whole session)
//! soundcraft-cli commands [FILTER]               list commands
//! soundcraft-cli describe ID                     one command's details
//! soundcraft-cli app [--port P] METHOD [JSON]     call a running app's control channel
//! soundcraft-cli mcp [--connect PORT] [--demo] [--automation-read-root DIR]... [--automation-write-root DIR]...
//!                                                 MCP server on stdio. The roots confine every file the
//!                                                 agent's commands read (open, import, presets) and write
//!                                                 (save, bounce, export) to those folders, links followed;
//!                                                 each flag may be given more than once, a path inside any
//!                                                 of its folders being allowed, and a kind left out grants
//!                                                 none of its access. They run the in-process server, so
//!                                                 they conflict with --connect.
//! soundcraft-cli parity [--write PATH]            parity report (markdown)
//! soundcraft-cli plugins                          list built-in plugins
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

fn main() {
    soundcraft_cli::logger::install();
    // `args_os`, not `args`: a non-Unicode argument must reach clap, not panic before anything is printed.
    let code = soundcraft_cli::run(std::env::args_os().skip(1), &mut std::io::stdout(), &mut std::io::stderr());
    // The subcommand's engine and its plugin instances are gone by now. This is process teardown,
    // so it lives here and not in `run`, which tests call many times in one process.
    soundcraft_engine::shutdown_plugin_hosts();
    std::process::exit(code);
}
