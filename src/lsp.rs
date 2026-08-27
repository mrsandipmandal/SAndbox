use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typechecker::TypeChecker;
use lsp_server::{Connection, Message, Notification};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, Diagnostic, DiagnosticSeverity,
    DidOpenTextDocumentParams, HoverContents, HoverProviderCapability, InitializeParams,
    MarkupKind, OneOf, Position, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn run_lsp() -> anyhow::Result<()> {
    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        ..Default::default()
    };

    let initialize_params: InitializeParams = serde_json::from_value(
        connection.initialize(serde_json::to_value(&server_capabilities)?)?,
    )?;
    #[allow(deprecated)]
    let _root_path = initialize_params.root_uri.map(|u| u.path().to_string());

    let documents: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    eprintln!("[sandbox-lsp] Server started");

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                match req.method.as_str() {
                    "textDocument/completion" => {
                        let items = get_completions();
                        let response = Response::new_ok(req.id, items);
                        connection.sender.send(Message::Response(response))?;
                    }
                    "textDocument/hover" => {
                        let response =
                            Response::new_ok(req.id, serde_json::to_value(get_hover_info()).ok());
                        connection.sender.send(Message::Response(response))?;
                    }
                    "textDocument/definition" => {
                        let response = Response::new_ok(req.id, serde_json::Value::Null);
                        connection.sender.send(Message::Response(response))?;
                    }
                    _ => {}
                }
            }
            Message::Response(_) => {}
            Message::Notification(not) => {
                if not.method == "textDocument/didOpen" {
                    if let Ok(params) =
                        serde_json::from_value::<DidOpenTextDocumentParams>(not.params)
                    {
                        let uri = params.text_document.uri.to_string();
                        let text = params.text_document.text;
                        let diagnostics = analyze_source(&text);
                        documents.lock().unwrap().insert(uri.clone(), text);

                        if let Ok(parsed_uri) = uri.parse() {
                            let notification = Notification::new(
                                "textDocument/publishDiagnostics".to_string(),
                                PublishDiagnosticsParams {
                                    uri: parsed_uri,
                                    diagnostics,
                                    version: Some(0),
                                },
                            );
                            let _ = connection.sender.send(Message::Notification(notification));
                        }
                    }
                }
            }
        }
    }

    io_threads.join()?;
    Ok(())
}

fn analyze_source(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    match Lexer::new(source).tokenize() {
        Ok(tokens) => match Parser::new(tokens).parse() {
            Ok(program) => {
                let mut checker = TypeChecker::new();
                if let Err(e) = checker.check(&program) {
                    let msg = e.to_string();
                    let (line, col) = parse_error_location(&msg);
                    diagnostics.push(Diagnostic {
                        range: lsp_types::Range {
                            start: Position {
                                line,
                                character: col,
                            },
                            end: Position {
                                line,
                                character: col + 20,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: msg,
                        ..Default::default()
                    });
                }
            }
            Err(e) => {
                let msg = e.to_string();
                let (line, col) = parse_error_location(&msg);
                diagnostics.push(Diagnostic {
                    range: lsp_types::Range {
                        start: Position {
                            line,
                            character: col,
                        },
                        end: Position {
                            line,
                            character: col + 20,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: msg,
                    ..Default::default()
                });
            }
        },
        Err(e) => {
            let msg = e.to_string();
            let (line, col) = parse_error_location(&msg);
            diagnostics.push(Diagnostic {
                range: lsp_types::Range {
                    start: Position {
                        line,
                        character: col,
                    },
                    end: Position {
                        line,
                        character: col + 20,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: msg,
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn parse_error_location(msg: &str) -> (u32, u32) {
    if let Some(pos) = msg.find(" at ") {
        let rest = &msg[pos + 4..];
        if let Some(colon) = rest.find(':') {
            if let Ok(line) = rest[..colon].parse::<u32>() {
                let after_colon = &rest[colon + 1..];
                if let Some(end) = after_colon.find(|c: char| !c.is_ascii_digit()) {
                    if let Ok(col) = after_colon[..end].parse::<u32>() {
                        return (line.saturating_sub(1), col.saturating_sub(1));
                    }
                }
            }
        }
    }
    (0, 0)
}

fn get_completions() -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let keywords = vec![
        ("fn", "Function definition"),
        ("let", "Variable declaration"),
        ("mut", "Mutable variable"),
        ("if", "If expression"),
        ("else", "Else branch"),
        ("while", "While loop"),
        ("for", "For loop"),
        ("return", "Return value"),
        ("print", "Print function"),
        ("struct", "Struct definition"),
        ("mod", "Module definition"),
        ("use", "Import module"),
        ("ledger", "Ledger definition"),
        ("database", "Database definition"),
        ("table", "Table definition"),
        ("query", "Query definition"),
        ("select", "SQL SELECT"),
        ("insert", "SQL INSERT"),
        ("update", "SQL UPDATE"),
        ("delete", "SQL DELETE"),
        ("where", "SQL WHERE"),
        ("from", "SQL FROM"),
        ("into", "SQL INTO"),
        ("values", "SQL VALUES"),
        ("set", "SQL SET"),
        ("debit", "Ledger debit"),
        ("credit", "Ledger credit"),
    ];

    for (kw, desc) in keywords {
        items.push(CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(desc.to_string()),
            ..Default::default()
        });
    }

    let types = vec![
        ("i64", "64-bit integer"),
        ("f64", "64-bit float"),
        ("bool", "Boolean"),
        ("string", "String"),
        ("Money<INR>", "Indian Rupee"),
        ("Money<USD>", "US Dollar"),
        ("Money<EUR>", "Euro"),
        ("Decimal", "Exact decimal"),
        ("Result", "Result type"),
    ];

    for (ty, desc) in types {
        items.push(CompletionItem {
            label: ty.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some(desc.to_string()),
            ..Default::default()
        });
    }

    let stdlib_fns = vec![
        "math::abs",
        "math::max",
        "math::min",
        "math::sqrt",
        "math::pow",
        "math::floor",
        "math::ceil",
        "string::length",
        "string::concat",
        "string::substring",
        "string::equals",
        "string::trim",
        "string::starts_with",
        "string::contains",
        "string::find",
        "array::len",
        "array::push",
        "array::sort",
    ];

    for func in stdlib_fns {
        items.push(CompletionItem {
            label: func.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("Standard library".to_string()),
            ..Default::default()
        });
    }

    items
}

fn get_hover_info() -> HoverContents {
    HoverContents::Markup(lsp_types::MarkupContent {
        kind: MarkupKind::Markdown,
        value: "**Sandbox** v1.0.0 — A memory-safe, financially-safe, general-purpose language"
            .to_string(),
    })
}

use lsp_server::Response;
