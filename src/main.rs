use anyhow::Result;
use clap::Parser;
use notify::{recommended_watcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use spectralint::cli::{Cli, Commands, OutputFormat, Preset};
use spectralint::config::Config;
use spectralint::engine;
use spectralint::types::Severity;

/// Run a single check pass. Returns true if diagnostics meet the fail_on threshold.
#[allow(clippy::too_many_arguments)]
fn run_check(
    project_root: &Path,
    cfg: &Config,
    config_path: Option<&Path>,
    rule: &[String],
    output_format: OutputFormat,
    quiet: bool,
    count: bool,
    fail_on: Severity,
    use_cache: bool,
    apply_fix: bool,
) -> Result<bool> {
    let mut result = engine::run(project_root, cfg, use_cache, config_path)?;

    // Apply --rule filter
    if !rule.is_empty() {
        use spectralint::types::Category;
        let normalized: Vec<String> = rule.iter().map(|r| r.replace('_', "-")).collect();
        let parsed_categories: Vec<Category> =
            normalized.iter().filter_map(|r| r.parse().ok()).collect();
        let fallback_set: std::collections::HashSet<&str> =
            normalized.iter().map(|s| s.as_str()).collect();
        result.diagnostics.retain(|d| {
            parsed_categories.contains(&d.category)
                || fallback_set.contains(d.category.as_str())
                || fallback_set.contains(&*d.category.to_string())
        });
    }

    // Apply autofixes if --fix is set
    if apply_fix {
        let fixed = engine::apply_fixes(&result.diagnostics);
        if fixed > 0 {
            tracing::info!("Applied {fixed} fix(es).");
        }
    }

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
            spectralint::cli::output::render(&result, project_root, output_format);
        }
    }

    Ok(result.has_severity_at_least(fail_on))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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
            no_cache,
            watch,
            fix,
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

            let output_format = format.unwrap_or(cfg.format);

            let use_cache = !no_cache;

            // First run
            let failed = run_check(
                &project_root,
                &cfg,
                config.as_deref(),
                &rule,
                output_format,
                quiet,
                count,
                fail_on,
                use_cache,
                fix,
            )?;

            if !watch {
                if failed {
                    std::process::exit(1);
                }
                return Ok(());
            }

            // Watch mode: use filesystem notifications
            let (tx, rx) = mpsc::channel();
            let mut watcher = recommended_watcher(tx).expect("Failed to initialize file watcher");
            watcher
                .watch(project_root.as_ref(), RecursiveMode::Recursive)
                .expect("Failed to watch directory");

            loop {
                // Block until we get a filesystem event
                match rx.recv() {
                    Ok(_) => {
                        // Debounce: drain any additional events that arrived
                        std::thread::sleep(Duration::from_millis(100));
                        while rx.try_recv().is_ok() {}

                        println!("\n--- Re-checking... ---\n");
                        match run_check(
                            &project_root,
                            &cfg,
                            config.as_deref(),
                            &rule,
                            output_format,
                            quiet,
                            count,
                            fail_on,
                            use_cache,
                            fix,
                        ) {
                            Ok(_) => {}
                            Err(e) => tracing::error!("Error: {e}"),
                        }
                    }
                    Err(e) => {
                        tracing::error!("Watch error: {e}");
                        break;
                    }
                }
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
        #[cfg(feature = "lsp")]
        Commands::Lsp => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(spectralint::lsp::run_server());
        }
    }

    Ok(())
}
