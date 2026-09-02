use crate::ast::{Program, TopLevel};
use crate::codegen::CodeGen;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typechecker::TypeChecker;
use crate::wasmgen::WasmGen;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Compiler {
    pub source: String,
    pub filename: String,
}

impl Compiler {
    pub fn new(source: &str, filename: &str) -> Self {
        Self {
            source: source.to_string(),
            filename: filename.to_string(),
        }
    }

    /// Load file-based modules: `mod utils` without a body loads from utils.sbx
    fn load_file_modules(program: &mut Program, source_file: &str) {
        let source_dir = Path::new(source_file).parent().unwrap_or(Path::new("."));
        for item in &mut program.items {
            if let TopLevel::ModuleDef { name, items, .. } = item {
                if items.is_empty() {
                    let mod_name = name.clone();
                    let possible_paths = [
                        source_dir.join(format!("{}.sbx", mod_name)),
                        source_dir.join(&mod_name).join(format!("{}.sbx", mod_name)),
                        source_dir.join(&mod_name).join("mod.sbx"),
                    ];
                    if let Some(path) = possible_paths.iter().find_map(|p| {
                        if p.exists() { Some(p.clone()) } else { None }
                    }) {
                        if let Ok(source) = fs::read_to_string(&path) {
                            if let Ok(mod_program) = Self::parse_vendor_source(&source) {
                                *items = mod_program.items;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Load vendored packages referenced by `use` statements.
    /// Scans the AST for `use pkg_name::...` and if `pkg_name` exists
    /// in `.sandbox/vendor/`, parses its source and merges items into the program.
    fn load_vendored_packages(program: &mut Program, source_file: &str) {
        // Collect unique package names from use statements
        let mut seen = std::collections::HashSet::new();
        let pkg_names: Vec<String> = program.items.iter().filter_map(|item| {
            if let TopLevel::Use { path, .. } = item {
                path.first().cloned().filter(|n| seen.insert(n.clone()))
            } else {
                None
            }
        }).collect();

        // Resolve vendor root relative to the source file's directory
        let source_dir = Path::new(source_file).parent().unwrap_or(Path::new("."));
        let vendor_root = source_dir.join(".sandbox").join("vendor");

        for pkg_name in &pkg_names {
            // Try multiple possible paths for the vendored package
            let possible_paths = [
                vendor_root.join(pkg_name).join(format!("{}.sbx", pkg_name)),
                vendor_root.join(pkg_name).join("lib.sbx"),
                vendor_root.join(pkg_name).join("src").join("main.sbx"),
                vendor_root.join(pkg_name).join(pkg_name).join(format!("{}.sbx", pkg_name)),
                // Fallback to CWD-relative paths
                PathBuf::from(format!(".sandbox/vendor/{}/{}/{}.sbx", pkg_name, pkg_name, pkg_name)),
                PathBuf::from(format!(".sandbox/vendor/{}/lib.sbx", pkg_name)),
            ];

            let vendor_source = possible_paths.iter().find_map(|p| {
                fs::read_to_string(p).ok()
            });

            if let Some(source) = vendor_source {
                // Parse the vendored source and wrap in a ModuleDef
                // so functions are registered as pkgname::funcname in the typechecker
                if let Ok(vendor_program) = Self::parse_vendor_source(&source) {
                    let module = TopLevel::ModuleDef {
                        name: pkg_name.clone(),
                        items: vendor_program.items,
                        doc: None,
                    };
                    program.items.insert(0, module);
                }
            }
        }
    }

    /// Parse vendored package source into a Program (quietly, no output)
    fn parse_vendor_source(source: &str) -> Result<Program> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        Ok(parser.parse()?)
    }

    /// Parse source and load vendored packages (common path for all commands)
    fn parse_with_vendors(&self, source: &str, print: bool) -> Result<Program> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let mut program = parser.parse()?;
        let vendor_count = program.items.len();
        Self::load_file_modules(&mut program, &self.filename);
        Self::load_vendored_packages(&mut program, &self.filename);
        if print && program.items.len() > vendor_count {
            println!("  ✓ Loaded {} vendored/package(s)", program.items.len() - vendor_count);
        }
        Ok(program)
    }

    pub fn compile(&self) -> Result<String> {
        println!("[sandbox] Compiling {}", self.filename);

        println!("  → Lexing...");
        let mut lexer = Lexer::new(&self.source);
        let tokens = lexer.tokenize()?;
        println!("  ✓ {} tokens", tokens.len());

        println!("  → Parsing...");
        let program = self.parse_with_vendors(&self.source, true)?;
        println!("  ✓ {} top-level items", program.items.len());
        for (i, item) in program.items.iter().enumerate() {
            match item {
                TopLevel::FnDef { name, .. } => println!("    [{}] FnDef {}", i, name),
                TopLevel::ModuleDef { name, items, .. } => println!("    [{}] ModuleDef {} ({} items)", i, name, items.len()),
                TopLevel::Use { path, .. } => println!("    [{}] Use {}", i, path.join("::")),
                _ => println!("    [{}] Other", i),
            }
        }

        println!("  → Type checking...");
        let mut checker = TypeChecker::new();
        checker.check(&program)?;

        println!("  → Generating C code...");
        let mut codegen = CodeGen::new();
        let c_code = codegen.generate(&program, None);
        println!("  ✓ {} lines of C", c_code.lines().count());

        Ok(c_code)
    }

    /// Compile without printing progress messages (for REPL)
    pub fn compile_quiet(&self) -> Result<String> {
        let program = self.parse_with_vendors(&self.source, false)?;
        let mut checker = TypeChecker::new();
        checker.check(&program)?;
        let mut codegen = CodeGen::new();
        let c_code = codegen.generate(&program, None);
        Ok(c_code)
    }

    fn parse_for_codegen(&self) -> Result<Program> {
        println!("[sandbox] Compiling {}", self.filename);

        println!("  → Lexing...");
        let mut lexer = Lexer::new(&self.source);
        let tokens = lexer.tokenize()?;
        println!("  ✓ {} tokens", tokens.len());

        println!("  → Parsing and loading vendors...");
        let program = self.parse_with_vendors(&self.source, true)?;

        println!("  → Type checking...");
        let mut checker = TypeChecker::new();
        checker.check(&program)?;

        Ok(program)
    }

    pub fn build(&self, output: &str) -> Result<()> {
        let c_code = self.compile()?;
        let c_path = format!("{}.c", output);
        fs::write(&c_path, &c_code)?;

        println!("  → Compiling C to native binary...");
        let status = Command::new("gcc")
            .arg("-o")
            .arg(output)
            .arg(&c_path)
            .arg("-lm")
            .arg("-Wno-incompatible-pointer-types")
            .status()?;
        if !status.success() {
            return Err(anyhow!("gcc compilation failed"));
        }
        println!("  ✓ Built: {}", output);
        let _ = fs::remove_file(&c_path);
        Ok(())
    }

    pub fn build_wasm(&self, output: &str) -> Result<()> {
        let program = self.parse_for_codegen()?;

        println!("  → Generating WebAssembly (.wat)...");
        let mut wasmgen = WasmGen::new();
        let wat = wasmgen.generate(&program);

        let wat_path = format!("{}.wat", output);
        fs::write(&wat_path, &wat)?;
        println!("  ✓ {} lines of WAT", wat.lines().count());

        // Try to compile with wat2wasm if available
        let wasm_path = format!("{}.wasm", output);
        let status = Command::new("wat2wasm")
            .arg(&wat_path)
            .arg("-o")
            .arg(&wasm_path)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("  ✓ Built: {}", wasm_path);
                println!("  Run with: wasmtime {}", wasm_path);
            }
            _ => {
                println!(
                    "  ⚠ wat2wasm not found. .wat file generated at: {}",
                    wat_path
                );
                println!("  Install wabt: apt install wabt  (or brew install wabt)");
            }
        }

        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        let c_code = self.compile()?;

        let id = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(format!("sandbox_{}.c", id));
        let bin = std::env::temp_dir().join(format!("sandbox_{}", id));

        fs::write(&tmp, &c_code)?;

        let status = Command::new("gcc")
            .arg("-o")
            .arg(&bin)
            .arg(&tmp)
            .arg("-lm")
            .arg("-Wno-incompatible-pointer-types")
            .status()?;
        if !status.success() {
            let debug_path = std::env::temp_dir().join("sandbox_debug.c");
            let _ = fs::copy(&tmp, &debug_path);
            let _ = fs::remove_file(&tmp);
            return Err(anyhow!(
                "gcc compilation failed — C saved to {}",
                debug_path.display()
            ));
        }

        let status = Command::new(&bin).status()?;

        let _ = fs::remove_file(&tmp);
        let _ = fs::remove_file(&bin);

        if !status.success() {
            return Err(anyhow!(
                "Program exited with status {}",
                status.code().unwrap_or(1)
            ));
        }
        Ok(())
    }

    pub fn check(&self) -> Result<()> {
        println!("[sandbox] Checking {}", self.filename);

        let mut lexer = Lexer::new(&self.source);
        let tokens = lexer.tokenize()?;
        println!("  ✓ {} tokens", tokens.len());

        let program = self.parse_with_vendors(&self.source, true)?;
        println!("  ✓ {} top-level items", program.items.len());

        let mut checker = TypeChecker::new();
        checker.check(&program)?;

        println!("[sandbox] All checks passed for {}", self.filename);
        Ok(())
    }

    /// Run tests: compile with __run_tests() as entry point
    pub fn run_tests(&self, filter: Option<&str>) -> Result<()> {
        println!("[sandbox] Running tests in {}", self.filename);

        println!("  → Lexing...");
        let mut lexer = Lexer::new(&self.source);
        let tokens = lexer.tokenize()?;
        println!("  ✓ {} tokens", tokens.len());

        println!("  → Parsing...");
        let mut parser = Parser::new(tokens);
        let program = parser.parse()?;
        println!("  ✓ {} top-level items", program.items.len());

        println!("  → Type checking...");
        let mut checker = TypeChecker::new();
        checker.check(&program)?;

        println!("  → Generating C code...");
        let mut codegen = CodeGen::new();
        let c_code = codegen.generate(&program, filter);
        println!("  ✓ {} lines of C", c_code.lines().count());

        // Check if there are test functions
        let has_tests = program.items.iter().any(|item| matches!(item, crate::ast::TopLevel::TestDef { .. }));
        if !has_tests {
            println!("  ⚠ No test functions found. Use 'test fn name {{ ... }}' to define tests.");
            return Ok(());
        }

        let id = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(format!("sandbox_test_{}.c", id));
        let bin = std::env::temp_dir().join(format!("sandbox_test_{}", id));

        fs::write(&tmp, &c_code)?;

        let status = Command::new("gcc")
            .arg("-o")
            .arg(&bin)
            .arg(&tmp)
            .arg("-lm")
            .arg("-Wno-incompatible-pointer-types")
            .status()?;
        if !status.success() {
            let debug_path = std::env::temp_dir().join("sandbox_test_debug.c");
            let _ = fs::copy(&tmp, &debug_path);
            let _ = fs::remove_file(&tmp);
            return Err(anyhow!(
                "gcc compilation failed — C saved to {}",
                debug_path.display()
            ));
        }

        let status = Command::new(&bin).status()?;

        let _ = fs::remove_file(&tmp);
        let _ = fs::remove_file(&bin);

        if !status.success() {
            return Err(anyhow!(
                "Tests failed with exit code {}",
                status.code().unwrap_or(1)
            ));
        }
        Ok(())
    }

    pub fn wasm(&self, output: &str) -> Result<()> {
        let program = self.parse_for_codegen()?;

        println!("  → Generating WebAssembly text format...");
        let mut wasmgen = WasmGen::new();
        let wat = wasmgen.generate(&program);

        fs::write(output, &wat)?;
        println!("  ✓ {} lines of WAT → {}", wat.lines().count(), output);
        Ok(())
    }

    pub fn llvm(&self, output: &str) -> Result<()> {
        let program = self.parse_for_codegen()?;

        println!("  Generating LLVM IR...");
        let mut llvmgen = crate::llvmgen::LlvmGen::new();
        let ir = llvmgen.generate(&program);

        fs::write(output, &ir)?;
        println!("  {} lines of LLVM IR -> {}", ir.lines().count(), output);

        // Also emit the C runtime so the user can link it manually.
        // Strip `static` from function definitions so they're visible to the linker.
        let runtime = strip_static_funcs(crate::stdlib::c_preamble());
        let runtime_path = format!("{}.runtime.c", output);
        fs::write(&runtime_path, &runtime)?;
        println!("  Runtime C source -> {}", runtime_path);
        println!("  Compile with: clang {} {} -o output -lm -lpthread", output, runtime_path);
        Ok(())
    }

    pub fn build_llvm(&self, output: &str) -> Result<()> {
        let program = self.parse_for_codegen()?;

        println!("  Generating LLVM IR...");
        let mut llvmgen = crate::llvmgen::LlvmGen::new();
        let ir = llvmgen.generate(&program);

        let ll_path = format!("{}.ll", output);
        fs::write(&ll_path, &ir)?;
        println!("  {} lines of LLVM IR -> {}", ir.lines().count(), ll_path);

        // Write the C runtime to a file so clang can link it.
        // Strip `static` from function definitions so they link correctly.
        let runtime = strip_static_funcs(crate::stdlib::c_preamble());
        let runtime_path = format!("{}.runtime.c", output);
        fs::write(&runtime_path, &runtime)?;

        println!("  Compiling with clang...");
        let status = Command::new("clang")
            .arg(&ll_path)
            .arg(&runtime_path)
            .arg("-o")
            .arg(output)
            .arg("-lm")
            .arg("-lpthread")
            .status()?;
        if !status.success() {
            return Err(anyhow!("clang compilation failed"));
        }
        println!("  Built: {}", output);
        Ok(())
    }
}

/// Strip the `static` keyword from function definitions in the C runtime preamble
/// so that the functions are visible when linked as a separate translation unit
/// (the LLVM backend emits a .ll file and links the runtime .c separately).
fn strip_static_funcs(c_code: String) -> String {
    c_code
        .lines()
        .map(|line| {
            // Only strip `static ` from function definitions (not static variables/arrays).
            // Pattern: `static <type> <name>(` → `<type> <name>(`
            if line.starts_with("static ") {
                let rest = &line[7..]; // skip "static "
                // Check if it looks like a function definition: has `(` (params)
                // and isn't a struct/typedef/variable declaration.
                // We strip static from ALL function defs, including `const char* func(...)`.
                if rest.contains('(')
                    && !rest.starts_with("struct ")
                    && !rest.contains("[]")
                    && !rest.contains(" typedef ")
                {
                    return rest.to_string();
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
