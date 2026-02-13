use anyhow::Result;
use spectralint::cli::{Cli, Commands};
use spectralint::config::Config;
use spectralint::engine;

use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            path,
            format,
            config,
            fail_on,
        } => {
            let project_root = path.canonicalize().unwrap_or(path);
            let cfg = Config::load(config.as_deref(), &project_root)?;
            let result = engine::run(&project_root, &cfg)?;

            let output_format = format.unwrap_or(cfg.format);
            spectralint::cli::output::render(&result, &project_root, output_format);

            if result.has_severity_at_least(fail_on) {
                std::process::exit(1);
            }
        }
        Commands::Init => {
            let path = std::env::current_dir()?.join(".spectralintrc.toml");
            if path.exists() {
                eprintln!(".spectralintrc.toml already exists");
                std::process::exit(1);
            }
            std::fs::write(&path, Config::default_toml())?;
            println!("Created .spectralintrc.toml");
        }
    }

    Ok(())
}
