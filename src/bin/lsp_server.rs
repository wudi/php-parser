use bumpalo::Bump;
use dashmap::DashMap;
use php_parser::ast::locator::AstNode;
use php_parser::ast::visitor::{Visitor, walk_class_member, walk_expr, walk_stmt};
use php_parser::ast::*;
use php_parser::lexer::Lexer;
use php_parser::line_index::LineIndex;
use php_parser::parser::Parser;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq)]
enum SymbolType {
    Definition,
    Reference,
}

#[derive(Debug, Clone)]
struct IndexEntry {
    uri: Url,
    range: Range,
    kind: SymbolType,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: DashMap<Url, String>,
    index: DashMap<String, Vec<IndexEntry>>,
    file_map: DashMap<Url, Vec<String>>,
    root_path: Arc<RwLock<Option<PathBuf>>>,
}

struct IndexingVisitor<'a> {
    entries: Vec<(String, Range, SymbolType)>,
    line_index: &'a LineIndex,
    source: &'a [u8],
}

impl<'a> IndexingVisitor<'a> {
    fn new(line_index: &'a LineIndex, source: &'a [u8]) -> Self {
        Self {
            entries: Vec::new(),
            line_index,
            source,
        }
    }

    fn add(&mut self, name: String, span: php_parser::span::Span, kind: SymbolType) {
        let start = self.line_index.line_col(span.start);
        let end = self.line_index.line_col(span.end);
        let range = Range {
            start: Position {
                line: start.0 as u32,
                character: start.1 as u32,
            },
            end: Position {
                line: end.0 as u32,
                character: end.1 as u32,
            },
        };
        self.entries.push((name, range, kind));
    }

    fn get_text(&self, span: php_parser::span::Span) -> String {
        String::from_utf8_lossy(&self.source[span.start..span.end]).to_string()
    }
}

impl<'a, 'ast> Visitor<'ast> for IndexingVisitor<'a> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt<'ast>) {
        match stmt {
            Stmt::Class {
                name,
                extends,
                implements,
                ..
            } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);

                if let Some(extends) = extends {
                    let ext_name = self.get_text(extends.span);
                    self.add(ext_name, extends.span, SymbolType::Reference);
                }
                for implement in *implements {
                    let imp_name = self.get_text(implement.span);
                    self.add(imp_name, implement.span, SymbolType::Reference);
                }
                walk_stmt(self, stmt);
            }
            Stmt::Function { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);
                walk_stmt(self, stmt);
            }
            Stmt::Interface { name, extends, .. } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);
                for extend in *extends {
                    let ext_name = self.get_text(extend.span);
                    self.add(ext_name, extend.span, SymbolType::Reference);
                }
                walk_stmt(self, stmt);
            }
            Stmt::Trait { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);
                walk_stmt(self, stmt);
            }
            Stmt::Enum {
                name, implements, ..
            } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);
                for implement in *implements {
                    let imp_name = self.get_text(implement.span);
                    self.add(imp_name, implement.span, SymbolType::Reference);
                }
                walk_stmt(self, stmt);
            }
            Stmt::Const { consts, .. } => {
                for c in *consts {
                    let name_str = self.get_text(c.name.span);
                    self.add(name_str, c.name.span, SymbolType::Definition);
                }
                walk_stmt(self, stmt);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        match expr {
            Expr::New { class, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *class
                {
                    let name_str = self.get_text(*name_span);
                    self.add(name_str, *name_span, SymbolType::Reference);
                }
                walk_expr(self, expr);
            }
            Expr::Call { func, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *func
                {
                    let name_str = self.get_text(*name_span);
                    self.add(name_str, *name_span, SymbolType::Reference);
                }
                walk_expr(self, expr);
            }
            Expr::StaticCall { class, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *class
                {
                    let name_str = self.get_text(*name_span);
                    self.add(name_str, *name_span, SymbolType::Reference);
                }
                walk_expr(self, expr);
            }
            Expr::ClassConstFetch { class, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *class
                {
                    let name_str = self.get_text(*name_span);
                    self.add(name_str, *name_span, SymbolType::Reference);
                }
                walk_expr(self, expr);
            }
            _ => walk_expr(self, expr),
        }
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Method { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);
                walk_class_member(self, member);
            }
            ClassMember::Property { entries, .. } => {
                for entry in *entries {
                    let name_str = self.get_text(entry.name.span);
                    self.add(name_str, entry.name.span, SymbolType::Definition);
                }
                walk_class_member(self, member);
            }
            ClassMember::Const { consts, .. } => {
                for c in *consts {
                    let name_str = self.get_text(c.name.span);
                    self.add(name_str, c.name.span, SymbolType::Definition);
                }
                walk_class_member(self, member);
            }
            ClassMember::Case { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add(name_str, name.span, SymbolType::Definition);
                walk_class_member(self, member);
            }
            _ => walk_class_member(self, member),
        }
    }
}

