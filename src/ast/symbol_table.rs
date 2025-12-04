use std::collections::HashMap;
use crate::span::Span;
use crate::ast::visitor::{Visitor, walk_program, walk_stmt, walk_expr, walk_param};
use crate::ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Function,
    Class,
    Interface,
    Trait,
    Enum,
    EnumCase,
    Parameter,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
}

#[derive(Debug, Default)]
pub struct Scope {
    pub symbols: HashMap<String, Symbol>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

impl Scope {
    pub fn new(parent: Option<usize>) -> Self {
        Self {
            symbols: HashMap::new(),
            parent,
            children: Vec::new(),
        }
    }

    pub fn add(&mut self, name: String, kind: SymbolKind, span: Span) {
        self.symbols.insert(name.clone(), Symbol { name, kind, span });
    }
    
    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }
}

#[derive(Debug)]
pub struct SymbolTable {
    pub scopes: Vec<Scope>,
    pub current_scope_idx: usize,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self {
            scopes: vec![Scope::new(None)], // Root scope
            current_scope_idx: 0,
        }
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter_scope(&mut self) {
        let new_scope_idx = self.scopes.len();
        let new_scope = Scope::new(Some(self.current_scope_idx));
        self.scopes.push(new_scope);
        
        // Register as child of current scope
        self.scopes[self.current_scope_idx].children.push(new_scope_idx);
        
        self.current_scope_idx = new_scope_idx;
    }

    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope_idx].parent {
            self.current_scope_idx = parent;
        } else {
            // Should not happen if balanced
            eprintln!("Warning: Attempted to exit root scope");
        }
    }

    pub fn add_symbol(&mut self, name: String, kind: SymbolKind, span: Span) {
        self.scopes[self.current_scope_idx].add(name, kind, span);
    }
    
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        let mut current = Some(self.current_scope_idx);
        while let Some(idx) = current {
            if let Some(sym) = self.scopes[idx].get(name) {
                return Some(sym);
            }
            current = self.scopes[idx].parent;
        }
        None
    }
}

pub struct SymbolVisitor<'src> {
    pub table: SymbolTable,
    pub source: &'src [u8],
}

impl<'src> SymbolVisitor<'src> {
    pub fn new(source: &'src [u8]) -> Self {
        Self {
            table: SymbolTable::new(),
            source,
        }
    }

    fn get_text(&self, span: Span) -> String {
        String::from_utf8_lossy(span.as_str(self.source)).to_string()
    }
}

impl<'ast, 'src> Visitor<'ast> for SymbolVisitor<'src> {
    fn visit_program(&mut self, program: &'ast Program<'ast>) {
        // Root scope is already created in default()
        walk_program(self, program);
    }

    fn visit_stmt(&mut self, stmt: StmtId<'ast>) {
        match stmt {
            Stmt::Function { name, params, body, span, .. } => {
                let func_name = self.get_text(name.span);
                self.table.add_symbol(func_name, SymbolKind::Function, *span);
                
                self.table.enter_scope();
                for param in *params {
                    self.visit_param(param);
                }
                for s in *body {
                    self.visit_stmt(s);
                }
                self.table.exit_scope();
            },
            Stmt::Class { name, members, span, .. } => {
                let class_name = self.get_text(name.span);
                self.table.add_symbol(class_name, SymbolKind::Class, *span);
                self.table.enter_scope();
                for member in *members {
                    self.visit_class_member(member);
                }
                self.table.exit_scope();
            },
            Stmt::Interface { name, members, span, .. } => {
                let interface_name = self.get_text(name.span);
                self.table.add_symbol(interface_name, SymbolKind::Interface, *span);
                self.table.enter_scope();
                for member in *members {
                    self.visit_class_member(member);
                }
                self.table.exit_scope();
            },
            Stmt::Trait { name, members, span, .. } => {
                let trait_name = self.get_text(name.span);
                self.table.add_symbol(trait_name, SymbolKind::Trait, *span);
                self.table.enter_scope();
                for member in *members {
                    self.visit_class_member(member);
                }
                self.table.exit_scope();
            },
            Stmt::Enum { name, members, span, .. } => {
                let enum_name = self.get_text(name.span);
                self.table.add_symbol(enum_name, SymbolKind::Enum, *span);
                self.table.enter_scope();
                for member in *members {
                    self.visit_class_member(member);
                }
                self.table.exit_scope();
            },
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_param(&mut self, param: &'ast Param<'ast>) {
        let name = self.get_text(param.name.span);
        self.table.add_symbol(name, SymbolKind::Parameter, param.span);
        walk_param(self, param);
    }

    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        match expr {
            Expr::Assign { var, .. } => {
                if let Expr::Variable { name, span } = var {
                     let var_name = self.get_text(*name);
                     if self.table.scopes[self.table.current_scope_idx].get(&var_name).is_none() {
                         self.table.add_symbol(var_name, SymbolKind::Variable, *span);
                     }
                }
                walk_expr(self, expr);
            },
            _ => walk_expr(self, expr),
        }
    }
}

impl<'src> SymbolVisitor<'src> {
    fn visit_class_member<'ast>(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Method { name, params, body, span, .. } => {
                let method_name = self.get_text(name.span);
                self.table.add_symbol(method_name, SymbolKind::Function, *span);
                self.table.enter_scope();
                for param in *params {
                    self.visit_param(param);
                }
                for stmt in *body {
                    self.visit_stmt(stmt);
                }
                self.table.exit_scope();
            },
            ClassMember::Property { entries, .. } => {
                for entry in *entries {
                    let prop_name = self.get_text(entry.name.span);
                    self.table.add_symbol(prop_name, SymbolKind::Variable, entry.span);
                }
            },
            ClassMember::Case { name, span, .. } => {
                let case_name = self.get_text(name.span);
                // Enum cases are like constants or static properties
                self.table.add_symbol(case_name, SymbolKind::EnumCase, *span); 
            },
            _ => {} 
        }
    }
}
