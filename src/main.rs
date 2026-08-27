mod ast;
mod codegen;
mod compiler;
mod lexer;
mod lsp;
mod parser;
mod stdlib;
mod token;
mod typechecker;
mod wasmgen;

use clap::{Parser as ClapParser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(
    name = "sandbox",
    version = "0.4.0",
    about = "A memory-safe, financially-safe, general-purpose programming language"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile and run a .sbx file
    Run {
        /// Path to .sbx file
        file: PathBuf,
    },
    /// Compile a .sbx file to native binary or WebAssembly
    Build {
        /// Path to .sbx file
        file: PathBuf,
        /// Output binary name
        #[arg(short, long)]
        output: Option<String>,
        /// Target: native (default) or wasm
        #[arg(short, long, default_value = "native")]
        target: String,
    },
    /// Type-check a .sbx file without compiling
    Check {
        /// Path to .sbx file
        file: PathBuf,
    },
    /// Initialize a new Sandbox project
    Init {
        /// Project name
        name: String,
    },
    /// Format .sbx source files
    Fmt {
        /// Path to .sbx file or directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Check formatting without modifying files
        #[arg(long)]
        check: bool,
    },
    /// Add a dependency to sandbox.toml
    Add {
        /// Package name
        package: String,
        /// Version constraint (e.g. "1.0.0", "^1.0")
        #[arg(short, long, default_value = "*")]
        version: String,
    },
    /// Install all dependencies from sandbox.toml
    Install,
    /// Show dependency tree
    Tree,
    /// Generate WebAssembly text format (.wat)
    Wasm {
        /// Path to .sbx file
        file: PathBuf,
        /// Output .wat file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Start the LSP server for IDE support
    Lsp,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let compiler = compiler::Compiler::new(&source, &filename);
            compiler.run()?;
        }
        Commands::Build {
            file,
            output,
            target,
        } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let out_name = output.unwrap_or_else(|| {
                file.file_stem()
                    .map_or("a.out".to_string(), |s| s.to_string_lossy().to_string())
            });
            if target == "wasm" {
                let compiler = compiler::Compiler::new(&source, &filename);
                compiler.build_wasm(&out_name)?;
            } else {
                let compiler = compiler::Compiler::new(&source, &filename);
                compiler.build(&out_name)?;
            }
        }
        Commands::Check { file } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let compiler = compiler::Compiler::new(&source, &filename);
            compiler.check()?;
        }
        Commands::Init { name } => {
            init_project(&name)?;
        }
        Commands::Fmt { path, check } => {
            run_fmt(&path, check)?;
        }
        Commands::Add { package, version } => {
            add_dependency(&package, &version)?;
        }
        Commands::Install => {
            install_dependencies()?;
        }
        Commands::Tree => {
            show_tree()?;
        }
        Commands::Wasm { file, output } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let out_name = output.unwrap_or_else(|| {
                file.file_stem().map_or("output.wat".to_string(), |s| {
                    format!("{}.wat", s.to_string_lossy())
                })
            });
            let compiler = compiler::Compiler::new(&source, &filename);
            compiler.wasm(&out_name)?;
        }
        Commands::Lsp => {
            lsp::run_lsp()?;
        }
    }

    Ok(())
}

fn init_project(name: &str) -> anyhow::Result<()> {
    println!("🔧 Initializing project '{}'", name);

    fs::create_dir_all(name)?;

    let toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
description = "A Sandbox project"

[dependencies]
"#,
        name
    );
    fs::write(format!("{}/sandbox.toml", name), toml)?;

    let main_sbx = format!(
        r#"// {} - main.sbx

fn main() {{
    print("Hello, {}!")
}}
"#,
        name, name
    );
    fs::write(format!("{}/main.sbx", name), main_sbx)?;

    fs::create_dir_all(format!("{}/src", name))?;

    println!("✅ Project '{}' created!", name);
    println!();
    println!("  cd {}", name);
    println!("  sandbox run main.sbx");

    Ok(())
}

// ── Format ──

fn run_fmt(path: &PathBuf, check_only: bool) -> anyhow::Result<()> {
    if path.is_dir() {
        let mut files = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "sbx") {
                files.push(p);
            }
        }
        files.sort();

        let mut all_ok = true;
        for f in &files {
            let ok = fmt_single_file(f, check_only)?;
            if !ok {
                all_ok = false;
            }
        }

        if check_only && !all_ok {
            println!("❌ Some files need formatting");
            std::process::exit(1);
        } else if check_only {
            println!("✅ All files formatted correctly");
        }
    } else {
        let ok = fmt_single_file(path, check_only)?;
        if check_only && !ok {
            std::process::exit(1);
        }
    }

    Ok(())
}

