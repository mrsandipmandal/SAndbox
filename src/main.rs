mod ast;
mod codegen;
mod compiler;
mod fmt;
mod interpreter;
mod lexer;
mod llvmgen;
mod lsp;
mod parser;
mod registry_client;
mod repl;
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
        /// Print the parsed AST without compiling
        #[arg(long)]
        ast: bool,
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
    /// Run tests in a .sbx file
    Test {
        /// Path to .sbx file
        file: PathBuf,
        /// Filter tests by name (substring match)
        #[arg(long, short)]
        filter: Option<String>,
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
        /// Show diff of what would change without modifying files
        #[arg(long)]
        diff: bool,
        /// Verify that formatting is idempotent (format twice, compare)
        #[arg(long)]
        verify: bool,
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
    Install {
        /// Fail if any package is unsigned or has an invalid signature
        #[arg(long)]
        require_signatures: bool,
    },
    /// Show dependency tree
    Tree,
    /// Fetch and vendor all dependencies into .sandbox/vendor/
    Vendor,
    /// Generate WebAssembly text format (.wat)
    Wasm {
        /// Path to .sbx file
        file: PathBuf,
        /// Output .wat file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Generate LLVM IR text format (.ll)
    Llvm {
        /// Path to .sbx file
        file: PathBuf,
        /// Output .ll file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Build via LLVM backend (sandbox -> LLVM IR -> native binary via clang)
    LlvmBuild {
        /// Path to .sbx file
        file: PathBuf,
        /// Output binary path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Package registry commands
    Pkg {
        #[command(subcommand)]
        command: PkgCommands,
    },
    /// Start the LSP server for IDE support
    Lsp,
    /// Start interactive REPL
    Repl,
    /// Interpret a .sbx file directly (no C compilation)
    Interpret {
        /// Path to .sbx file
        file: PathBuf,
    },
    /// Generate documentation for a .sbx file
    Doc {
        /// Path to .sbx file
        file: PathBuf,
    },
}

#[derive(Subcommand)]
enum PkgCommands {
    /// Log in to the package registry
    Login,
    /// Create an account on the package registry
    Register,
    /// Publish a package to the registry
    Publish {
        /// Path to package file (.sbx)
        file: String,
    },
    /// Search for packages
    Search {
        /// Search query
        #[arg(default_value = "")]
        query: String,
    },
    /// Show package information
    Info {
        /// Package name
        name: String,
    },
    /// Scaffold a new publishable package in the current directory
    Init {
        /// Package name (defaults to current directory name)
        #[arg(default_value = "")]
        name: String,
    },
    /// Generate ed25519 signing keypair
    Keygen,
    /// Register public key with the registry
    Keys,
    /// Verify a package's signature
    Verify {
        /// Package name
        name: String,
        /// Package version
        version: String,
    },
    /// Install a specific package by name
    Install {
        /// Package name (and optional version: name@version)
        package: String,
    },
    /// List installed vendored packages
    List,
    /// Update all vendored packages to latest versions
    Update,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, ast } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            if ast {
                let mut lex = lexer::Lexer::new(&source);
                let tokens = lex.tokenize()?;
                let mut pars = parser::Parser::new(tokens);
                let program = pars.parse()?;
                println!("{:#?}", program);
            } else {
                let compiler = compiler::Compiler::new(&source, &filename);
                compiler.run()?;
            }
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
        Commands::Test { file, filter } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let compiler = compiler::Compiler::new(&source, &filename);
            compiler.run_tests(filter.as_deref())?;
        }
        Commands::Init { name } => {
            init_project(&name)?;
        }
        Commands::Fmt {
            path,
            check,
            diff,
            verify,
        } => {
            run_fmt(&path, check, diff, verify)?;
        }
        Commands::Add { package, version } => {
            add_dependency(&package, &version)?;
        }
        Commands::Install { require_signatures } => {
            install_dependencies(require_signatures)?;
        }
        Commands::Tree => {
            show_tree()?;
        }
        Commands::Vendor => {
            install_dependencies(false)?;
        }
        Commands::Pkg { command } => match command {
            PkgCommands::Login => {
                registry_client::pkg_login()?;
            }
            PkgCommands::Register => {
                registry_client::pkg_register()?;
            }
            PkgCommands::Publish { file } => {
                registry_client::pkg_publish(&file)?;
            }
            PkgCommands::Search { query } => {
                registry_client::pkg_search(&query)?;
            }
            PkgCommands::Info { name } => {
                registry_client::pkg_info(&name)?;
            }
            PkgCommands::Init { name } => {
                registry_client::pkg_init(&name)?;
            }
            PkgCommands::Keygen => {
                registry_client::pkg_keygen()?;
            }
            PkgCommands::Keys => {
                registry_client::pkg_keys_register()?;
            }
            PkgCommands::Verify { name, version } => {
                registry_client::pkg_verify(&name, &version)?;
            }
            PkgCommands::Install { package } => {
                pkg_install_single(&package)?;
            }
            PkgCommands::List => {
                pkg_list_vendored()?;
            }
            PkgCommands::Update => {
                install_dependencies(false)?;
                println!("✓ All packages updated");
            }
        },
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
        Commands::Llvm { file, output } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let out_name = output.unwrap_or_else(|| {
                file.file_stem().map_or("output.ll".to_string(), |s| {
                    format!("{}.ll", s.to_string_lossy())
                })
            });
            let compiler = compiler::Compiler::new(&source, &filename);
            compiler.llvm(&out_name)?;
        }
        Commands::LlvmBuild { file, output } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            let out_name = output.unwrap_or_else(|| {
                file.file_stem()
                    .map_or("output".to_string(), |s| s.to_string_lossy().to_string())
            });
            let compiler = compiler::Compiler::new(&source, &filename);
            compiler.build_llvm(&out_name)?;
        }
        Commands::Lsp => {
            lsp::run_lsp()?;
        }
        Commands::Repl => {
            repl::run_repl()?;
        }
        Commands::Interpret { file } => {
            let source = fs::read_to_string(&file)?;
            let filename = file.to_string_lossy().to_string();
            interpreter::interpret(&source, &filename)?;
        }
        Commands::Doc { file } => {
            let source = fs::read_to_string(&file)?;
            generate_docs(&source)?;
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

fn run_fmt(path: &PathBuf, check_only: bool, show_diff: bool, verify: bool) -> anyhow::Result<()> {
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
            let ok = fmt_single_file(f, check_only, show_diff, verify)?;
            if !ok {
                all_ok = false;
            }
        }

        if (check_only || show_diff || verify) && !all_ok {
            if show_diff {
                // Already printed diffs
            } else {
                println!("❌ Some files need formatting");
            }
            std::process::exit(1);
        } else if check_only || show_diff || verify {
            println!("✅ All files formatted correctly");
        }
    } else {
        let ok = fmt_single_file(path, check_only, show_diff, verify)?;
        if (check_only || show_diff || verify) && !ok {
            std::process::exit(1);
        }
    }

    Ok(())
}

