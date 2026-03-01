use anyhow::Result;
use clap::Parser;

use spectralint::cli::{Cli, Commands, Preset};
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
            rule,
            quiet,
            no_color,
            count,
        } => {
            // Handle --no-color and NO_COLOR env var
            if no_color || std::env::var("NO_COLOR").is_ok() {
                owo_colors::set_override(false);
            }

            let project_root = path.canonicalize().unwrap_or(path);
            let mut cfg = Config::load(config.as_deref(), &project_root)?;
            if strict {
                cfg.strict = true;
            }
            let mut result = engine::run(&project_root, &cfg)?;

            // Apply per-checker severity overrides
            for d in &mut result.diagnostics {
                if let Some(sev) = cfg.severity_override(&d.category) {
                    d.severity = sev;
                }
            }

            // Apply --rule filter
            if !rule.is_empty() {
                let normalized_rules: std::collections::HashSet<String> =
                    rule.iter().map(|r| r.replace('_', "-")).collect();
                result
                    .diagnostics
                    .retain(|d| normalized_rules.contains(&d.category.to_string()));
            }

            let output_format = format.unwrap_or(cfg.format);
            if !quiet {
                if count {
                    let (e, w, i) = result.severity_counts();
                    if e + w + i == 0 {
                        println!("no issues found");
                    } else {
                        let s = |n: usize| if n == 1 { "" } else { "s" };
                        let parts: Vec<String> = [
                            (e > 0).then(|| format!("{e} error{}", s(e))),
                            (w > 0).then(|| format!("{w} warning{}", s(w))),
                            (i > 0).then(|| format!("{i} info")),
                        ]
                        .into_iter()
                        .flatten()
                        .collect();
                        println!("{}", parts.join(", "));
                    }
                } else {
                    spectralint::cli::output::render(&result, &project_root, output_format);
                }
            }

            if result.has_severity_at_least(fail_on) {
                std::process::exit(1);
            }
        }
        Commands::Init { preset } => {
            use std::io::Write;
            let path = std::env::current_dir()?.join(".spectralintrc.toml");
            let content = match preset {
                Some(Preset::Minimal) => Config::minimal_toml(),
                Some(Preset::Strict) => Config::strict_toml(),
                Some(Preset::Standard) | None => Config::default_toml(),
            };
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    f.write_all(content.as_bytes())?;
                    let label = match preset {
                        Some(Preset::Minimal) => " (minimal preset)",
                        Some(Preset::Strict) => " (strict preset)",
                        _ => "",
                    };
                    println!("Created .spectralintrc.toml{label}");
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
