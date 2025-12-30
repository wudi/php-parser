use bumpalo::Bump;
use dashmap::DashMap;
use php_parser::ast::locator::AstNode;
use php_parser::ast::visitor::{Visitor, walk_class_member, walk_expr, walk_stmt};
use php_parser::ast::*;
use php_parser::lexer::Lexer;
use php_parser::line_index::LineIndex;
use php_parser::parser::Parser;
use php_parser::span::Span;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
};
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
    symbol_kind: Option<SymbolKind>,
    parameters: Option<Vec<String>>,
    extends: Option<Vec<String>>,
    implements: Option<Vec<String>>,
    type_info: Option<String>,
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
    entries: Vec<(
        String,
        Range,
        SymbolType,
        Option<SymbolKind>,
        Option<Vec<String>>,
        Option<Vec<String>>,
        Option<Vec<String>>,
        Option<String>,
    )>,
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

    fn add(
        &mut self,
        name: String,
        span: php_parser::span::Span,
        kind: SymbolType,
        symbol_kind: Option<SymbolKind>,
        parameters: Option<Vec<String>>,
        extends: Option<Vec<String>>,
        implements: Option<Vec<String>>,
        type_info: Option<String>,
    ) {
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
        self.entries.push((
            name,
            range,
            kind,
            symbol_kind,
            parameters,
            extends,
            implements,
            type_info,
        ));
    }

    fn get_text(&self, span: php_parser::span::Span) -> String {
        String::from_utf8_lossy(&self.source[span.start..span.end]).to_string()
    }

    fn get_type_text(&self, ty: &Type) -> String {
        match ty {
            Type::Simple(token) => self.get_text(token.span),
            Type::Name(name) => self.get_text(name.span),
            Type::Union(types) => types
                .iter()
                .map(|t| self.get_type_text(t))
                .collect::<Vec<_>>()
                .join("|"),
            Type::Intersection(types) => types
                .iter()
                .map(|t| self.get_type_text(t))
                .collect::<Vec<_>>()
                .join("&"),
            Type::Nullable(ty) => format!("?{}", self.get_type_text(ty)),
        }
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

                let extends_vec = extends.map(|e| vec![self.get_text(e.span)]);
                let implements_vec = if implements.is_empty() {
                    None
                } else {
                    Some(implements.iter().map(|i| self.get_text(i.span)).collect())
                };

                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::CLASS),
                    None,
                    extends_vec,
                    implements_vec,
                    None,
                );

                if let Some(extends) = extends {
                    let ext_name = self.get_text(extends.span);
                    self.add(
                        ext_name,
                        extends.span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                for implement in *implements {
                    let imp_name = self.get_text(implement.span);
                    self.add(
                        imp_name,
                        implement.span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_stmt(self, stmt);
            }
            Stmt::Function {
                name,
                params,
                return_type,
                ..
            } => {
                let name_str = self.get_text(name.span);
                let parameters = params.iter().map(|p| self.get_text(p.name.span)).collect();
                let type_info = return_type.as_ref().map(|t| self.get_type_text(t));
                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::FUNCTION),
                    Some(parameters),
                    None,
                    None,
                    type_info,
                );
                walk_stmt(self, stmt);
            }
            Stmt::Interface { name, extends, .. } => {
                let name_str = self.get_text(name.span);

                let extends_vec = if extends.is_empty() {
                    None
                } else {
                    Some(extends.iter().map(|e| self.get_text(e.span)).collect())
                };

                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::INTERFACE),
                    None,
                    extends_vec,
                    None,
                    None,
                );
                for extend in *extends {
                    let ext_name = self.get_text(extend.span);
                    self.add(
                        ext_name,
                        extend.span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_stmt(self, stmt);
            }
            Stmt::Trait { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::INTERFACE),
                    None,
                    None,
                    None,
                    None,
                );
                walk_stmt(self, stmt);
            }
            Stmt::Enum {
                name, implements, ..
            } => {
                let name_str = self.get_text(name.span);

                let implements_vec = if implements.is_empty() {
                    None
                } else {
                    Some(implements.iter().map(|i| self.get_text(i.span)).collect())
                };

                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::ENUM),
                    None,
                    None,
                    implements_vec,
                    None,
                );
                for implement in *implements {
                    let imp_name = self.get_text(implement.span);
                    self.add(
                        imp_name,
                        implement.span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_stmt(self, stmt);
            }
            Stmt::Const { consts, .. } => {
                for c in *consts {
                    let name_str = self.get_text(c.name.span);
                    self.add(
                        name_str,
                        c.name.span,
                        SymbolType::Definition,
                        Some(SymbolKind::CONSTANT),
                        None,
                        None,
                        None,
                        None,
                    );
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
                    self.add(
                        name_str,
                        *name_span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_expr(self, expr);
            }
            Expr::Call { func, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *func
                {
                    let name_str = self.get_text(*name_span);
                    self.add(
                        name_str,
                        *name_span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_expr(self, expr);
            }
            Expr::StaticCall { class, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *class
                {
                    let name_str = self.get_text(*name_span);
                    self.add(
                        name_str,
                        *name_span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_expr(self, expr);
            }
            Expr::ClassConstFetch { class, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *class
                {
                    let name_str = self.get_text(*name_span);
                    self.add(
                        name_str,
                        *name_span,
                        SymbolType::Reference,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_expr(self, expr);
            }
            _ => walk_expr(self, expr),
        }
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Method {
                name,
                params,
                return_type,
                ..
            } => {
                let name_str = self.get_text(name.span);
                let parameters = params.iter().map(|p| self.get_text(p.name.span)).collect();
                let type_info = return_type.as_ref().map(|t| self.get_type_text(t));
                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::METHOD),
                    Some(parameters),
                    None,
                    None,
                    type_info,
                );
                walk_class_member(self, member);
            }
            ClassMember::Property { entries, ty, .. } => {
                let type_info = ty.as_ref().map(|t| self.get_type_text(t));
                for entry in *entries {
                    let name_str = self.get_text(entry.name.span);
                    self.add(
                        name_str,
                        entry.name.span,
                        SymbolType::Definition,
                        Some(SymbolKind::PROPERTY),
                        None,
                        None,
                        None,
                        type_info.clone(),
                    );
                }
                walk_class_member(self, member);
            }
            ClassMember::Const { consts, .. } => {
                for c in *consts {
                    let name_str = self.get_text(c.name.span);
                    self.add(
                        name_str,
                        c.name.span,
                        SymbolType::Definition,
                        Some(SymbolKind::CONSTANT),
                        None,
                        None,
                        None,
                        None,
                    );
                }
                walk_class_member(self, member);
            }
            ClassMember::Case { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add(
                    name_str,
                    name.span,
                    SymbolType::Definition,
                    Some(SymbolKind::ENUM_MEMBER),
                    None,
                    None,
                    None,
                    None,
                );
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
                .map(|e: &ParseError| {
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
                        source: Some("pls".to_string()),
                        message: e.to_human_readable(source),
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
        for (name, range, kind, symbol_kind, parameters, extends, implements, type_info) in
            new_entries
        {
            self.index
                .entry(name.clone())
                .or_default()
                .push(IndexEntry {
                    uri: uri.clone(),
                    range,
                    kind,
                    symbol_kind,
                    parameters,
                    extends,
                    implements,
                    type_info,
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

struct FoldingRangeVisitor<'a> {
    ranges: Vec<FoldingRange>,
    line_index: &'a LineIndex,
}

impl<'a> FoldingRangeVisitor<'a> {
    fn new(line_index: &'a LineIndex) -> Self {
        Self {
            ranges: Vec::new(),
            line_index,
        }
    }

    fn add_range(&mut self, span: php_parser::span::Span, kind: Option<FoldingRangeKind>) {
        let start = self.line_index.line_col(span.start);
        let end = self.line_index.line_col(span.end);

        if start.0 < end.0 {
            self.ranges.push(FoldingRange {
                start_line: start.0 as u32,
                start_character: Some(start.1 as u32),
                end_line: end.0 as u32,
                end_character: Some(end.1 as u32),
                kind,
                collapsed_text: None,
            });
        }
    }
}

impl<'a, 'ast> Visitor<'ast> for FoldingRangeVisitor<'a> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt<'ast>) {
        match stmt {
            Stmt::Class { span, .. }
            | Stmt::Function { span, .. }
            | Stmt::Interface { span, .. }
            | Stmt::Trait { span, .. }
            | Stmt::Enum { span, .. } => {
                self.add_range(*span, Some(FoldingRangeKind::Region));
                walk_stmt(self, stmt);
            }
            Stmt::Block { span, .. } => {
                self.add_range(*span, Some(FoldingRangeKind::Region));
                walk_stmt(self, stmt);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Method { span, .. } => {
                self.add_range(*span, Some(FoldingRangeKind::Region));
                walk_class_member(self, member);
            }
            _ => walk_class_member(self, member),
        }
    }
}

struct DocumentLinkVisitor<'a> {
    links: Vec<DocumentLink>,
    line_index: &'a LineIndex,
    source: &'a [u8],
    base_url: Url,
}

impl<'a> DocumentLinkVisitor<'a> {
    fn new(line_index: &'a LineIndex, source: &'a [u8], base_url: Url) -> Self {
        Self {
            links: Vec::new(),
            line_index,
            source,
            base_url,
        }
    }

    fn get_text(&self, span: php_parser::span::Span) -> String {
        String::from_utf8_lossy(&self.source[span.start..span.end]).to_string()
    }
}

impl<'a, 'ast> Visitor<'ast> for DocumentLinkVisitor<'a> {
    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        match expr {
            Expr::Include {
                expr: path_expr, ..
            } => {
                if let Expr::String { span, .. } = path_expr {
                    let raw_text = self.get_text(*span);
                    let path_str = raw_text.trim_matches(|c| c == '"' || c == '\'');

                    if let Ok(target) = self.base_url.join(path_str) {
                        let start = self.line_index.line_col(span.start);
                        let end = self.line_index.line_col(span.end);

                        self.links.push(DocumentLink {
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
                            target: Some(target),
                            tooltip: Some(path_str.to_string()),
                            data: None,
                        });
                    }
                }
                walk_expr(self, expr);
            }
            _ => walk_expr(self, expr),
        }
    }
}

struct CodeLensVisitor<'a> {
    lenses: Vec<CodeLens>,
    line_index: &'a LineIndex,
    source: &'a [u8],
    index: &'a DashMap<String, Vec<IndexEntry>>,
}

impl<'a> CodeLensVisitor<'a> {
    fn new(
        line_index: &'a LineIndex,
        source: &'a [u8],
        index: &'a DashMap<String, Vec<IndexEntry>>,
    ) -> Self {
        Self {
            lenses: Vec::new(),
            line_index,
            source,
            index,
        }
    }

    fn get_text(&self, span: php_parser::span::Span) -> String {
        String::from_utf8_lossy(&self.source[span.start..span.end]).to_string()
    }

    fn add_lens(&mut self, name: String, span: php_parser::span::Span) {
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

        let mut count = 0;
        if let Some(entries) = self.index.get(&name) {
            count = entries
                .iter()
                .filter(|e| e.kind == SymbolType::Reference)
                .count();
        }

        let title = if count == 1 {
            "1 reference".to_string()
        } else {
            format!("{} references", count)
        };

        self.lenses.push(CodeLens {
            range,
            command: Some(Command {
                title,
                command: "".to_string(),
                arguments: None,
            }),
            data: None,
        });
    }
}

impl<'a, 'ast> Visitor<'ast> for CodeLensVisitor<'a> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt<'ast>) {
        match stmt {
            Stmt::Class { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add_lens(name_str, name.span);
                walk_stmt(self, stmt);
            }
            Stmt::Function { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add_lens(name_str, name.span);
                walk_stmt(self, stmt);
            }
            Stmt::Interface { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add_lens(name_str, name.span);
                walk_stmt(self, stmt);
            }
            Stmt::Trait { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add_lens(name_str, name.span);
                walk_stmt(self, stmt);
            }
            Stmt::Enum { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add_lens(name_str, name.span);
                walk_stmt(self, stmt);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Method { name, .. } => {
                let name_str = self.get_text(name.span);
                self.add_lens(name_str, name.span);
                walk_class_member(self, member);
            }
            _ => walk_class_member(self, member),
        }
    }
}

struct InlayHintVisitor<'a> {
    hints: Vec<InlayHint>,
    line_index: &'a LineIndex,
    source: &'a [u8],
    index: &'a DashMap<String, Vec<IndexEntry>>,
}

impl<'a> InlayHintVisitor<'a> {
    fn new(
        line_index: &'a LineIndex,
        source: &'a [u8],
        index: &'a DashMap<String, Vec<IndexEntry>>,
    ) -> Self {
        Self {
            hints: Vec::new(),
            line_index,
            source,
            index,
        }
    }

    fn get_text(&self, span: php_parser::span::Span) -> String {
        String::from_utf8_lossy(&self.source[span.start..span.end]).to_string()
    }
}

impl<'a, 'ast> Visitor<'ast> for InlayHintVisitor<'a> {
    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        match expr {
            Expr::Call { func, args, .. } => {
                if let Expr::Variable {
                    name: name_span, ..
                } = *func
                {
                    let name_str = self.get_text(*name_span);
                    if let Some(entries) = self.index.get(&name_str) {
                        if let Some(entry) = entries
                            .iter()
                            .find(|e| e.kind == SymbolType::Definition && e.parameters.is_some())
                        {
                            if let Some(params) = &entry.parameters {
                                for (i, arg) in args.iter().enumerate() {
                                    if i < params.len() {
                                        if let Some(arg_name) = arg.name {
                                            let arg_name_str = self.get_text(arg_name.span);
                                            if arg_name_str == params[i] {
                                                continue;
                                            }
                                        }

                                        let param_name = &params[i];
                                        let start = self.line_index.line_col(arg.span.start);

                                        self.hints.push(InlayHint {
                                            position: Position {
                                                line: start.0 as u32,
                                                character: start.1 as u32,
                                            },
                                            label: InlayHintLabel::String(format!(
                                                "{}:",
                                                param_name
                                            )),
                                            kind: Some(InlayHintKind::PARAMETER),
                                            text_edits: None,
                                            tooltip: None,
                                            padding_left: None,
                                            padding_right: Some(true),
                                            data: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                walk_expr(self, expr);
            }
            _ => walk_expr(self, expr),
        }
    }
}

struct Formatter<'a> {
    source: &'a [u8],
    line_index: &'a LineIndex,
    indent_unit: &'a str,
}

impl<'a> Formatter<'a> {
    fn new(source: &'a [u8], line_index: &'a LineIndex) -> Self {
        Self {
            source,
            line_index,
            indent_unit: "    ",
        }
    }

    fn format(&self) -> Vec<TextEdit> {
        use php_parser::lexer::token::TokenKind;

        let mut edits = Vec::new();
        let mut lexer = Lexer::new(self.source);
        let mut indent_level: usize = 0;
        let mut last_token_end = 0;
        let mut safety_counter = 0;

        while let Some(token) = lexer.next() {
            safety_counter += 1;
            if safety_counter > 100000 {
                break;
            }
            // Check gap
            let gap_start = last_token_end;
            let gap_end = token.span.start;

            if gap_end > gap_start {
                let gap = &self.source[gap_start..gap_end];
                // Check if gap contains newline
                if gap.contains(&b'\n') {
                    // Calculate indent
                    let mut current_indent = indent_level;
                    match token.kind {
                        TokenKind::CloseBrace | TokenKind::CloseBracket | TokenKind::CloseParen => {
                            current_indent = current_indent.saturating_sub(1);
                        }
                        _ => {}
                    }

                    // Create edit
                    let newlines = gap.iter().filter(|&&b| b == b'\n').count();
                    let mut new_text = String::new();
                    for _ in 0..newlines {
                        new_text.push('\n');
                    }
                    for _ in 0..current_indent {
                        new_text.push_str(self.indent_unit);
                    }

                    let start_pos = self.line_index.line_col(gap_start);
                    let end_pos = self.line_index.line_col(gap_end);

                    edits.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: start_pos.0 as u32,
                                character: start_pos.1 as u32,
                            },
                            end: Position {
                                line: end_pos.0 as u32,
                                character: end_pos.1 as u32,
                            },
                        },
                        new_text,
                    });
                }
            }

            match token.kind {
                TokenKind::OpenBrace
                | TokenKind::OpenBracket
                | TokenKind::OpenParen
                | TokenKind::Attribute
                | TokenKind::CurlyOpen
                | TokenKind::DollarOpenCurlyBraces => {
                    indent_level += 1;
                }
                TokenKind::CloseBrace | TokenKind::CloseBracket | TokenKind::CloseParen => {
                    indent_level = indent_level.saturating_sub(1);
                }
                _ => {}
            }

            last_token_end = token.span.end;
        }

        edits
    }
}

impl Backend {
    async fn type_hierarchy_supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let item = params.item;
        let name = item.name;

        let mut parents = Vec::new();

        if let Some(entries) = self.index.get(&name) {
            for entry in entries.iter() {
                if entry.kind == SymbolType::Definition {
                    if let Some(extends) = &entry.extends {
                        for parent_name in extends {
                            if let Some(parent_entries) = self.index.get(parent_name) {
                                for parent_entry in parent_entries.iter() {
                                    if parent_entry.kind == SymbolType::Definition {
                                        parents.push(TypeHierarchyItem {
                                            name: parent_name.clone(),
                                            kind: parent_entry
                                                .symbol_kind
                                                .unwrap_or(SymbolKind::CLASS),
                                            tags: None,
                                            detail: parent_entry.type_info.clone(),
                                            uri: parent_entry.uri.clone(),
                                            range: parent_entry.range,
                                            selection_range: parent_entry.range,
                                            data: Some(serde_json::json!({ "name": parent_name })),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    if let Some(implements) = &entry.implements {
                        for parent_name in implements {
                            if let Some(parent_entries) = self.index.get(parent_name) {
                                for parent_entry in parent_entries.iter() {
                                    if parent_entry.kind == SymbolType::Definition {
                                        parents.push(TypeHierarchyItem {
                                            name: parent_name.clone(),
                                            kind: parent_entry
                                                .symbol_kind
                                                .unwrap_or(SymbolKind::INTERFACE),
                                            tags: None,
                                            detail: parent_entry.type_info.clone(),
                                            uri: parent_entry.uri.clone(),
                                            range: parent_entry.range,
                                            selection_range: parent_entry.range,
                                            data: Some(serde_json::json!({ "name": parent_name })),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if parents.is_empty() {
            Ok(None)
        } else {
            Ok(Some(parents))
        }
    }

    async fn type_hierarchy_subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let item = params.item;
        let name = item.name;

        let mut children = Vec::new();

        for entry in self.index.iter() {
            let child_name = entry.key();
            for child_entry in entry.value() {
                if child_entry.kind == SymbolType::Definition {
                    let mut is_child = false;
                    if let Some(extends) = &child_entry.extends {
                        if extends.contains(&name) {
                            is_child = true;
                        }
                    }
                    if let Some(implements) = &child_entry.implements {
                        if implements.contains(&name) {
                            is_child = true;
                        }
                    }

                    if is_child {
                        children.push(TypeHierarchyItem {
                            name: child_name.clone(),
                            kind: child_entry.symbol_kind.unwrap_or(SymbolKind::CLASS),
                            tags: None,
                            detail: child_entry.type_info.clone(),
                            uri: child_entry.uri.clone(),
                            range: child_entry.range,
                            selection_range: child_entry.range,
                            data: Some(serde_json::json!({ "name": child_name })),
                        });
                    }
                }
            }
        }

        if children.is_empty() {
            Ok(None)
        } else {
            Ok(Some(children))
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
                                    for (
                                        name,
                                        range,
                                        kind,
                                        symbol_kind,
                                        parameters,
                                        extends,
                                        implements,
                                        type_info,
                                    ) in visitor.entries
                                    {
                                        index.entry(name.clone()).or_default().push(IndexEntry {
                                            uri: uri.clone(),
                                            range,
                                            kind,
                                            symbol_kind,
                                            parameters,
                                            extends,
                                            implements,
                                            type_info,
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
                workspace_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: Default::default(),
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                experimental: Some(serde_json::json!({
                    "typeHierarchyProvider": true
                })),
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

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let mut target_name = String::new();

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
                        AstNode::Stmt(Stmt::Class { name, .. }) => {
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
                        _ => {}
                    }
                }
            }
        }

        if target_name.is_empty() {
            return Ok(None);
        }

        let mut items = Vec::new();
        if let Some(entries) = self.index.get(&target_name) {
            for entry in entries.iter() {
                if entry.kind == SymbolType::Definition {
                    items.push(TypeHierarchyItem {
                        name: target_name.clone(),
                        kind: entry.symbol_kind.unwrap_or(SymbolKind::CLASS),
                        tags: None,
                        detail: entry.type_info.clone(),
                        uri: entry.uri.clone(),
                        range: entry.range,
                        selection_range: entry.range,
                        data: Some(serde_json::json!({ "name": target_name })),
                    });
                }
            }
        }

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
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

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let mut symbols = Vec::new();

        for entry in self.index.iter() {
            let name = entry.key();
            if name.to_lowercase().contains(&query) {
                for index_entry in entry.value() {
                    if index_entry.kind == SymbolType::Definition {
                        #[allow(deprecated)]
                        symbols.push(SymbolInformation {
                            name: name.clone(),
                            kind: index_entry.symbol_kind.unwrap_or(SymbolKind::VARIABLE),
                            tags: None,
                            deprecated: None,
                            location: Location {
                                uri: index_entry.uri.clone(),
                                range: index_entry.range,
                            },
                            container_name: None,
                        });
                    }
                }
            }
        }

        Ok(Some(symbols))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);
            let program = parser.parse_program();
            let line_index = LineIndex::new(source);

            let mut visitor = FoldingRangeVisitor::new(&line_index);
            visitor.visit_program(&program);

            return Ok(Some(visitor.ranges));
        }
        Ok(None)
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);
            let program = parser.parse_program();

            let mut result = Vec::new();

            for position in params.positions {
                let offset = line_index.offset(position.line as usize, position.character as usize);
                if let Some(offset) = offset {
                    let mut locator = php_parser::ast::locator::Locator::new(offset);
                    locator.visit_program(&program);

                    let mut current: Option<Box<SelectionRange>> = None;

                    for node in locator.path.iter() {
                        let span = node.span();
                        let start = line_index.line_col(span.start);
                        let end = line_index.line_col(span.end);
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

                        let selection_range = SelectionRange {
                            range,
                            parent: current,
                        };
                        current = Some(Box::new(selection_range));
                    }

                    if let Some(r) = current {
                        result.push(*r);
                    }
                }
            }
            return Ok(Some(result));
        }
        Ok(None)
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

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

        let mut highlights = Vec::new();
        if let Some(entries) = self.index.get(&target_name) {
            for entry in entries.iter() {
                if entry.uri == uri {
                    highlights.push(DocumentHighlight {
                        range: entry.range,
                        kind: Some(match entry.kind {
                            SymbolType::Definition => DocumentHighlightKind::WRITE,
                            SymbolType::Reference => DocumentHighlightKind::READ,
                        }),
                    });
                }
            }
        }

        Ok(Some(highlights))
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

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        // For now, declaration is same as definition
        let def_params = GotoDefinitionParams {
            text_document_position_params: params.text_document_position_params,
            work_done_progress_params: params.work_done_progress_params,
            partial_result_params: params.partial_result_params,
        };
        match self.goto_definition(def_params).await? {
            Some(GotoDefinitionResponse::Scalar(loc)) => {
                Ok(Some(GotoDeclarationResponse::Scalar(loc)))
            }
            Some(GotoDefinitionResponse::Array(locs)) => {
                Ok(Some(GotoDeclarationResponse::Array(locs)))
            }
            Some(GotoDefinitionResponse::Link(links)) => {
                Ok(Some(GotoDeclarationResponse::Link(links)))
            }
            None => Ok(None),
        }
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let mut target_name = String::new();

        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let cursor_offset =
                line_index.offset(position.line as usize, position.character as usize);

            if let Some(cursor_offset) = cursor_offset {
                let bump = Bump::new();
                let lexer = Lexer::new(source);
                let mut parser = Parser::new(lexer, &bump);
                let program = parser.parse_program();

                let path = php_parser::ast::locator::Locator::find(&program, cursor_offset);

                if let Some(node) = path.last() {
                    match node {
                        AstNode::Expr(Expr::New { class, .. }) => {
                            target_name = String::from_utf8_lossy(
                                &source[class.span().start..class.span().end],
                            )
                            .to_string();
                        }
                        AstNode::Expr(Expr::ClassConstFetch { class, .. }) => {
                            target_name = String::from_utf8_lossy(
                                &source[class.span().start..class.span().end],
                            )
                            .to_string();
                        }
                        AstNode::Expr(Expr::StaticCall { class, .. }) => {
                            target_name = String::from_utf8_lossy(
                                &source[class.span().start..class.span().end],
                            )
                            .to_string();
                        }
                        AstNode::Stmt(Stmt::Class { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        AstNode::Stmt(Stmt::Interface { name, .. }) => {
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

        for entry in self.index.iter() {
            for ie in entry.value() {
                if ie.kind == SymbolType::Definition {
                    let mut is_impl = false;
                    if let Some(extends) = &ie.extends {
                        if extends.contains(&target_name) {
                            is_impl = true;
                        }
                    }
                    if let Some(implements) = &ie.implements {
                        if implements.contains(&target_name) {
                            is_impl = true;
                        }
                    }

                    if is_impl {
                        locations.push(Location {
                            uri: ie.uri.clone(),
                            range: ie.range,
                        });
                    }
                }
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(GotoImplementationResponse::Array(locations)))
        }
    }

    async fn goto_type_definition(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> Result<Option<GotoTypeDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let mut target_type_name = String::new();

        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let cursor_offset =
                line_index.offset(position.line as usize, position.character as usize);

            if let Some(cursor_offset) = cursor_offset {
                let bump = Bump::new();
                let lexer = Lexer::new(source);
                let mut parser = Parser::new(lexer, &bump);
                let program = parser.parse_program();

                let path = php_parser::ast::locator::Locator::find(&program, cursor_offset);

                fn get_type_text(ty: &Type, source: &[u8]) -> String {
                    let get_text = |span: Span| -> String {
                        String::from_utf8_lossy(&source[span.start..span.end]).to_string()
                    };
                    match ty {
                        Type::Simple(token) => get_text(token.span),
                        Type::Name(name) => get_text(name.span),
                        Type::Union(types) => types
                            .iter()
                            .map(|t| get_type_text(t, source))
                            .collect::<Vec<_>>()
                            .join("|"),
                        Type::Intersection(types) => types
                            .iter()
                            .map(|t| get_type_text(t, source))
                            .collect::<Vec<_>>()
                            .join("&"),
                        Type::Nullable(ty) => format!("?{}", get_type_text(ty, source)),
                    }
                }

                if let Some(node) = path.last() {
                    match node {
                        AstNode::Expr(Expr::PropertyFetch { property, .. }) => {
                            if let Expr::Variable {
                                name: name_span, ..
                            } = **property
                            {
                                if name_span.start <= cursor_offset
                                    && cursor_offset <= name_span.end
                                {
                                    let name = String::from_utf8_lossy(
                                        &source[name_span.start..name_span.end],
                                    )
                                    .to_string();
                                    let lookup_name = format!("${}", name);

                                    if let Some(entries) = self.index.get(&lookup_name) {
                                        for entry in entries.value() {
                                            if entry.kind == SymbolType::Definition
                                                && entry.type_info.is_some()
                                            {
                                                target_type_name = entry.type_info.clone().unwrap();
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        AstNode::Expr(Expr::MethodCall { method, .. }) => {
                            if let Expr::Variable {
                                name: name_span, ..
                            } = **method
                            {
                                if name_span.start <= cursor_offset
                                    && cursor_offset <= name_span.end
                                {
                                    let name = String::from_utf8_lossy(
                                        &source[name_span.start..name_span.end],
                                    )
                                    .to_string();
                                    if let Some(entries) = self.index.get(&name) {
                                        for entry in entries.value() {
                                            if entry.kind == SymbolType::Definition
                                                && entry.type_info.is_some()
                                            {
                                                target_type_name = entry.type_info.clone().unwrap();
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        AstNode::Stmt(Stmt::Function { params, .. }) => {
                            for param in *params {
                                if param.name.span.start <= cursor_offset
                                    && cursor_offset <= param.name.span.end
                                {
                                    if let Some(ty) = &param.ty {
                                        target_type_name = get_type_text(ty, source);
                                    }
                                }
                            }
                        }
                        AstNode::ClassMember(ClassMember::Method { params, .. }) => {
                            for param in *params {
                                if param.name.span.start <= cursor_offset
                                    && cursor_offset <= param.name.span.end
                                {
                                    if let Some(ty) = &param.ty {
                                        target_type_name = get_type_text(ty, source);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if target_type_name.is_empty() {
            return Ok(None);
        }

        let clean_name = target_type_name.trim_start_matches('?').to_string();
        let type_names: Vec<&str> = clean_name.split('|').collect();
        let mut locations = Vec::new();

        for name in type_names {
            let name = name.trim();
            if let Some(entries) = self.index.get(name) {
                for entry in entries.value() {
                    if entry.kind == SymbolType::Definition {
                        locations.push(Location {
                            uri: entry.uri.clone(),
                            range: entry.range,
                        });
                    }
                }
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(GotoTypeDefinitionResponse::Array(locations)))
        }
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

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

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

        let mut changes = std::collections::HashMap::new();

        if let Some(entries) = self.index.get(&target_name) {
            for entry in entries.iter() {
                changes
                    .entry(entry.uri.clone())
                    .or_insert_with(Vec::new)
                    .push(TextEdit {
                        range: entry.range,
                        new_text: new_name.clone(),
                    });
            }
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        if let Some(content) = self.documents.get(&uri) {
            let source = content.as_bytes();
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);
            let program = parser.parse_program();
            let line_index = LineIndex::new(source);

            let mut visitor = CodeLensVisitor::new(&line_index, source, &self.index);
            visitor.visit_program(&program);

            Ok(Some(visitor.lenses))
        } else {
            Ok(None)
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut actions = Vec::new();

        for diagnostic in params.context.diagnostics {
            if diagnostic.message == "Missing semicolon" {
                let title = "Add missing semicolon".to_string();
                let mut changes = std::collections::HashMap::new();
                changes.insert(
                    params.text_document.uri.clone(),
                    vec![TextEdit {
                        range: Range {
                            start: diagnostic.range.start,
                            end: diagnostic.range.start,
                        },
                        new_text: ";".to_string(),
                    }],
                );

                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title,
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diagnostic]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    command: None,
                    is_preferred: Some(true),
                    disabled: None,
                    data: None,
                }));
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        if let Some(content) = self.documents.get(&uri) {
            let source = content.as_bytes();
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);
            let program = parser.parse_program();
            let line_index = LineIndex::new(source);

            let mut visitor = InlayHintVisitor::new(&line_index, source, &self.index);
            visitor.visit_program(&program);

            Ok(Some(visitor.hints))
        } else {
            Ok(None)
        }
    }

    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = params.text_document.uri;
        if let Some(content) = self.documents.get(&uri) {
            let source = content.as_bytes();
            let bump = Bump::new();
            let lexer = Lexer::new(source);
            let mut parser = Parser::new(lexer, &bump);
            let program = parser.parse_program();
            let line_index = LineIndex::new(source);

            let mut visitor = DocumentLinkVisitor::new(&line_index, source, uri.clone());
            visitor.visit_program(&program);

            Ok(Some(visitor.links))
        } else {
            Ok(None)
        }
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
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

                for node in locator.path.iter().rev() {
                    if let AstNode::Expr(Expr::Call { func, args, .. }) = node {
                        let func_name = if let Expr::Variable { name, .. } = **func {
                            String::from_utf8_lossy(&source[name.start..name.end]).to_string()
                        } else {
                            continue;
                        };

                        let mut active_parameter = 0;
                        for (i, arg) in args.iter().enumerate() {
                            if offset > arg.span.end {
                                active_parameter = i + 1;
                            }
                        }

                        return Ok(Some(SignatureHelp {
                            signatures: vec![SignatureInformation {
                                label: format!("{}(...)", func_name),
                                documentation: None,
                                parameters: None,
                                active_parameter: None,
                            }],
                            active_signature: Some(0),
                            active_parameter: Some(active_parameter as u32),
                        }));
                    }
                }
            }
        }
        Ok(None)
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

        let mut target_name = String::new();
        let mut range = None;

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
                        AstNode::Expr(Expr::Variable { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.start..name.end]).to_string();
                        }
                        AstNode::Expr(Expr::New { class, .. }) => {
                            if let Expr::Variable { name, .. } = *class {
                                target_name =
                                    String::from_utf8_lossy(&source[name.start..name.end])
                                        .to_string();
                            }
                        }
                        AstNode::Expr(Expr::Call { func, .. }) => {
                            if let Expr::Variable { name, .. } = *func {
                                target_name =
                                    String::from_utf8_lossy(&source[name.start..name.end])
                                        .to_string();
                            }
                        }
                        AstNode::Expr(Expr::StaticCall { class, .. }) => {
                            if let Expr::Variable { name, .. } = *class {
                                target_name =
                                    String::from_utf8_lossy(&source[name.start..name.end])
                                        .to_string();
                            }
                        }
                        AstNode::Expr(Expr::ClassConstFetch { class, .. }) => {
                            if let Expr::Variable { name, .. } = *class {
                                target_name =
                                    String::from_utf8_lossy(&source[name.start..name.end])
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
                        AstNode::ClassMember(ClassMember::Method { name, .. }) => {
                            target_name =
                                String::from_utf8_lossy(&source[name.span.start..name.span.end])
                                    .to_string();
                        }
                        AstNode::ClassMember(ClassMember::Property { entries, .. }) => {
                            if let Some(entry) = entries.first() {
                                target_name = String::from_utf8_lossy(
                                    &source[entry.name.span.start..entry.name.span.end],
                                )
                                .to_string();
                            }
                        }
                        _ => {}
                    }

                    let span = node.span();
                    let start = line_index.line_col(span.start);
                    let end = line_index.line_col(span.end);
                    range = Some(Range {
                        start: Position {
                            line: start.0 as u32,
                            character: start.1 as u32,
                        },
                        end: Position {
                            line: end.0 as u32,
                            character: end.1 as u32,
                        },
                    });
                }
            }
        }

        if target_name.is_empty() {
            return Ok(None);
        }

        if let Some(entries) = self.index.get(&target_name) {
            for entry in entries.iter() {
                if entry.kind == SymbolType::Definition {
                    let mut contents = format!("**{}**", target_name);
                    if let Some(kind) = entry.symbol_kind {
                        contents.push_str(&format!(" ({:?})", kind));
                    }
                    if let Some(type_info) = &entry.type_info {
                        contents.push_str(&format!("\n\nType: `{}`", type_info));
                    }
                    if let Some(params) = &entry.parameters {
                        contents.push_str(&format!("\n\nParams: ({})", params.join(", ")));
                    }

                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(contents)),
                        range,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        if let Some(text) = self.documents.get(&uri) {
            let source = text.as_bytes();
            let line_index = LineIndex::new(source);
            let formatter = Formatter::new(source, &line_index);
            Ok(Some(formatter.format()))
        } else {
            Ok(None)
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        documents: DashMap::new(),
        index: DashMap::new(),
        file_map: DashMap::new(),
        root_path: Arc::new(RwLock::new(None)),
    })
    .custom_method(
        "typeHierarchy/supertypes",
        Backend::type_hierarchy_supertypes,
    )
    .custom_method("typeHierarchy/subtypes", Backend::type_hierarchy_subtypes)
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use php_parser::lexer::Lexer;
    use php_parser::parser::Parser;

    fn with_parsed<F>(code: &str, f: F)
    where
        F: FnOnce(&php_parser::ast::Program, &LineIndex, &[u8]),
    {
        let source = code.as_bytes();
        let bump = Bump::new();
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer, &bump);
        let program = parser.parse_program();
        let line_index = LineIndex::new(source);

        f(&program, &line_index, source);
    }

    #[test]
    fn test_document_symbols() {
        let code = r#"<?php
        class User {
            public function getName() {}
        }
        "#;

        with_parsed(code, |program, line_index, source| {
            let mut visitor = DocumentSymbolVisitor::new(line_index, source);
            visitor.visit_program(program);

            assert_eq!(visitor.symbols.len(), 1);
            let class_symbol = &visitor.symbols[0];
            assert_eq!(class_symbol.name, "User");
            assert_eq!(class_symbol.kind, SymbolKind::CLASS);

            assert!(class_symbol.children.is_some());
            let children = class_symbol.children.as_ref().unwrap();
            assert_eq!(children.len(), 1);
            let method_symbol = &children[0];
            assert_eq!(method_symbol.name, "getName");
            assert_eq!(method_symbol.kind, SymbolKind::METHOD);
        });
    }

    #[test]
    fn test_folding_ranges() {
        let code = r#"<?php
        class User {
            public function getName() {
                // body
            }
        }
        "#;

        with_parsed(code, |program, line_index, _| {
            let mut visitor = FoldingRangeVisitor::new(line_index);
            visitor.visit_program(program);

            // Expecting ranges for Class and Method
            assert!(visitor.ranges.len() >= 2);

            // Verify we have a range covering the class
            let has_class_range = visitor.ranges.iter().any(|r| {
                // Class starts at line 1 (0-indexed)
                r.start_line == 1
            });
            assert!(has_class_range, "Should have folding range for class");

            // Verify we have a range covering the method
            let has_method_range = visitor.ranges.iter().any(|r| {
                // Method starts at line 2
                r.start_line == 2
            });
            assert!(has_method_range, "Should have folding range for method");
        });
    }

    #[test]
    fn test_indexing() {
        let code = r#"<?php
        function globalFunc() {}
        class GlobalClass {}
        "#;

        with_parsed(code, |program, line_index, source| {
            let mut visitor = IndexingVisitor::new(line_index, source);
            visitor.visit_program(program);

            let names: Vec<&str> = visitor
                .entries
                .iter()
                .map(|(n, _, _, _, _, _, _, _)| n.as_str())
                .collect();
            assert!(names.contains(&"globalFunc"));
            assert!(names.contains(&"GlobalClass"));

            for (name, _, kind, sym_kind, _, _, _, _) in &visitor.entries {
                if name == "globalFunc" {
                    assert_eq!(*kind, SymbolType::Definition);
                    assert_eq!(*sym_kind, Some(SymbolKind::FUNCTION));
                }
                if name == "GlobalClass" {
                    assert_eq!(*kind, SymbolType::Definition);
                    assert_eq!(*sym_kind, Some(SymbolKind::CLASS));
                }
            }
        });
    }

    #[test]
    fn test_selection_range() {
        let code = "<?php $a = 1;";

        with_parsed(code, |program, line_index, _| {
            // "<?php " is 6 chars.
            // "$a" is at 6..8
            // Offset 7 is inside "$a"
            let offset = 7;

            let mut locator = php_parser::ast::locator::Locator::new(offset);
            locator.visit_program(program);

            assert!(!locator.path.is_empty());

            // Simulate the selection range construction logic
            let mut current: Option<Box<SelectionRange>> = None;
            for node in locator.path.iter() {
                let span = node.span();
                let start = line_index.line_col(span.start);
                let end = line_index.line_col(span.end);
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

                current = Some(Box::new(SelectionRange {
                    range,
                    parent: current,
                }));
            }

            let leaf = current.expect("Should have selection range");
            // Leaf should be the variable $a (6..8)
            assert_eq!(leaf.range.start.character, 6);
            assert_eq!(leaf.range.end.character, 8);

            // Parent should be Assignment (6..12)
            let parent = leaf.parent.as_ref().expect("Should have parent");
            assert_eq!(parent.range.start.character, 6);
            assert_eq!(parent.range.end.character, 12);

            // Grandparent should be Statement (6..13)
            let grandparent = parent.parent.as_ref().expect("Should have grandparent");
            assert_eq!(grandparent.range.start.character, 6);
            assert_eq!(grandparent.range.end.character, 13);
        });
    }

    #[test]
    fn test_formatting() {
        let code = "<?php\nif(true){\necho 1;\n}";
        let source = code.as_bytes();
        let line_index = LineIndex::new(source);
        let formatter = Formatter::new(source, &line_index);
        let edits = formatter.format();

        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "\n    ");
        assert_eq!(edits[1].new_text, "\n");
    }
}