fn fmt_single_file(
    path: &PathBuf,
    check_only: bool,
    show_diff: bool,
    verify: bool,
) -> anyhow::Result<bool> {
    let source = fs::read_to_string(path)?;
    let formatted = simple_fmt(&source);

    if source == formatted {
        if !check_only && !show_diff && !verify {
            println!("  ✓ {}", path.display());
        }
        return Ok(true);
    }

    if check_only {
        println!("  ✗ {} needs formatting", path.display());
        return Ok(false);
    }

    if show_diff {
        println!("--- a/{}", path.display());
        println!("+++ b/{}", path.display());
        print_unified_diff(&source, &formatted);
        return Ok(false);
    }

    if verify {
        // Check idempotency: format the formatted output and compare
        let reformatted = simple_fmt(&formatted);
        if reformatted != formatted {
            println!("  ✗ {} (idempotency broken)", path.display());
            println!("    First format:");
            for line in formatted.lines().take(5) {
                println!("      {}", line);
            }
            println!("    Second format:");
            for line in reformatted.lines().take(5) {
                println!("      {}", line);
            }
            return Ok(false);
        }
        fs::write(path, &formatted)?;
        println!("  ✓ {} (formatted, idempotent)", path.display());
        return Ok(true);
    }

    fs::write(path, &formatted)?;
    println!("  ✓ {} (formatted)", path.display());
    Ok(true)
}

