use anyhow::Result;
use clap::Parser;

use spectralint::cli::{Cli, Commands};
use spectralint::config::Config;
use spectralint::engine;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            path,
            format,
            config,
            fail_on,
            strict,
        } => {
            let project_root = path.canonicalize().unwrap_or(path);
            let mut cfg = Config::load(config.as_deref(), &project_root)?;
            if strict {
                cfg.strict = true;
            }
            let result = engine::run(&project_root, &cfg)?;

            let output_format = format.unwrap_or(cfg.format);
            spectralint::cli::output::render(&result, &project_root, output_format);

            if result.has_severity_at_least(fail_on) {
                std::process::exit(1);
            }
        }
        Commands::Init => {
            use std::io::Write;
            let path = std::env::current_dir()?.join(".spectralintrc.toml");
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    f.write_all(Config::default_toml().as_bytes())?;
                    println!("Created .spectralintrc.toml");
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    eprintln!(".spectralintrc.toml already exists");
                    std::process::exit(1);
                }
                Err(e) => return Err(e.into()),
            }
        }
        Commands::Explain { rule: None } => {
            println!("{}", spectralint::cli::explain::list_rules());
        }
        Commands::Explain { rule: Some(rule) } => {
            use spectralint::cli::explain::{explain, list_rules};
            match explain(&rule) {
                Some(text) => println!("{text}"),
                None => {
                    eprintln!("Unknown rule: {rule}\n");
                    eprintln!("{}", list_rules());
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
