use crate::codegen::CodeGen;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typechecker::TypeChecker;
use anyhow::{anyhow, Result};
use std::fs;
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

    pub fn compile(&self) -> Result<String> {
        println!("[sandbox] Compiling {}", self.filename);

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
        let c_code = codegen.generate(&program);
        println!("  ✓ {} lines of C", c_code.lines().count());

        Ok(c_code)
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
            .status()?;
        if !status.success() {
            return Err(anyhow!("gcc compilation failed"));
        }
        println!("  ✓ Built: {}", output);
        let _ = fs::remove_file(&c_path);
        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        let c_code = self.compile()?;

        // Use unique temp files to avoid race conditions
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
            .status()?;
        if !status.success() {
            let _ = fs::remove_file(&tmp);
            return Err(anyhow!("gcc compilation failed"));
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

        let mut parser = Parser::new(tokens);
        let program = parser.parse()?;
        println!("  ✓ {} top-level items", program.items.len());

        let mut checker = TypeChecker::new();
        checker.check(&program)?;

        println!("[sandbox] All checks passed for {}", self.filename);
        Ok(())
    }
}