impl Backend {
    async fn update_index(&self, uri: Url, source: &[u8]) {
        // 1. Remove old entries for this file
        if let Some(old_symbols) = self.file_map.get(&uri) {
            for sym in old_symbols.iter() {
                if let Some(mut entries) = self.index.get_mut(sym) {
                    entries.retain(|e| e.uri != uri);
                }
            }
        }

        // 2. Parse and extract new entries
        let (diagnostics, new_entries) = {
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);
            let program = parser.parse_program();
            let line_index = LineIndex::new(source);

            // Publish diagnostics
            let diagnostics: Vec<Diagnostic> = program
                .errors
                .iter()
                .map(|e| {
                    let start = line_index.line_col(e.span.start);
                    let end = line_index.line_col(e.span.end);
                    Diagnostic {
                        range: Range {
                            start: Position {
                                line: start.0 as u32,
                                character: start.1 as u32,
                            },
                            end: Position {
                                line: end.0 as u32,
                                character: end.1 as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: Some("php-parser".to_string()),
                        message: e.message.to_string(),
                        related_information: None,
                        tags: None,
                        data: None,
                    }
                })
                .collect();

            let mut visitor = IndexingVisitor::new(&line_index, source);
            visitor.visit_program(&program);
            (diagnostics, visitor.entries)
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;

        // 3. Update index
        let mut new_symbols = Vec::new();
        for (name, range, kind) in new_entries {
            self.index
                .entry(name.clone())
                .or_default()
                .push(IndexEntry {
                    uri: uri.clone(),
                    range,
                    kind,
                });
            new_symbols.push(name);
        }

        self.file_map.insert(uri, new_symbols);
    }
}

struct DocumentSymbolVisitor<'a> {
    symbols: Vec<DocumentSymbol>,
    stack: Vec<DocumentSymbol>,
    line_index: &'a LineIndex,
    source: &'a [u8],
}

impl<'a> DocumentSymbolVisitor<'a> {
    fn new(line_index: &'a LineIndex, source: &'a [u8]) -> Self {
        Self {
            symbols: Vec::new(),
            stack: Vec::new(),
            line_index,
            source,
        }
    }

    fn range(&self, span: php_parser::span::Span) -> Range {
        let start = self.line_index.line_col(span.start);
        let end = self.line_index.line_col(span.end);
        Range {
            start: Position {
                line: start.0 as u32,
                character: start.1 as u32,
            },
            end: Position {
                line: end.0 as u32,
                character: end.1 as u32,
            },
        }
    }

    fn push_symbol(
        &mut self,
        mut name: String,
        kind: SymbolKind,
        span: php_parser::span::Span,
        mut selection_span: php_parser::span::Span,
    ) {
        if name.is_empty() {
            name = "<unknown>".to_string();
        }

        // Ensure selection_span is contained in span.
        // If the parser recovered with a default span (0..0) for the name,
        // it might be outside the statement's span.
        if selection_span.start < span.start || selection_span.end > span.end {
            selection_span = span;
        }

        let range = self.range(span);
        let selection_range = self.range(selection_span);

        #[allow(deprecated)]
        let symbol = DocumentSymbol {
            name,
            detail: None,
            kind,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: Some(Vec::new()),
        };

        self.stack.push(symbol);
    }

    fn pop_symbol(&mut self) {
        if let Some(symbol) = self.stack.pop() {
            if let Some(parent) = self.stack.last_mut() {
                parent.children.as_mut().unwrap().push(symbol);
            } else {
                self.symbols.push(symbol);
            }
        }
    }