fn fmt_single_file(path: &PathBuf, check_only: bool) -> anyhow::Result<bool> {
    let source = fs::read_to_string(path)?;
    let formatted = simple_fmt(&source);

    if source == formatted {
        if !check_only {
            println!("  ✓ {}", path.display());
        }
        return Ok(true);
    }

    if check_only {
        println!("  ✗ {} needs formatting", path.display());
        return Ok(false);
    }

    fs::write(path, &formatted)?;
    println!("  ✓ {} (formatted)", path.display());
    Ok(true)
}

fn simple_fmt(source: &str) -> String {
    let mut output = String::new();
    let mut indent: usize = 0;
    let mut prev_blank = false;

    for line in source.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if !prev_blank {
                output.push('\n');
                prev_blank = true;
            }
            continue;
        }
        prev_blank = false;

        if trimmed.starts_with('}') || trimmed.starts_with(']') {
            indent = indent.saturating_sub(1);
        }

        for _ in 0..indent {
            output.push_str("    ");
        }
        output.push_str(trimmed);
        output.push('\n');

        if trimmed.ends_with('{') || trimmed.ends_with('[') {
            indent += 1;
        }
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }

    output
}

// ── Package Manager ──

#[derive(Debug, Clone, serde::Deserialize)]
struct SandboxToml {
    #[serde(default)]
    package: PackageInfo,
    #[serde(default)]
    dependencies: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct PackageInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    #[allow(dead_code)]
    authors: Vec<String>,
}

fn find_sandbox_toml() -> anyhow::Result<String> {
    let content = fs::read_to_string("sandbox.toml")
        .map_err(|_| anyhow::anyhow!("No sandbox.toml found. Run 'sandbox init' first."))?;
    Ok(content)
}

fn parse_sandbox_toml(content: &str) -> anyhow::Result<SandboxToml> {
    let config: SandboxToml = toml::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse sandbox.toml: {}", e))?;
    Ok(config)
}

fn add_dependency(name: &str, version: &str) -> anyhow::Result<()> {
    let mut content = find_sandbox_toml()?;
    let mut config = parse_sandbox_toml(&content)?;

    if config.dependencies.contains_key(name) {
        println!("📦 '{}' already in dependencies (updating)", name);
    }

    config
        .dependencies
        .insert(name.to_string(), version.to_string());

    content = rebuild_toml(&config);
    fs::write("sandbox.toml", &content)?;

    println!("✅ Added '{}' v{}", name, version);
    Ok(())
}

fn rebuild_toml(config: &SandboxToml) -> String {
    let mut out = String::new();

    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{}\"\n", config.package.name));
    out.push_str(&format!("version = \"{}\"\n", config.package.version));
    if !config.package.description.is_empty() {
        out.push_str(&format!(
            "description = \"{}\"\n",
            config.package.description
        ));
    }
    out.push('\n');

    out.push_str("[dependencies]\n");
    if config.dependencies.is_empty() {
        out.push_str("# no dependencies yet\n");
    } else {
        let mut deps: Vec<_> = config.dependencies.iter().collect();
        deps.sort_by_key(|(k, _)| (*k).clone());
        for (name, version) in deps {
            out.push_str(&format!("{} = \"{}\"\n", name, version));
        }
    }

    out
}

fn install_dependencies() -> anyhow::Result<()> {
    let content = find_sandbox_toml()?;
    let config = parse_sandbox_toml(&content)?;

    if config.dependencies.is_empty() {
        println!("📦 No dependencies to install");
        return Ok(());
    }

    println!("📦 Installing dependencies...");
    for (name, version) in &config.dependencies {
        println!("  → {} v{}", name, version);
        println!("  ✓ {} (placeholder — registry coming in future)", name);
    }

    println!("✅ All dependencies installed");
    Ok(())
}

fn show_tree() -> anyhow::Result<()> {
    let content = find_sandbox_toml()?;
    let config = parse_sandbox_toml(&content)?;

    println!("📦 {} v{}", config.package.name, config.package.version);

    if config.dependencies.is_empty() {
        println!("  (no dependencies)");
    } else {
        let mut deps: Vec<_> = config.dependencies.iter().collect();
        deps.sort_by_key(|(k, _)| (*k).clone());
        for (i, (name, version)) in deps.iter().enumerate() {
            let prefix = if i == deps.len() - 1 {
                "└──"
            } else {
                "├──"
            };
            println!("  {} {} v{}", prefix, name, version);
        }
    }

    Ok(())
}
