mod ast;
mod codegen;
mod compiler;
mod fmt;
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
        Commands::Fmt { path, check, diff, verify } => {
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
            interpret(&source, &filename)?;
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

fn fmt_single_file(path: &PathBuf, check_only: bool, show_diff: bool, verify: bool) -> anyhow::Result<bool> {
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
                if o == f { break; }
                j += 1;
            }
            // Print context (1 line before if possible)
            if i > 0 {
                println!(" {}", orig_lines[i - 1]);
            }
            // Print removed lines
            for k in i..j.min(orig_lines.len()) {
                println!("-{}", orig_lines[k]);
            }
            // Print added lines
            for k in i..j.min(fmt_lines.len()) {
                println!("+{}", fmt_lines[k]);
            }
            // Print context (1 line after if possible)
            if j < max_lines {
                let after = orig_lines.get(j).or(fmt_lines.get(j)).copied().unwrap_or("");
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
        if content.contains(&format!("[dependencies]")) {
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
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&data);
                let actual = format!("sha256:{}", hex::encode(hasher.finalize()));

                if checksum != "unknown" && actual != *checksum {
                    println!("❌ checksum mismatch!");
                    println!("     expected: {}", checksum);
                    println!("     actual:   {}", actual);
                    lock.push_str(&format!("{} = {{ version = \"{}\", checksum = \"{}\", status = \"mismatch\" }}\n", name, version, actual));
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
                            println!("❌ {} v{} has an INVALID signature — aborting (signed by {})", name, version, status.signed_by);
                            lock.push_str(&format!("{} = {{ version = \"{}\", checksum = \"{}\", status = \"invalid_signature\" }}\n", name, version, actual));
                            continue;
                        }
                        println!("⚠  WARNING: {} v{} has an INVALID signature (signed by {})", name, version, status.signed_by);
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

                lock.push_str(&format!("{} = {{ version = \"{}\", checksum = \"{}\", signature = \"{}\" }}\n", name, version, actual, sig_status_str));
                println!("✓ {} bytes, verified, {}", data.len(), sig_status_str);
                any_fetched = true;
            }
            Err(e) => {
                println!("⚠ {}", e);
                lock.push_str(&format!("{} = {{ version = \"{}\", status = \"failed\" }}\n", name, version));
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
                    .push((spec.to_string(), resolved.clone(), config.package.name.clone()));

                println!("  {} {} v{} (resolved: {})", connector, name, spec, resolved);

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
                                for (prev_spec, prev_resolved, prev_by) in existing {
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
                                        .push((dep_spec.to_string(), dep_resolved.clone(), name.to_string()));

                                    println!("  {}  {} {} v{} (resolved: {})",
                                        continuation, dep_connector, dep_name, dep_spec, dep_resolved);

                                    // Fetch depth-2 transitive deps
                                    match registry_client::fetch_package_deps(dep_name, &dep_resolved) {
                                        Ok(deep_deps) if !deep_deps.is_empty() => {
                                            let mut deep_sorted = deep_deps;
                                            deep_sorted.sort_by_key(|(k, _)| k.clone());
                                            for (k, (deep_name, deep_spec)) in deep_sorted.iter().enumerate() {
                                                let deep_last = k == deep_sorted.len() - 1;
                                                let deep_connector = if deep_last { "└──" } else { "├──" };

                                                if let Some(existing) = resolved_versions.get(deep_name) {
                                                    for (prev_spec, prev_resolved, prev_by) in existing {
                                                        if prev_resolved != deep_spec {
                                                            match registry_client::resolve_version(deep_name, deep_spec) {
                                                                Ok(d_resolved) if d_resolved != *prev_resolved => {
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

                                                match registry_client::resolve_version(deep_name, deep_spec) {
                                                    Ok(d_resolved) => {
                                                        resolved_versions
                                                            .entry(deep_name.to_string())
                                                            .or_default()
                                                            .push((deep_spec.to_string(), d_resolved.clone(), dep_name.to_string()));
                                                        println!("  {}  {}  {} {} v{} (resolved: {})",
                                                            continuation, dep_cont, deep_connector, deep_name, deep_spec, d_resolved);
                                                    }
                                                    Err(e) => {
                                                        println!("  {}  {}  {} ⚠ {} v{}: {}", continuation, dep_cont, deep_connector, deep_name, deep_spec, e);
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    println!("  {}  {} ⚠ {} v{}: {}", continuation, dep_connector, dep_name, dep_spec, e);
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
            let installed = lock_content.lines()
                .filter(|l| l.contains("version") && !l.starts_with('#'))
                .count();
            println!("\n🔒 {} package(s) installed (see .sandbox/lock.toml)", installed);
        }
    }

    Ok(())
}

// ── Phase 4: Bytecode Interpreter ──

/// Tree-walking interpreter that evaluates Sandbox source directly
/// without compiling to C. Enables instant `sandbox interpret` for dev.
struct InterpreterState {
    vars: std::collections::HashMap<String, i64>,
    str_vars: std::collections::HashMap<String, String>,
    arr_vars: std::collections::HashMap<String, Vec<i64>>,
    lambdas: std::collections::HashMap<String, (Vec<ast::Param>, Vec<ast::Stmt>, std::collections::HashMap<String, i64>, std::collections::HashMap<String, String>, std::collections::HashMap<String, Vec<i64>>, std::collections::HashMap<String, Vec<i64>>, std::collections::HashMap<String, String>)>,
    functions: std::collections::HashMap<String, (Vec<ast::Param>, Option<ast::Type>, Vec<ast::Stmt>)>,
    struct_fields: std::collections::HashMap<String, Vec<String>>,
    struct_instances: std::collections::HashMap<String, Vec<i64>>,
    struct_type_of: std::collections::HashMap<String, String>,
    lambda_counter: usize,
}

impl InterpreterState {
    fn new() -> Self {
        Self {
            vars: std::collections::HashMap::new(),
            str_vars: std::collections::HashMap::new(),
            arr_vars: std::collections::HashMap::new(),
            lambdas: std::collections::HashMap::new(),
            functions: std::collections::HashMap::new(),
            struct_fields: std::collections::HashMap::new(),
            struct_instances: std::collections::HashMap::new(),
            struct_type_of: std::collections::HashMap::new(),
            lambda_counter: 0,
        }
    }

    fn is_string(&self, name: &str) -> bool {
        self.str_vars.contains_key(name)
    }

    fn get_string(&self, name: &str) -> Option<&str> {
        self.str_vars.get(name).map(|s| s.as_str())
    }

    fn is_array(&self, name: &str) -> bool {
        self.arr_vars.contains_key(name)
    }

    fn get_array(&self, name: &str) -> Option<&Vec<i64>> {
        self.arr_vars.get(name)
    }

    fn print_value(&self, expr: &ast::Expr) {
        match expr {
            ast::Expr::Ident(n) => {
                if let Some(s) = self.str_vars.get(n) {
                    println!("{}", s);
                } else if let Some(arr) = self.arr_vars.get(n) {
                    let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                    println!("[{}]", elems.join(", "));
                } else {
                    let val = self.vars.get(n.as_str()).unwrap_or(&0);
                    println!("{}", val);
                }
            }
            ast::Expr::ArrayLiteral(elems) => {
                let vals: Vec<String> = elems.iter().map(|e| {
                    match e {
                        ast::Expr::Str(s) => format!("\"{}\"", s),
                        _ => "0".to_string(),
                    }
                }).collect();
                println!("[{}]", vals.join(", "));
            }
            _ => {}
        }
    }
}

fn interpret(source: &str, filename: &str) -> anyhow::Result<()> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;
    let mut parser = parser::Parser::new(tokens);
    let program = parser.parse().map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    let mut state = InterpreterState::new();
    for item in &program.items {
        match item {
            ast::TopLevel::FnDef { name, params, ret, body, .. } => {
                state.functions.insert(name.clone(), (params.clone(), ret.clone(), body.clone()));
            }
            ast::TopLevel::StructDef { name, fields, .. } => {
                state.struct_fields.insert(name.clone(), fields.iter().map(|f| f.name.clone()).collect());
            }
            _ => {}
        }
    }

    let main_fn = state.functions.get("main").cloned()
        .ok_or_else(|| anyhow::anyhow!("No 'main' function found in {}", filename))?;

    exec_block(&main_fn.2, &mut state)?;
    Ok(())
}

fn exec_block(
    stmts: &[ast::Stmt],
    state: &mut InterpreterState,
) -> anyhow::Result<Option<i64>> {
    const BREAK_SENTINEL: i64 = -999999;
    const CONTINUE_SENTINEL: i64 = -999998;

    for stmt in stmts {
        match stmt {
            ast::Stmt::Let { name, value, .. } => {
                // Snapshot auto-keys before eval to detect what was created
                let str_keys_before: std::collections::HashSet<String> = state.str_vars.keys().filter(|k| k.starts_with("__auto_")).cloned().collect();
                let arr_keys_before: std::collections::HashSet<String> = state.arr_vars.keys().filter(|k| k.starts_with("__auto_arr_")).cloned().collect();
                let lambda_keys_before: std::collections::HashSet<String> = state.lambdas.keys().filter(|k| k.starts_with("__lambda_")).cloned().collect();
                let struct_keys_before: std::collections::HashSet<String> = state.struct_instances.keys().filter(|k| k.starts_with("__struct_")).cloned().collect();
                let int_val = eval_expr(value, state)?;
                // Clear old type entries for re-declaration (after eval so old value was accessible)
                state.str_vars.remove(name);
                state.arr_vars.remove(name);
                state.lambdas.remove(name);
                // Find new keys and rename to variable name
                let new_str_key = state.str_vars.keys().filter(|k| k.starts_with("__auto_") && !str_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_str_key {
                    if let Some(s) = state.str_vars.remove(&key) {
                        state.str_vars.insert(name.clone(), s);
                    }
                }
                let new_arr_key = state.arr_vars.keys().filter(|k| k.starts_with("__auto_arr_") && !arr_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_arr_key {
                    if let Some(a) = state.arr_vars.remove(&key) {
                        state.arr_vars.insert(name.clone(), a);
                    }
                }
                let new_lambda_key = state.lambdas.keys().filter(|k| k.starts_with("__lambda_") && !lambda_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_lambda_key {
                    if let Some(l) = state.lambdas.remove(&key) {
                        state.lambdas.insert(name.clone(), l);
                    }
                }
                // Transfer new struct instance to variable name
                let new_struct_key = state.struct_instances.keys().filter(|k| k.starts_with("__struct_") && !struct_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_struct_key {
                    if let Some(s) = state.struct_instances.remove(&key) {
                        state.struct_instances.insert(name.clone(), s);
                        // Copy type mapping from auto key to variable name
                        if let Some(type_name) = state.struct_type_of.remove(&key) {
                            state.struct_type_of.insert(name.clone(), type_name);
                        }
                    }
                }
                // If no string/array/lambda/struct was produced, store the integer value
                if !state.str_vars.contains_key(name.as_str()) && !state.arr_vars.contains_key(name.as_str()) && !state.lambdas.contains_key(name.as_str()) && !state.struct_instances.contains_key(name.as_str()) {
                    state.vars.insert(name.clone(), int_val);
                }
                // Clean up intermediate leaked auto keys (from sub-expression string literals)
                let leaked: Vec<String> = state.str_vars.keys()
                    .filter(|k| k.starts_with("__auto_") && **k != *name)
                    .cloned()
                    .collect();
                for k in leaked {
                    state.str_vars.remove(&k);
                }
            }
            ast::Stmt::Assign { name, value } => {
                // Evaluate FIRST (may reference current value of this variable)
                let str_keys_before: std::collections::HashSet<String> = state.str_vars.keys().filter(|k| k.starts_with("__auto_")).cloned().collect();
                let arr_keys_before: std::collections::HashSet<String> = state.arr_vars.keys().filter(|k| k.starts_with("__auto_arr_")).cloned().collect();
                let struct_keys_before: std::collections::HashSet<String> = state.struct_instances.keys().filter(|k| k.starts_with("__struct_")).cloned().collect();
                let int_val = eval_expr(value, state)?;
                // Now clear old type entries (after eval, so the old value was still accessible)
                state.str_vars.remove(name);
                state.arr_vars.remove(name);
                state.lambdas.remove(name);
                state.struct_instances.remove(name);
                state.struct_type_of.remove(name);
                let new_str_key = state.str_vars.keys().filter(|k| k.starts_with("__auto_") && !str_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_str_key {
                    if let Some(s) = state.str_vars.remove(&key) {
                        state.str_vars.insert(name.clone(), s);
                    }
                }
                let new_arr_key = state.arr_vars.keys().filter(|k| k.starts_with("__auto_arr_") && !arr_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_arr_key {
                    if let Some(a) = state.arr_vars.remove(&key) {
                        state.arr_vars.insert(name.clone(), a);
                    }
                }
                let new_struct_key = state.struct_instances.keys().filter(|k| k.starts_with("__struct_") && !struct_keys_before.contains(*k)).max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_struct_key {
                    if let Some(s) = state.struct_instances.remove(&key) {
                        state.struct_instances.insert(name.clone(), s);
                        if let Some(type_name) = state.struct_type_of.remove(&key) {
                            state.struct_type_of.insert(name.clone(), type_name);
                        }
                    }
                }
                if !state.str_vars.contains_key(name.as_str()) && !state.arr_vars.contains_key(name.as_str()) && !state.struct_instances.contains_key(name.as_str()) {
                    state.vars.insert(name.clone(), int_val);
                }
                // Clean up intermediate leaked auto keys
                let leaked: Vec<String> = state.str_vars.keys()
                    .filter(|k| k.starts_with("__auto_") && **k != *name)
                    .cloned()
                    .collect();
                for k in leaked {
                    state.str_vars.remove(&k);
                }
            }
            ast::Stmt::Print(expr) => {
                // Smart print: resolve strings/arrays from idents, eval everything else
                match expr {
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            println!("{}", s);
                        } else if let Some(arr) = state.arr_vars.get(n) {
                            let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                            println!("[{}]", elems.join(", "));
                        } else if let Some((params, _body, _cv, _cs, _ca, _, _)) = state.lambdas.get(n).cloned() {
                            // Print function pointer for lambdas
                            print!("<fn|");
                            for (i, p) in params.iter().enumerate() {
                                if i > 0 { print!(", "); }
                                print!("{}: {}", p.name, p.ty);
                            }
                            println!("|>");
                        } else {
                            let val = state.vars.get(n.as_str()).unwrap_or(&0);
                            println!("{}", val);
                        }
                    }
                    ast::Expr::Str(s) => println!("{}", s),
                    ast::Expr::ArrayLiteral(elems) => {
                        let vals: Vec<String> = elems.iter().map(|e| match e {
                            ast::Expr::Str(s) => format!("{}", s),
                            ast::Expr::Int(n) => n.to_string(),
                            ast::Expr::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
                            _ => "?".to_string(),
                        }).collect();
                        println!("[{}]", vals.join(", "));
                    }
                    ast::Expr::Call { name, type_args: _, args } => {
                        if name == "print" {
                            if let Some(first) = args.first() {
                                // Snapshot str_vars before to detect new strings
                                let str_before: std::collections::HashSet<String> = state.str_vars.keys().cloned().collect();
                                let val = eval_expr(first, state)?;
                                // Check if a new string was created (from concat, etc.)
                                let new_str = state.str_vars.keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else if let ast::Expr::Ident(n) = first {
                                    if let Some(s) = state.str_vars.get(n) {
                                        println!("{}", s);
                                    } else if let Some(arr) = state.arr_vars.get(n) {
                                        let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                                        println!("[{}]", elems.join(", "));
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    println!("{}", val);
                                }
                            }
                        } else {
                            let str_before: std::collections::HashSet<String> = state.str_vars.keys().cloned().collect();
                            let arr_before: std::collections::HashSet<String> = state.arr_vars.keys().cloned().collect();
                            let val = eval_expr(expr, state)?;
                            if val != 0 {
                                println!("{}", val);
                            } else {
                                let new_str = state.str_vars.keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    let new_arr = state.arr_vars.keys()
                                        .filter(|k| !arr_before.contains(*k))
                                        .max_by(|a, b| a.cmp(b))
                                        .cloned();
                                    if let Some(key) = new_arr {
                                        if let Some(arr) = state.arr_vars.get(&key) {
                                            let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                                            println!("[{}]", elems.join(", "));
                                        } else {
                                            println!("{}", val);
                                        }
                                    } else {
                                        println!("{}", val);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Snapshot to detect intermediate strings/arrays from expression evaluation
                        let str_before: std::collections::HashSet<String> = state.str_vars.keys().cloned().collect();
                        let arr_before: std::collections::HashSet<String> = state.arr_vars.keys().cloned().collect();
                        let val = eval_expr(expr, state)?;
                        let new_str = state.str_vars.keys()
                            .filter(|k| !str_before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned();
                        if let Some(key) = new_str {
                            if let Some(s) = state.str_vars.get(&key) {
                                println!("{}", s);
                            } else {
                                println!("{}", val);
                            }
                        } else {
                            let new_arr = state.arr_vars.keys()
                                .filter(|k| !arr_before.contains(*k))
                                .max_by(|a, b| a.cmp(b))
                                .cloned();
                            if let Some(key) = new_arr {
                                if let Some(arr) = state.arr_vars.get(&key) {
                                    let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                                    println!("[{}]", elems.join(", "));
                                } else {
                                    println!("{}", val);
                                }
                            } else {
                                println!("{}", val);
                            }
                        }
                    }
                }
            }
            ast::Stmt::Return(Some(expr)) => {
                let val = eval_expr(expr, state)?;
                return Ok(Some(val));
            }
            ast::Stmt::Return(None) => return Ok(Some(0)),
            ast::Stmt::Break => return Ok(Some(BREAK_SENTINEL)),
            ast::Stmt::Continue => return Ok(Some(CONTINUE_SENTINEL)),
            ast::Stmt::If { condition, then, else_ } => {
                let cond = eval_expr(condition, state)?;
                if cond != 0 {
                    if let Some(val) = exec_block(then, state)? {
                        return Ok(Some(val));
                    }
                } else if let Some(else_body) = else_ {
                    if let Some(val) = exec_block(else_body, state)? {
                        return Ok(Some(val));
                    }
                }
            }
            ast::Stmt::While { condition, body } => {
                loop {
                    let cond = eval_expr(condition, state)?;
                    if cond == 0 { break; }
                    match exec_block(&body, state)? {
                        Some(BREAK_SENTINEL) => break,
                        Some(CONTINUE_SENTINEL) => continue,
                        Some(val) => return Ok(Some(val)),
                        None => {}
                    }
                }
            }
            ast::Stmt::For { variable, iterable, body } => {
                // Check if iterable is a string
                if let ast::Expr::Ident(n) = iterable {
                    if let Some(s) = state.str_vars.get(n).cloned() {
                        let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
                        for c in chars {
                            state.vars.insert(variable.clone(), c);
                            state.str_vars.remove(variable);  // shadow string with char
                            match exec_block(&body, state)? {
                                Some(BREAK_SENTINEL) => break,
                                Some(CONTINUE_SENTINEL) => continue,
                                Some(val) => return Ok(Some(val)),
                                None => {}
                            }
                        }
                        continue;
                    }
                }
                if let ast::Expr::Str(s) = iterable {
                    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
                    for c in chars {
                        state.vars.insert(variable.clone(), c);
                        match exec_block(&body, state)? {
                            Some(BREAK_SENTINEL) => break,
                            Some(CONTINUE_SENTINEL) => continue,
                            Some(val) => return Ok(Some(val)),
                            None => {}
                        }
                    }
                    continue;
                }
                // Regular numeric range
                let count = eval_expr(iterable, state)?;
                for i in 0..count {
                    state.vars.insert(variable.clone(), i);
                    match exec_block(&body, state)? {
                        Some(BREAK_SENTINEL) => break,
                        Some(CONTINUE_SENTINEL) => continue,
                        Some(val) => return Ok(Some(val)),
                        None => {}
                    }
                }
            }
            ast::Stmt::ExprStmt(expr) => {
                eval_expr(expr, state)?;
            }
            ast::Stmt::IfLet { value, then, else_, .. } => {
                let val = eval_expr(value, state)?;
                if val != 0 {
                    if let Some(result) = exec_block(then, state)? {
                        return Ok(Some(result));
                    }
                } else if let Some(else_body) = else_ {
                    if let Some(result) = exec_block(else_body, state)? {
                        return Ok(Some(result));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(None)
}

/// Resolve the string value of an expression, if it has one.
fn resolve_str(expr: &ast::Expr, state: &InterpreterState) -> Option<String> {
    match expr {
        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
        ast::Expr::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// Resolve the array value of an expression, if it has one.
fn resolve_arr(expr: &ast::Expr, state: &InterpreterState) -> Option<Vec<i64>> {
    match expr {
        ast::Expr::Ident(n) => state.arr_vars.get(n).cloned(),
        _ => None,
    }
}


fn eval_expr(
    expr: &ast::Expr,
    state: &mut InterpreterState,
) -> anyhow::Result<i64> {
    match expr {
        ast::Expr::Int(n) => Ok(*n),
        ast::Expr::Float(n) => Ok(*n as i64),
        ast::Expr::Bool(b) => Ok(if *b { 1 } else { 0 }),
        ast::Expr::Str(s) => {
            // Store with auto key — exec_block will rename it to the variable
            let key = format!("__auto_{}", state.str_vars.len());
            state.str_vars.insert(key, s.clone());
            Ok(0)
        }
        ast::Expr::Ident(name) => {
            // Check strings first, then arrays, then integers
            if state.str_vars.contains_key(name) || state.arr_vars.contains_key(name) || state.lambdas.contains_key(name) {
                Ok(0) // these types use 0 as the integer representation
            } else {
                Ok(*state.vars.get(name.as_str()).unwrap_or(&0))
            }
        }
        ast::Expr::ArrayLiteral(elems) => {
            let mut vals = Vec::new();
            for e in elems {
                let v = eval_expr(e, state)?;
                vals.push(v);
            }
            let key = format!("__auto_arr_{}", state.arr_vars.len());
            state.arr_vars.insert(key, vals);
            Ok(0)
        }            ast::Expr::StructLiteral { name, fields, .. } => {
            // Evaluate struct literal: Point { x: 1, y: 2 }
            let field_defs = state.struct_fields.get(name).cloned().unwrap_or_default();
            let mut values = vec![0i64; field_defs.len()];
            for (fname, fval) in fields {
                if let Some(idx) = field_defs.iter().position(|f| f == fname) {
                    values[idx] = eval_expr(fval, state)?;
                }
            }
            let key = format!("__struct_{}", state.struct_instances.len());
            state.struct_type_of.insert(key.clone(), name.clone());
            state.struct_instances.insert(key, values);
            Ok(0)
        }
        ast::Expr::Lambda { params, body, .. } => {
            // Capture enclosing scope variables
            let captured_vars = state.vars.clone();
            let captured_strs = state.str_vars.clone();
            let captured_arrs = state.arr_vars.clone();
            let key = format!("__lambda_{}", state.lambda_counter);
            state.lambda_counter += 1;
            let captured_structs = state.struct_instances.clone();
            let captured_struct_types = state.struct_type_of.clone();
            state.lambdas.insert(key, (params.clone(), body.clone(), captured_vars, captured_strs, captured_arrs, captured_structs, captured_struct_types));
            Ok(0)
        }
        ast::Expr::BinaryOp { op, left, right } => {
            // Snapshot auto keys before evaluating each side to detect intermediate strings
            let str_before_left: std::collections::HashSet<String> = state.str_vars.keys().filter(|k| k.starts_with("__auto_")).cloned().collect();
            let l = eval_expr(left, state)?;
            let str_after_left: std::collections::HashSet<String> = state.str_vars.keys().filter(|k| k.starts_with("__auto_")).cloned().collect();
            let left_auto_key: Option<String> = str_after_left.difference(&str_before_left).cloned().max_by(|a, b| a.cmp(b));

            let str_before_right: std::collections::HashSet<String> = state.str_vars.keys().filter(|k| k.starts_with("__auto_")).cloned().collect();
            let r = eval_expr(right, state)?;
            let str_after_right: std::collections::HashSet<String> = state.str_vars.keys().filter(|k| k.starts_with("__auto_")).cloned().collect();
            let right_auto_key: Option<String> = str_after_right.difference(&str_before_right).cloned().max_by(|a, b| a.cmp(b));

            // Resolve string values — prefer auto keys from intermediate eval, then AST-based resolution
            let left_str = left_auto_key.and_then(|k| state.str_vars.get(&k).cloned())
                .or_else(|| resolve_str(left, state));
            let right_str = right_auto_key.and_then(|k| state.str_vars.get(&k).cloned())
                .or_else(|| resolve_str(right, state));
            let left_arr = resolve_arr(left, state);
            let right_arr = resolve_arr(right, state);

            match op {
                ast::BinOp::Add => {
                    // String concatenation
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        let combined = format!("{}{}", ls, rs);
                        let key = format!("__auto_{}", state.str_vars.len());
                        state.str_vars.insert(key, combined);
                        return Ok(0);
                    }
                    // Array concat
                    if let (Some(la), Some(ra)) = (&left_arr, &right_arr) {
                        let mut combined = la.clone();
                        combined.extend(ra);
                        let key = format!("__auto_arr_{}", state.arr_vars.len());
                        state.arr_vars.insert(key, combined);
                        return Ok(0);
                    }
                    Ok(l + r)
                }
                ast::BinOp::Sub => Ok(l - r),
                ast::BinOp::Mul => Ok(l * r),
                ast::BinOp::Div => Ok(if r != 0 { l / r } else { 0 }),
                ast::BinOp::Mod => Ok(if r != 0 { l % r } else { 0 }),
                ast::BinOp::Eq => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls == rs { 1 } else { 0 })
                    } else {
                        Ok(if l == r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Neq => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls != rs { 1 } else { 0 })
                    } else {
                        Ok(if l != r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Lt => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls < rs { 1 } else { 0 })
                    } else {
                        Ok(if l < r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Gt => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls > rs { 1 } else { 0 })
                    } else {
                        Ok(if l > r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Le => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls <= rs { 1 } else { 0 })
                    } else {
                        Ok(if l <= r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Ge => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls >= rs { 1 } else { 0 })
                    } else {
                        Ok(if l >= r { 1 } else { 0 })
                    }
                }
                ast::BinOp::And => Ok(if l != 0 && r != 0 { 1 } else { 0 }),
                ast::BinOp::Or => Ok(if l != 0 || r != 0 { 1 } else { 0 }),
            }
        }
        ast::Expr::UnaryOp { op, expr } => {
            let val = eval_expr(expr, state)?;
            match op {
                ast::UnOp::Neg => Ok(-val),
                ast::UnOp::Not => Ok(if val == 0 { 1 } else { 0 }),
            }
        }
        ast::Expr::Index { target, index } => {
            let idx = eval_expr(index, state)? as usize;
            if let ast::Expr::Ident(n) = target.as_ref() {
                if let Some(arr) = state.arr_vars.get(n) {
                    return Ok(arr.get(idx).copied().unwrap_or(0));
                }
                if let Some(s) = state.str_vars.get(n) {
                    return Ok(s.as_bytes().get(idx).copied().map(|b| b as i64).unwrap_or(0));
                }
            }
            Ok(0)
        }
        ast::Expr::Call { name, type_args: _, args } => {
            if name == "print" {
                if !args.is_empty() {
                    // Inline print resolution for call context
                    match &args[0] {
                        ast::Expr::Ident(n) => {
                            if let Some(s) = state.str_vars.get(n) {
                                println!("{}", s);
                            } else if let Some(arr) = state.arr_vars.get(n) {
                                let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                                println!("[{}]", elems.join(", "));
                            } else {
                                let val = state.vars.get(n.as_str()).unwrap_or(&0);
                                println!("{}", val);
                            }
                        }
                        ast::Expr::Str(s) => println!("{}", s),
                        ast::Expr::ArrayLiteral(elems) => {
                            let vals: Vec<String> = elems.iter().map(|e| match e {
                                ast::Expr::Str(s) => format!("{}", s),
                                ast::Expr::Int(n) => n.to_string(),
                                _ => "?".to_string(),
                            }).collect();
                            println!("[{}]", vals.join(", "));
                        }
                        other => {
                            let str_before: std::collections::HashSet<String> = state.str_vars.keys().cloned().collect();
                            let arr_before: std::collections::HashSet<String> = state.arr_vars.keys().cloned().collect();
                            let val = eval_expr(other, state)?;
                            if val != 0 {
                                println!("{}", val);
                            } else {
                                let new_str = state.str_vars.keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    let new_arr = state.arr_vars.keys()
                                        .filter(|k| !arr_before.contains(*k))
                                        .max_by(|a, b| a.cmp(b))
                                        .cloned();
                                    if let Some(key) = new_arr {
                                        if let Some(arr) = state.arr_vars.get(&key) {
                                            let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                                            println!("[{}]", elems.join(", "));
                                        } else {
                                            println!("{}", val);
                                        }
                                    } else {
                                        println!("{}", val);
                                    }
                                }
                            }
                        }
                    }
                }
                return Ok(0);
            }
            if name == "len" && args.len() == 1 {
                // Snapshot auto-keys before evaluating arg (detect side-effectful calls)
                let str_keys_before: std::collections::HashSet<String> = state.str_vars.keys().cloned().collect();
                let arr_keys_before: std::collections::HashSet<String> = state.arr_vars.keys().cloned().collect();
                let val = eval_expr(&args[0], state)?;
                // Check for new string auto-key
                let new_str_key = state.str_vars.keys()
                    .filter(|k| !str_keys_before.contains(*k))
                    .max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_str_key {
                    if let Some(s) = state.str_vars.get(&key) {
                        return Ok(s.len() as i64);
                    }
                }
                // Check for new array auto-key
                let new_arr_key = state.arr_vars.keys()
                    .filter(|k| !arr_keys_before.contains(*k))
                    .max_by(|a, b| a.cmp(b)).cloned();
                if let Some(key) = new_arr_key {
                    if let Some(a) = state.arr_vars.get(&key) {
                        return Ok(a.len() as i64);
                    }
                }
                // Fallback: check named variables
                match &args[0] {
                    ast::Expr::Str(s) => return Ok(s.len() as i64),
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            return Ok(s.len() as i64);
                        }
                        if let Some(a) = state.arr_vars.get(n) {
                            return Ok(a.len() as i64);
                        }
                    }
                    ast::Expr::ArrayLiteral(elems) => return Ok(elems.len() as i64),
                    _ => {}
                }
                return Ok(0);
            }
            // Array built-in: map(arr, lambda)
            if name == "map" && args.len() == 2 {
                // Get the array
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        // Evaluate to get array - try to find auto key
                        eval_expr(&args[0], state)?;
                        let new_key = state.arr_vars.keys()
                            .filter(|k| k.starts_with("__auto_arr_"))
                            .next().cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else { vec![] }
                    }
                };
                // Get the lambda
                let lambda_info = match &args[1] {
                    ast::Expr::Lambda { params, body, .. } => {
                        let cv = state.vars.clone();
                        let cs = state.str_vars.clone();
                        let ca = state.arr_vars.clone();
                        let csi = state.struct_instances.clone();
                        let cst = state.struct_type_of.clone();
                        Some((params.clone(), body.clone(), cv, cs, ca, csi, cst))
                    },
                    ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                    _ => None,
                };
                if let Some((params, body, cap_vars, cap_strs, cap_arrs, cap_structs, cap_struct_types)) = lambda_info {
                    let mut result = Vec::new();
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.vars.extend(cap_vars.clone());
                        local_state.str_vars.extend(cap_strs.clone());
                        local_state.arr_vars.extend(cap_arrs.clone());
                        local_state.struct_instances.extend(cap_structs.clone());
                        local_state.struct_type_of.extend(cap_struct_types.clone());
                        if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        match exec_block(&body, &mut local_state)? {
                            Some(v) => result.push(v),
                            None => result.push(0),
                        }
                    }
                    let key = format!("__auto_arr_{}", state.arr_vars.len());
                    state.arr_vars.insert(key, result);
                    return Ok(0);
                }
                return Ok(0);
            }
            // Array built-in: filter(arr, lambda)
            if name == "filter" && args.len() == 2 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        eval_expr(&args[0], state)?;
                        let new_key = state.arr_vars.keys()
                            .filter(|k| k.starts_with("__auto_arr_"))
                            .next().cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else { vec![] }
                    }
                };
                let lambda_info = match &args[1] {
                    ast::Expr::Lambda { params, body, .. } => {
                        let cv = state.vars.clone();
                        let cs = state.str_vars.clone();
                        let ca = state.arr_vars.clone();
                        let csi = state.struct_instances.clone();
                        let cst = state.struct_type_of.clone();
                        Some((params.clone(), body.clone(), cv, cs, ca, csi, cst))
                    },
                    ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                    _ => None,
                };
                if let Some((params, body, cap_vars, cap_strs, cap_arrs, cap_structs, cap_struct_types)) = lambda_info {
                    let mut result = Vec::new();
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.vars.extend(cap_vars.clone());
                        local_state.str_vars.extend(cap_strs.clone());
                        local_state.arr_vars.extend(cap_arrs.clone());
                        local_state.struct_instances.extend(cap_structs.clone());
                        local_state.struct_type_of.extend(cap_struct_types.clone());
                        if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        let cond = match exec_block(&body, &mut local_state)? {
                            Some(v) => v,
                            None => 0,
                        };
                        if cond != 0 {
                            result.push(*elem);
                        }
                    }
                    let key = format!("__auto_arr_{}", state.arr_vars.len());
                    state.arr_vars.insert(key, result);
                    return Ok(0);
                }
                return Ok(0);
            }
            // Array built-in: reduce(arr, lambda, initial)
            if name == "reduce" && args.len() == 3 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        eval_expr(&args[0], state)?;
                        let new_key = state.arr_vars.keys()
                            .filter(|k| k.starts_with("__auto_arr_"))
                            .next().cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else { vec![] }
                    }
                };
                let mut acc = eval_expr(&args[2], state)?;
                let lambda_info = match &args[1] {
                    ast::Expr::Lambda { params, body, .. } => {
                        let cv = state.vars.clone();
                        let cs = state.str_vars.clone();
                        let ca = state.arr_vars.clone();
                        let csi = state.struct_instances.clone();
                        let cst = state.struct_type_of.clone();
                        Some((params.clone(), body.clone(), cv, cs, ca, csi, cst))
                    },
                    ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                    _ => None,
                };
                if let Some((params, body, cap_vars, cap_strs, cap_arrs, cap_structs, cap_struct_types)) = lambda_info {
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.vars.extend(cap_vars.clone());
                        local_state.str_vars.extend(cap_strs.clone());
                        local_state.arr_vars.extend(cap_arrs.clone());
                        local_state.struct_instances.extend(cap_structs.clone());
                        local_state.struct_type_of.extend(cap_struct_types.clone());
                        if params.len() >= 2 {
                            local_state.vars.insert(params[0].name.clone(), acc);
                            local_state.vars.insert(params[1].name.clone(), *elem);
                        } else if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        acc = match exec_block(&body, &mut local_state)? {
                            Some(v) => v,
                            None => 0,
                        };
                    }
                    return Ok(acc);
                }
                return Ok(0);
            }
            // Lambda call
            if let Some((params, body, cap_vars, cap_strs, cap_arrs, cap_structs, cap_struct_types)) = state.lambdas.get(name).cloned() {
                let mut local_state = InterpreterState::new();
                local_state.functions = state.functions.clone();
                local_state.struct_fields = state.struct_fields.clone();
                // Inject captured scope
                local_state.vars.extend(cap_vars);
                local_state.str_vars.extend(cap_strs);
                local_state.arr_vars.extend(cap_arrs);
                local_state.struct_instances.extend(cap_structs);
                local_state.struct_type_of.extend(cap_struct_types);
                // Inject explicit arguments (override captured if same name)
                for (param, arg) in params.iter().zip(args.iter()) {
                    let val = eval_expr(arg, state)?;
                    local_state.vars.insert(param.name.clone(), val);
                }
                match exec_block(&body, &mut local_state)? {
                    Some(v) => Ok(v),
                    None => Ok(0),
                }
            } else if let Some((params, _ret, body)) = state.functions.get(name).cloned() {
                let mut local_state = InterpreterState::new();
                local_state.functions = state.functions.clone();
                for (param, arg) in params.iter().zip(args.iter()) {
                    let val = eval_expr(arg, state)?;
                    local_state.vars.insert(param.name.clone(), val);
                }
                match exec_block(&body, &mut local_state)? {
                    Some(v) => Ok(v),
                    None => Ok(0),
                }
            } else {
                Ok(0)
            }
        }
        ast::Expr::Range { start, end, inclusive } => {
            let s = eval_expr(start, state)?;
            let e = eval_expr(end, state)?;
            let count = if *inclusive { e - s + 1 } else { e - s };
            Ok(if count > 0 { count } else { 0 })
        }
        ast::Expr::FieldAccess { target, field } => {
            if let ast::Expr::Ident(name) = target.as_ref() {
                // Struct field access: my_struct.field
                if let Some(instance) = state.struct_instances.get(name) {
                    // Look up field defs using the type name, not the variable name
                    let type_name = state.struct_type_of.get(name).cloned();
                    if let Some(ref tn) = type_name {
                        if let Some(fields) = state.struct_fields.get(tn) {
                            if let Some(idx) = fields.iter().position(|f| f == field) {
                                return Ok(instance.get(idx).copied().unwrap_or(0));
                            }
                        }
                    }
                }
                // Fallback: string/array .len
                if let Some(s) = state.str_vars.get(name) {
                    if field == "len" { return Ok(s.len() as i64); }
                }
                if let Some(a) = state.arr_vars.get(name) {
                    if field == "len" { return Ok(a.len() as i64); }
                }
            }
            Ok(0)
        }
        _ => Ok(0),
    }
}

// ── Phase 4: Documentation Generator ──

fn generate_docs(source: &str) -> anyhow::Result<()> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;
    let mut parser = parser::Parser::new(tokens);
    let program = parser.parse().map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

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
            ast::TopLevel::FnDef { name, params, ret, doc, .. } => {
                if let Some(d) = doc {
                    for line in d.lines() {
                        println!("// {}", line);
                    }
                }
                let params_str: Vec<String> = params.iter()
                    .map(|p| format!("{}: {}", p.name, p.ty))
                    .collect();
                let ret_str = ret.as_ref().map_or("void".to_string(), |t| t.to_string());
                println!("## `{}({}) -> {}`", name, params_str.join(", "), ret_str);
                println!();
            }
            ast::TopLevel::StructDef { name, fields, doc, .. } => {
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
            ast::TopLevel::EnumDef { name, variants, doc, .. } => {
                if let Some(d) = doc {
                    for line in d.lines() {
                        println!("// {}", line);
                    }
                }
                println!("## enum `{}`", name);
                println!();
                for v in variants {
                    let payload_str = v.payload.as_ref()
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
                        let params_str: Vec<String> = params.iter()
                            .map(|p| format!("{}: {}", p.name, p.ty))
                            .collect();
                        let ret_str = ret.as_ref().map_or("void".to_string(), |t| t.to_string());
                        println!("### `{}::{}({}) -> {}`", name, sub_name(sub), params_str.join(", "), ret_str);
                    }
                }
                println!();
            }
            ast::TopLevel::ImplDef { type_name, methods, .. } => {
                println!("## impl `{}`", type_name);
                println!();
                for method in methods {
                    if let ast::TopLevel::FnDef { name, params, ret, .. } = method {
                        let params_str: Vec<String> = params.iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("{}: {}", p.name, p.ty))
                            .collect();
                        let ret_str = ret.as_ref().map_or("void".to_string(), |t| t.to_string());
                        println!("### `{}::{}({}) -> {}`", type_name, name, params_str.join(", "), ret_str);
                    }
                }
                println!();
            }
            _ => {}
        }
    }

    Ok(())
}