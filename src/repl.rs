use crate::compiler::Compiler;
use anyhow::Result;
use std::io::Write;

/// Sandbox REPL — interactive mode with incremental compilation.
/// Definitions (fn, enum, struct) accumulate across iterations.
/// Evaluations are wrapped in `__repl_main()` and re-executed each time.
pub fn run_repl() -> Result<()> {
    println!("Sandbox REPL v0.4.0");
    println!("Type expressions to evaluate, or define functions/enums/structs.");
    println!("Multi-line input: open a block with {{ and press Enter to continue.");
    println!("Commands: :q (quit), :history, :reset, :show");
    println!();

    let mut definitions = String::new(); // fn, enum, struct definitions
    let mut eval_body = String::new(); // accumulated eval statements
    let mut input_buffer = String::new();
    let mut brace_depth: i32 = 0;
    let mut history: Vec<String> = Vec::new();

    loop {
        // Prompt
        if brace_depth > 0 {
            print!("  ... ");
        } else {
            print!("sbx> ");
        }
        std::io::stdout().flush()?;

        let mut line = String::new();
        if std::io::stdin().read_line(&mut line)? == 0 {
            println!();
            break;
        }
        let line = line.trim_end().to_string();

        // Handle empty lines
        if line.is_empty() && brace_depth == 0 {
            continue;
        }

        // Handle commands
        if brace_depth == 0 && line.starts_with(':') {
            match line.trim() {
                ":q" | ":quit" => {
                    println!("Goodbye!");
                    break;
                }
                ":history" => {
                    for (i, h) in history.iter().enumerate() {
                        println!("  {}: {}", i + 1, h.lines().next().unwrap_or(""));
                    }
                    continue;
                }
                ":reset" => {
                    definitions.clear();
                    eval_body.clear();
                    println!("  Reset. All definitions cleared.");
                    continue;
                }
                ":show" => {
                    if definitions.trim().is_empty() && eval_body.trim().is_empty() {
                        println!("  (no accumulated code)");
                    } else {
                        if !definitions.trim().is_empty() {
                            println!("{}", definitions.trim());
                        }
                        if !eval_body.trim().is_empty() {
                            println!("// eval:");
                            println!("{}", eval_body.trim());
                        }
                    }
                    continue;
                }
                _ => {
                    println!("  Unknown command: {}", line);
                    println!("  Available: :q, :history, :reset, :show");
                    continue;
                }
            }
        }

        // Accumulate input
        if !input_buffer.is_empty() {
            input_buffer.push('\n');
        }
        input_buffer.push_str(&line);

        // Track brace depth for multi-line blocks
        for ch in line.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
        }

        // If we're in a multi-line block, keep reading
        if brace_depth > 0 {
            continue;
        }

        // We have a complete input — process it
        let input = input_buffer.trim().to_string();
        input_buffer.clear();

        if input.is_empty() {
            continue;
        }

        history.push(input.clone());

        // Route the input based on its type
        let trimmed = input.trim();
        let is_def = trimmed.starts_with("fn ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("mod ");

        if is_def {
            // Add to definitions, verify it compiles
            let test_source = build_full_source(&definitions, &eval_body);
            let test_source = format!("{}\n{}\n", test_source.trim_end(), input);
            match try_compile(&test_source) {
                Ok(_) => {
                    definitions.push_str(&format!("\n{}\n", input));
                    println!("  ✓ defined");
                }
                Err(e) => {
                    println!("  Error: {}", e);
                }
            }
        } else {
            // Expression or statement — add as eval line and execute
            let test_body = if eval_body.is_empty() {
                format!("\n{}\n", input)
            } else {
                format!("{}\n{}\n", eval_body, input)
            };

            let is_print = trimmed.starts_with("print(");
            let is_expr = !is_print
                && !trimmed.starts_with("if ")
                && !trimmed.starts_with("while ")
                && !trimmed.starts_with("for ")
                && !trimmed.starts_with("return ")
                && !trimmed.starts_with("db::")
                && !trimmed.starts_with("http::")
                && !trimmed.starts_with("json::")
                && !trimmed.starts_with("panic(")
                && !trimmed.contains('=')
                && !trimmed.ends_with('}');

            let source = if is_expr {
                // Bare expression — try to auto-print
                format!(
                    "{}\nfn __repl_main() {{\n{}\nprint({})\n}}\nfn main() {{ __repl_main() }}\n",
                    definitions, test_body, input
                )
            } else {
                // Statement — just add to body
                format!(
                    "{}\nfn __repl_main() {{\n{}\n}}\nfn main() {{ __repl_main() }}\n",
                    definitions, test_body
                )
            };

            match execute_source(&source) {
                Ok(output) => {
                    eval_body.push_str(&format!("\n{}\n", input));
                    if !output.trim().is_empty() {
                        print!("{}", output);
                    }
                }
                Err(e) => {
                    // If auto-print failed as expression, try as statement
                    if is_expr {
                        let source2 = format!(
                            "{}\nfn __repl_main() {{\n{}\n}}\nfn main() {{ __repl_main() }}\n",
                            definitions, test_body
                        );
                        match execute_source(&source2) {
                            Ok(output) => {
                                eval_body.push_str(&format!("\n{}\n", input));
                                if !output.trim().is_empty() {
                                    print!("{}", output);
                                }
                            }
                            Err(e2) => {
                                println!("  Error: {} / {}", e, e2);
                            }
                        }
                    } else {
                        println!("  Error: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Build the full Sandbox source from definitions and eval body
fn build_full_source(definitions: &str, eval_body: &str) -> String {
    if eval_body.trim().is_empty() {
        format!("{}\nfn main() {{}}\n", definitions)
    } else {
        format!(
            "{}\nfn __repl_main() {{\n{}\n}}\nfn main() {{\n  __repl_main()\n}}\n",
            definitions, eval_body
        )
    }
}

/// Try to compile source without running — just check for errors
fn try_compile(source: &str) -> Result<()> {
    let compiler = Compiler::new(source, "<repl>");
    compiler.compile_quiet()?;
    Ok(())
}

/// Execute a source string by compiling via C backend and running
fn execute_source(source: &str) -> Result<String> {
    let compiler = Compiler::new(source, "<repl>");
    let c_code = compiler.compile_quiet()?;

    let id = format!(
        "repl_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let tmp_c = std::env::temp_dir().join(format!("{}.c", id));
    let tmp_bin = std::env::temp_dir().join(&id);

    std::fs::write(&tmp_c, &c_code)?;

    let status = std::process::Command::new("gcc")
        .arg("-o")
        .arg(&tmp_bin)
        .arg(&tmp_c)
        .arg("-lm")
        .arg("-Wno-incompatible-pointer-types")
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp_c);
        let _ = std::fs::remove_file(&tmp_bin);
        return Err(anyhow::anyhow!("Compilation failed"));
    }

    let output = std::process::Command::new(&tmp_bin).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let _ = std::fs::remove_file(&tmp_c);
    let _ = std::fs::remove_file(&tmp_bin);

    if !output.status.success() && !stderr.is_empty() {
        return Err(anyhow::anyhow!("Runtime error: {}", stderr.trim()));
    }

    Ok(stdout)
}
