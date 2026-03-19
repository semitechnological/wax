use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::Cli;

pub fn completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "wax", &mut io::stdout());
}