    fn get_text(&self, span: php_parser::span::Span) -> String {
        String::from_utf8_lossy(&self.source[span.start..span.end]).to_string()
    }
}

impl<'a, 'ast> Visitor<'ast> for DocumentSymbolVisitor<'a> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt<'ast>) {
        match stmt {
            Stmt::Class {
                name,
                members,
                span,
                ..
            } => {
                let name_str = self.get_text(name.span);
                self.push_symbol(name_str, SymbolKind::CLASS, *span, name.span);
                for member in *members {
                    self.visit_class_member(member);
                }
                self.pop_symbol();
            }
            Stmt::Function {
                name,
                params: _,
                body,
                span,
                ..
            } => {
                let name_str = self.get_text(name.span);
                self.push_symbol(name_str, SymbolKind::FUNCTION, *span, name.span);
                for s in *body {
                    self.visit_stmt(s);
                }
                self.pop_symbol();
            }
            Stmt::Interface {
                name,
                members,
                span,
                ..
            } => {
                let name_str = self.get_text(name.span);
                self.push_symbol(name_str, SymbolKind::INTERFACE, *span, name.span);
                for member in *members {
                    self.visit_class_member(member);
                }
                self.pop_symbol();
            }
            Stmt::Trait {
                name,
                members,
                span,
                ..
            } => {
                let name_str = self.get_text(name.span);
                // SymbolKind::TRAIT is not in standard LSP 3.17? It is in 3.17.
                // But tower-lsp might use an older version or I need to check.
                // If not available, use INTERFACE.
                self.push_symbol(name_str, SymbolKind::INTERFACE, *span, name.span);
                for member in *members {
                    self.visit_class_member(member);
                }
                self.pop_symbol();
            }
            Stmt::Enum {
                name,
                members,
                span,
                ..
            } => {
                let name_str = self.get_text(name.span);
                self.push_symbol(name_str, SymbolKind::ENUM, *span, name.span);
                for member in *members {
                    self.visit_class_member(member);
                }
                self.pop_symbol();
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Method {
                name,
                params: _,
                body,
                span,
                ..
            } => {
                let name_str = self.get_text(name.span);
                self.push_symbol(name_str, SymbolKind::METHOD, *span, name.span);
                for s in *body {
                    self.visit_stmt(s);
                }
                self.pop_symbol();
            }
            ClassMember::Property { entries, .. } => {
                for entry in *entries {
                    let name_str = self.get_text(entry.name.span);
                    self.push_symbol(name_str, SymbolKind::PROPERTY, entry.span, entry.name.span);
                    self.pop_symbol();
                }
            }
            ClassMember::Const { consts, .. } => {
                for entry in *consts {
                    let name_str = self.get_text(entry.name.span);
                    self.push_symbol(name_str, SymbolKind::CONSTANT, entry.span, entry.name.span);
                    self.pop_symbol();
                }
            }
            ClassMember::Case { name, span, .. } => {
                let name_str = self.get_text(name.span);
                self.push_symbol(name_str, SymbolKind::ENUM_MEMBER, *span, name.span);
                self.pop_symbol();
            }
            _ => walk_class_member(self, member),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                {
                    let mut root = self.root_path.write().await;
                    *root = Some(path.clone());
                }

                let index = self.index.clone();
                let file_map = self.file_map.clone();
                let client = self.client.clone();

                // We need a way to call update_index from the spawned task.
                // Since update_index is on Backend, and we can't easily clone Backend into the task (it has Client which is cloneable, but DashMaps are too).
                // Actually Backend is just a struct of Arcs/DashMaps (which are Arc internally).
                // But we can't clone `self` easily if it's not Arc<Self>.
                // Let's just copy the logic or make update_index a standalone function or static method.
                // Or better, just inline the logic here since it's initialization.

                tokio::spawn(async move {
                    client
                        .log_message(MessageType::INFO, format!("Indexing {}", path.display()))
                        .await;
                    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                        if entry.path().extension().map_or(false, |ext| ext == "php") {
                            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                                let source = content.as_bytes();
                                let bump = Bump::new();
                                let lexer = Lexer::new(source);
                                let mut parser = Parser::new(lexer, &bump);
                                let program = parser.parse_program();
                                let line_index = LineIndex::new(source);

                                if let Ok(uri) = Url::from_file_path(entry.path()) {
                                    let mut visitor = IndexingVisitor::new(&line_index, source);
                                    visitor.visit_program(&program);

                                    let mut new_symbols = Vec::new();
                                    for (name, range, kind) in visitor.entries {
                                        index.entry(name.clone()).or_default().push(IndexEntry {
                                            uri: uri.clone(),
                                            range,
                                            kind,
                                        });
                                        new_symbols.push(name);
                                    }
                                    file_map.insert(uri, new_symbols);
                                }
                            } else {
                                client
                                    .log_message(
                                        MessageType::WARNING,
                                        format!("Failed to read file: {}", entry.path().display()),
                                    )
                                    .await;
                            }
                        }
                    }
                    client
                        .log_message(MessageType::INFO, "Indexing complete")
                        .await;
                });
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "PHP Parser LSP initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.documents.insert(
            params.text_document.uri.clone(),
            params.text_document.text.clone(),
        );
        self.update_index(
            params.text_document.uri,
            params.text_document.text.as_bytes(),
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.first() {
            self.documents
                .insert(params.text_document.uri.clone(), change.text.clone());
            self.update_index(params.text_document.uri, change.text.as_bytes())
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.documents.insert(uri.clone(), text.clone());
            self.update_index(uri, text.as_bytes()).await;
        } else {
            if let Some(text) = self.documents.get(&uri) {
                self.update_index(uri.clone(), text.as_bytes()).await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);

            let program = parser.parse_program();
            let line_index = LineIndex::new(source);
            let mut visitor = DocumentSymbolVisitor::new(&line_index, source);
            visitor.visit_program(&program);

            return Ok(Some(DocumentSymbolResponse::Nested(visitor.symbols)));
        }
        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let offset = line_index.offset(position.line as usize, position.character as usize);

            if let Some(offset) = offset {
                let bump = Bump::new();
                let lexer = Lexer::new(source);
                let mut parser = Parser::new(lexer, &bump);
                let program = parser.parse_program();

                let mut locator = php_parser::ast::locator::Locator::new(offset);
                locator.visit_program(&program);

                if let Some(node) = locator.path.last() {
                    match node {
                        AstNode::Expr(Expr::New { class, .. }) => {
                            if let Expr::Variable {
                                name: name_span, ..
                            } = *class
                            {
                                let target_name = String::from_utf8_lossy(
                                    &source[name_span.start..name_span.end],
                                );
                                for stmt in program.statements {
                                    if let Stmt::Class { name, span, .. } = stmt {
                                        let class_name = String::from_utf8_lossy(
                                            &source[name.span.start..name.span.end],
                                        );
                                        if class_name == target_name {
                                            let range = {
                                                let start = line_index.line_col(span.start);
                                                let end = line_index.line_col(span.end);
                                                Range {
                                                    start: Position {
                                                        line: start.0 as u32,
                                                        character: start.1 as u32,
                                                    },
                                                    end: Position {
                                                        line: end.0 as u32,
                                                        character: end.1 as u32,
                                                    },
                                                }
                                            };
                                            return Ok(Some(GotoDefinitionResponse::Scalar(
                                                Location {
                                                    uri: uri.clone(),
                                                    range,
                                                },
                                            )));
                                        }
                                    }
                                }

                                // Fallback to global index
                                if let Some(entries) = self.index.get(&target_name.to_string()) {
                                    for entry in entries.iter() {
                                        if entry.kind == SymbolType::Definition {
                                            return Ok(Some(GotoDefinitionResponse::Scalar(
                                                Location {
                                                    uri: entry.uri.clone(),
                                                    range: entry.range,
                                                },
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                        AstNode::Expr(Expr::Call { func, .. }) => {
                            if let Expr::Variable {
                                name: name_span, ..
                            } = *func
                            {
                                let target_name = String::from_utf8_lossy(
                                    &source[name_span.start..name_span.end],
                                );
                                for stmt in program.statements {
                                    if let Stmt::Function { name, span, .. } = stmt {
                                        let func_name = String::from_utf8_lossy(
                                            &source[name.span.start..name.span.end],
                                        );
                                        if func_name == target_name {
                                            let range = {
                                                let start = line_index.line_col(span.start);
                                                let end = line_index.line_col(span.end);
                                                Range {
                                                    start: Position {
                                                        line: start.0 as u32,
                                                        character: start.1 as u32,
                                                    },
                                                    end: Position {
                                                        line: end.0 as u32,
                                                        character: end.1 as u32,
                                                    },
                                                }
                                            };
                                            return Ok(Some(GotoDefinitionResponse::Scalar(
                                                Location {
                                                    uri: uri.clone(),
                                                    range,
                                                },
                                            )));
                                        }
                                    }
                                }

                                // Fallback to global index
                                if let Some(entries) = self.index.get(&target_name.to_string()) {
                                    for entry in entries.iter() {
                                        if entry.kind == SymbolType::Definition {
                                            return Ok(Some(GotoDefinitionResponse::Scalar(
                                                Location {
                                                    uri: entry.uri.clone(),
                                                    range: entry.range,
                                                },
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let mut target_name = String::new();

        // 1. Identify the symbol at the cursor
        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let offset = line_index.offset(position.line as usize, position.character as usize);

            if let Some(offset) = offset {
                let bump = Bump::new();
                let lexer = Lexer::new(source);
                let mut parser = Parser::new(lexer, &bump);
                let program = parser.parse_program();

                let mut locator = php_parser::ast::locator::Locator::new(offset);
                locator.visit_program(&program);

                if let Some(node) = locator.path.last() {
                    match node {
                        AstNode::Expr(Expr::New { class, .. }) => {
                            if let Expr::Variable {
                                name: name_span, ..
                            } = *class
                            {
                                target_name = String::from_utf8_lossy(
                                    &source[name_span.start..name_span.end],
                                )
                                .to_string();
                            }
                        }
                        AstNode::Expr(Expr::Call { func, .. }) => {
                            if let Expr::Variable {
                                name: name_span, ..
                            } = *func
                            {
                                target_name = String::from_utf8_lossy(
                                    &source[name_span.start..name_span.end],
                                )
                                .to_string();
                            }
                        }
                        AstNode::Stmt(Stmt::Class { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        AstNode::Stmt(Stmt::Function { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        AstNode::Stmt(Stmt::Interface { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        AstNode::Stmt(Stmt::Trait { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        AstNode::Stmt(Stmt::Enum { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        _ => {}
                    }
                }
            }
        }

        if target_name.is_empty() {
            return Ok(None);
        }

        let mut locations = Vec::new();

        // Use the persistent index
        if let Some(entries) = self.index.get(&target_name) {
            for entry in entries.iter() {
                // We want references, but maybe definitions too?
                // Usually "Find References" includes the definition.
                locations.push(Location {
                    uri: entry.uri.clone(),
                    range: entry.range,
                });
            }
        }

        Ok(Some(locations))
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut items = Vec::new();
        for entry in self.index.iter() {
            let name = entry.key();
            let locations = entry.value();

            // Check if any location is a Definition
            if locations.iter().any(|l| l.kind == SymbolType::Definition) {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some("Global Symbol".to_string()),
                    ..Default::default()
                });
            }
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let offset = line_index.offset(position.line as usize, position.character as usize);

            if let Some(offset) = offset {
                let bump = Bump::new();
                let lexer = Lexer::new(source);
                let mut parser = Parser::new(lexer, &bump);

                let program = parser.parse_program();
                let mut locator = php_parser::ast::locator::Locator::new(offset);
                locator.visit_program(&program);

                if let Some(node) = locator.path.last() {
                    let span = node.span();
                    let range = {
                        let start = line_index.line_col(span.start);
                        let end = line_index.line_col(span.end);
                        Range {
                            start: Position {
                                line: start.0 as u32,
                                character: start.1 as u32,
                            },
                            end: Position {
                                line: end.0 as u32,
                                character: end.1 as u32,
                            },
                        }
                    };

                    // Try to get doc comment
                    let doc_comment = match node {
                        AstNode::Stmt(stmt) => match stmt {
                            Stmt::Class { doc_comment, .. } => *doc_comment,
                            Stmt::Function { doc_comment, .. } => *doc_comment,
                            Stmt::Interface { doc_comment, .. } => *doc_comment,
                            Stmt::Trait { doc_comment, .. } => *doc_comment,
                            Stmt::Enum { doc_comment, .. } => *doc_comment,
                            _ => None,
                        },
                        AstNode::ClassMember(member) => match member {
                            ClassMember::Method { doc_comment, .. } => *doc_comment,
                            ClassMember::Property { doc_comment, .. } => *doc_comment,
                            ClassMember::PropertyHook { doc_comment, .. } => *doc_comment,
                            ClassMember::Const { doc_comment, .. } => *doc_comment,
                            ClassMember::TraitUse { doc_comment, .. } => *doc_comment,
                            ClassMember::Case { doc_comment, .. } => *doc_comment,
                        },
                        _ => None,
                    };

                    if let Some(doc_span) = doc_comment {
                        let doc_text =
                            String::from_utf8_lossy(&source[doc_span.start..doc_span.end])
                                .to_string();
                        // Clean up doc comment (remove /**, */, *)
                        let clean_doc = doc_text
                            .lines()
                            .map(|line| line.trim())
                            .map(|line| {
                                line.trim_start_matches("/**")
                                    .trim_start_matches("*/")
                                    .trim_start_matches('*')
                                    .trim()
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        return Ok(Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::String(clean_doc)),
                            range: Some(range),
                        }));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: DashMap::new(),
        index: DashMap::new(),
        file_map: DashMap::new(),
        root_path: Arc::new(RwLock::new(None)),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