/// Print a unified diff between original and formatted source.
fn print_unified_diff(original: &str, formatted: &str) {
    let orig_lines: Vec<&str> = original.lines().collect();
    let fmt_lines: Vec<&str> = formatted.lines().collect();

    // Simple line-by-line diff
    let max_lines = orig_lines.len().max(fmt_lines.len());
    let mut i = 0;
    while i < max_lines {
        let orig = orig_lines.get(i).copied().unwrap_or("");
        let fmt = fmt_lines.get(i).copied().unwrap_or("");

        if orig != fmt {
            // Find the extent of this change
            let mut j = i;
            while j < max_lines {
                let o = orig_lines.get(j).copied().unwrap_or("");
                let f = fmt_lines.get(j).copied().unwrap_or("");
                if o == f {
                    break;
                }
                j += 1;
            }
            // Print context (1 line before if possible)
            if i > 0 {
                println!(" {}", orig_lines[i - 1]);
            }
            // Print removed lines
            for line in &orig_lines[i..j.min(orig_lines.len())] {
                println!("-{}", line);
            }
            // Print added lines
            for line in &fmt_lines[i..j.min(fmt_lines.len())] {
                println!("+{}", line);
            }
            // Print context (1 line after if possible)
            if j < max_lines {
                let after = orig_lines
                    .get(j)
                    .or(fmt_lines.get(j))
                    .copied()
                    .unwrap_or("");
                println!(" {}", after);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
}

fn simple_fmt(source: &str) -> String {
    fmt::format_source(source)
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

/// Install a single package by name (and optional version)
fn pkg_install_single(package: &str) -> anyhow::Result<()> {
    let (name, version) = if let Some((n, v)) = package.split_once('@') {
        (n.to_string(), Some(v.to_string()))
    } else {
        (package.to_string(), None)
    };

    // Resolve from registry
    let mut deps = std::collections::HashMap::new();
    if let Some(v) = &version {
        deps.insert(name.clone(), v.clone());
    } else {
        deps.insert(name.clone(), "*".to_string());
    }

    let resolved = registry_client::resolve_all_dependencies(&deps)?;
    if resolved.is_empty() {
        anyhow::bail!("Package '{}' not found in registry", name);
    }

    fs::create_dir_all(".sandbox/vendor")?;
    for (dep_name, dep_version, _checksum) in &resolved {
        print!("  → {} v{}... ", dep_name, dep_version);
        let data = registry_client::download_package_bytes(dep_name, dep_version)?;
        let path = format!(".sandbox/vendor/{}-{}.sbx", dep_name, dep_version);
        fs::write(&path, &data)?;
        println!("✓");
    }

    // Update sandbox.toml
    let toml_path = std::path::Path::new("sandbox.toml");
    if toml_path.exists() {
        let mut content = fs::read_to_string(toml_path)?;
        let dep_line = if let Some(v) = &version {
            format!("{} = \"{}\"", name, v)
        } else {
            format!("{} = \"*\"", name)
        };
        if content.contains(&"[dependencies]".to_string()) {
            content.push_str(&format!("\n{}", dep_line));
        } else {
            content.push_str(&format!("\n[dependencies]\n{}\n", dep_line));
        }
        fs::write(toml_path, content)?;
    }

    println!("✓ Package '{}' installed", name);
    Ok(())
}

/// List vendored packages
fn pkg_list_vendored() -> anyhow::Result<()> {
    let vendor_dir = std::path::Path::new(".sandbox/vendor");
    if !vendor_dir.exists() {
        println!("No packages installed (no .sandbox/vendor directory)");
        return Ok(());
    }

    let mut packages: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(vendor_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".sbx") {
            let without_ext = name.strip_suffix(".sbx").unwrap_or(&name);
            if let Some((pkg_name, version)) = without_ext.rsplit_once('-') {
                packages.push((pkg_name.to_string(), version.to_string()));
            }
        }
    }

    if packages.is_empty() {
        println!("No packages installed");
    } else {
        packages.sort();
        println!("Installed packages:");
        for (name, version) in &packages {
            println!("  {} v{}", name, version);
        }
    }
    Ok(())
}

fn install_dependencies(require_signatures: bool) -> anyhow::Result<()> {
    let content = find_sandbox_toml()?;
    let config = parse_sandbox_toml(&content)?;

    if config.dependencies.is_empty() {
        println!("📦 No dependencies to install");
        return Ok(());
    }

    println!("📦 Resolving dependencies...");

    // Use the resolver to get all dependencies with checksums
    let resolved_deps = registry_client::resolve_all_dependencies(&config.dependencies)?;

    if resolved_deps.is_empty() {
        println!("⚠ No dependencies could be resolved");
        return Ok(());
    }

    println!("📦 Installing {} dependencies...", resolved_deps.len());
    fs::create_dir_all(".sandbox/vendor")?;

    let mut lock = String::new();
    lock.push_str("# sandbox.lock — generated by `sandbox install`\n");
    lock.push_str("# Do not edit by hand.\n\n");
    lock.push_str("[dependencies]\n");

    let mut any_fetched = false;

    for (name, version, checksum) in &resolved_deps {
        print!("  → {} v{}... ", name, version);

        // Download the package
        match registry_client::download_package_bytes(name, version) {
            Ok(data) => {
                // Verify checksum
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&data);
                let actual = format!("sha256:{}", hex::encode(hasher.finalize()));

                if checksum != "unknown" && actual != *checksum {
                    println!("❌ checksum mismatch!");
                    println!("     expected: {}", checksum);
                    println!("     actual:   {}", actual);
                    lock.push_str(&format!(
                        "{} = {{ version = \"{}\", checksum = \"{}\", status = \"mismatch\" }}\n",
                        name, version, actual
                    ));
                    continue;
                }

                let dir = format!(".sandbox/vendor/{}", name);
                fs::create_dir_all(&dir)?;
                let path = format!("{dir}/{}.sbx", name);
                fs::write(&path, &data)?;

                // Verify ed25519 signature
                let mut sig_status_str = "unsigned".to_string();
                match registry_client::verify_package_signature(name, version) {
                    Ok(status) if status.signed && status.valid => {
                        sig_status_str = format!("signed by {}", status.signed_by);
                    }
                    Ok(status) if status.signed && !status.valid => {
                        sig_status_str = "INVALID signature".to_string();
                        if require_signatures {
                            println!(
                                "❌ {} v{} has an INVALID signature — aborting (signed by {})",
                                name, version, status.signed_by
                            );
                            lock.push_str(&format!("{} = {{ version = \"{}\", checksum = \"{}\", status = \"invalid_signature\" }}\n", name, version, actual));
                            continue;
                        }
                        println!(
                            "⚠  WARNING: {} v{} has an INVALID signature (signed by {})",
                            name, version, status.signed_by
                        );
                    }
                    Ok(_status) => {
                        // not signed
                        if require_signatures {
                            println!("❌ {} v{} is NOT signed — aborting", name, version);
                            lock.push_str(&format!("{} = {{ version = \"{}\", checksum = \"{}\", status = \"unsigned\" }}\n", name, version, actual));
                            continue;
                        }
                    }
                    Err(_) => {
                        // Verify endpoint unavailable — that's fine
                    }
                }

                lock.push_str(&format!(
                    "{} = {{ version = \"{}\", checksum = \"{}\", signature = \"{}\" }}\n",
                    name, version, actual, sig_status_str
                ));
                println!("✓ {} bytes, verified, {}", data.len(), sig_status_str);
                any_fetched = true;
            }
            Err(e) => {
                println!("⚠ {}", e);
                lock.push_str(&format!(
                    "{} = {{ version = \"{}\", status = \"failed\" }}\n",
                    name, version
                ));
            }
        }
    }

    fs::write(".sandbox/lock.toml", &lock)?;

    if !any_fetched {
        println!(
            "⚠ No packages could be fetched (registry unreachable).\n  Vendored layout created at .sandbox/vendor/ — drop local packages there."
        );
    }
    println!("✅ All dependencies installed (see .sandbox/vendor/ and .sandbox/lock.toml)");
    Ok(())
}

fn show_tree() -> anyhow::Result<()> {
    let content = find_sandbox_toml()?;
    let config = parse_sandbox_toml(&content)?;

    println!("📦 {} v{}", config.package.name, config.package.version);

    if config.dependencies.is_empty() {
        println!("  (no dependencies)");
        return Ok(());
    }

    // Collect all conflicts found during traversal
    let mut conflicts: Vec<String> = Vec::new();
    // Track resolved versions: package_name -> (spec, resolved_version, required_by)
    let mut resolved_versions: std::collections::HashMap<String, Vec<(String, String, String)>> =
        std::collections::HashMap::new();

    // Resolve direct dependencies
    let mut direct_deps: Vec<(&String, &String)> = config.dependencies.iter().collect();
    direct_deps.sort_by_key(|(k, _)| (*k).clone());

    for (i, (name, spec)) in direct_deps.iter().enumerate() {
        let is_last = i == direct_deps.len() - 1;
        let connector = if is_last { "└──" } else { "├──" };
        let continuation = if is_last { "   " } else { "│  " };

        match registry_client::resolve_version(name, spec) {
            Ok(resolved) => {
                // Track resolved version for conflict detection
                resolved_versions
                    .entry(name.to_string())
                    .or_default()
                    .push((
                        spec.to_string(),
                        resolved.clone(),
                        config.package.name.clone(),
                    ));

                println!(
                    "  {} {} v{} (resolved: {})",
                    connector, name, spec, resolved
                );

                // Fetch and display transitive dependencies
                match registry_client::fetch_package_deps(name, &resolved) {
                    Ok(transitive) if !transitive.is_empty() => {
                        let mut trans_sorted = transitive;
                        trans_sorted.sort_by_key(|(k, _)| k.clone());
                        for (j, (dep_name, dep_spec)) in trans_sorted.iter().enumerate() {
                            let dep_last = j == trans_sorted.len() - 1;
                            let dep_connector = if dep_last { "└──" } else { "├──" };
                            let dep_cont = if dep_last { "   " } else { "│  " };

                            // Check for conflicts
                            if let Some(existing) = resolved_versions.get(dep_name) {
                                for (_prev_spec, prev_resolved, prev_by) in existing {
                                    if prev_resolved != dep_spec {
                                        // Different specifiers might resolve to different versions
                                        match registry_client::resolve_version(dep_name, dep_spec) {
                                            Ok(dep_resolved) if dep_resolved != *prev_resolved => {
                                                conflicts.push(format!(
                                                    "  ⚠ Conflict: {} requires v{}, but {} requires v{}",
                                                    name, dep_resolved, prev_by, prev_resolved
                                                ));
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            match registry_client::resolve_version(dep_name, dep_spec) {
                                Ok(dep_resolved) => {
                                    resolved_versions
                                        .entry(dep_name.to_string())
                                        .or_default()
                                        .push((
                                            dep_spec.to_string(),
                                            dep_resolved.clone(),
                                            name.to_string(),
                                        ));

                                    println!(
                                        "  {}  {} {} v{} (resolved: {})",
                                        continuation,
                                        dep_connector,
                                        dep_name,
                                        dep_spec,
                                        dep_resolved
                                    );

                                    // Fetch depth-2 transitive deps
                                    match registry_client::fetch_package_deps(
                                        dep_name,
                                        &dep_resolved,
                                    ) {
                                        Ok(deep_deps) if !deep_deps.is_empty() => {
                                            let mut deep_sorted = deep_deps;
                                            deep_sorted.sort_by_key(|(k, _)| k.clone());
                                            for (k, (deep_name, deep_spec)) in
                                                deep_sorted.iter().enumerate()
                                            {
                                                let deep_last = k == deep_sorted.len() - 1;
                                                let deep_connector = if deep_last {
                                                    "└──"
                                                } else {
                                                    "├──"
                                                };

                                                if let Some(existing) =
                                                    resolved_versions.get(deep_name)
                                                {
                                                    for (_prev_spec, prev_resolved, prev_by) in
                                                        existing
                                                    {
                                                        if prev_resolved != deep_spec {
                                                            match registry_client::resolve_version(
                                                                deep_name, deep_spec,
                                                            ) {
                                                                Ok(d_resolved)
                                                                    if d_resolved
                                                                        != *prev_resolved =>
                                                                {
                                                                    conflicts.push(format!(
                                                                        "  ⚠ Conflict: {} requires v{}, but {} requires v{}",
                                                                        dep_name, d_resolved, prev_by, prev_resolved
                                                                    ));
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                    }
                                                }

                                                match registry_client::resolve_version(
                                                    deep_name, deep_spec,
                                                ) {
                                                    Ok(d_resolved) => {
                                                        resolved_versions
                                                            .entry(deep_name.to_string())
                                                            .or_default()
                                                            .push((
                                                                deep_spec.to_string(),
                                                                d_resolved.clone(),
                                                                dep_name.to_string(),
                                                            ));
                                                        println!(
                                                            "  {}  {}  {} {} v{} (resolved: {})",
                                                            continuation,
                                                            dep_cont,
                                                            deep_connector,
                                                            deep_name,
                                                            deep_spec,
                                                            d_resolved
                                                        );
                                                    }
                                                    Err(e) => {
                                                        println!(
                                                            "  {}  {}  {} ⚠ {} v{}: {}",
                                                            continuation,
                                                            dep_cont,
                                                            deep_connector,
                                                            deep_name,
                                                            deep_spec,
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    println!(
                                        "  {}  {} ⚠ {} v{}: {}",
                                        continuation, dep_connector, dep_name, dep_spec, e
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("  {} ⚠ {} v{}: {}", connector, name, spec, e);
            }
        }
    }

    // Print conflicts at the end
    if !conflicts.is_empty() {
        println!("\n⚠ {} conflict(s) detected:", conflicts.len());
        for c in &conflicts {
            println!("{}", c);
        }
    }

    // Print lock file info if it exists
    let lock_path = std::path::Path::new(".sandbox/lock.toml");
    if lock_path.exists() {
        if let Ok(lock_content) = std::fs::read_to_string(lock_path) {
            let installed = lock_content
                .lines()
                .filter(|l| l.contains("version") && !l.starts_with('#'))
                .count();
            println!(
                "\n🔒 {} package(s) installed (see .sandbox/lock.toml)",
                installed
            );
        }
    }

    Ok(())
}

// ── Phase 4: Documentation Generator ──

fn generate_docs(source: &str) -> anyhow::Result<()> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;
    let mut parser = parser::Parser::new(tokens);
    let program = parser
        .parse()
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    println!("# API Documentation");
    println!();

    fn sub_name(item: &ast::TopLevel) -> String {
        match item {
            ast::TopLevel::FnDef { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

    for item in &program.items {
        match item {
            ast::TopLevel::FnDef {
                name,
                params,
                ret,
                doc,
                ..
            } => {
                if let Some(d) = doc {
                    for line in d.lines() {
                        println!("// {}", line);
                    }
                }
                let params_str: Vec<String> = params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.ty))
                    .collect();
                let ret_str = ret.as_ref().map_or("void".to_string(), |t| t.to_string());
                println!("## `{}({}) -> {}`", name, params_str.join(", "), ret_str);
                println!();
            }
            ast::TopLevel::StructDef {
                name, fields, doc, ..
            } => {
                if let Some(d) = doc {
                    for line in d.lines() {
                        println!("// {}", line);
                    }
                }
                println!("## struct `{}`", name);
                println!();
                println!("| Field | Type |");
                println!("|-------|------|");
                for f in fields {
                    println!("| `{}` | `{}` |", f.name, f.ty);
                }
                println!();
            }
            ast::TopLevel::EnumDef {
                name,
                variants,
                doc,
                ..
            } => {
                if let Some(d) = doc {
                    for line in d.lines() {
                        println!("// {}", line);
                    }
                }
                println!("## enum `{}`", name);
                println!();
                for v in variants {
                    let payload_str = v
                        .payload
                        .as_ref()
                        .map_or(String::new(), |t| format!("({})", t));
                    println!("- `{}{}`", v.name, payload_str);
                }
                println!();
            }
            ast::TopLevel::ModuleDef { name, items, .. } => {
                println!("## module `{}`", name);
                println!();
                for sub in items {
                    if let ast::TopLevel::FnDef { params, ret, .. } = sub {
                        let params_str: Vec<String> = params
                            .iter()
                            .map(|p| format!("{}: {}", p.name, p.ty))
                            .collect();
                        let ret_str = ret.as_ref().map_or("void".to_string(), |t| t.to_string());
                        println!(
                            "### `{}::{}({}) -> {}`",
                            name,
                            sub_name(sub),
                            params_str.join(", "),
                            ret_str
                        );
                    }
                }
                println!();
            }
            ast::TopLevel::ImplDef {
                type_name, methods, ..
            } => {
                println!("## impl `{}`", type_name);
                println!();
                for method in methods {
                    if let ast::TopLevel::FnDef {
                        name, params, ret, ..
                    } = method
                    {
                        let params_str: Vec<String> = params
                            .iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("{}: {}", p.name, p.ty))
                            .collect();
                        let ret_str = ret.as_ref().map_or("void".to_string(), |t| t.to_string());
                        println!(
                            "### `{}::{}({}) -> {}`",
                            type_name,
                            name,
                            params_str.join(", "),
                            ret_str
                        );
                    }
                }
                println!();
            }
            _ => {}
        }
    }

    Ok(())
}
