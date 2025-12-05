use php_parser::ast::{Expr, Stmt, BinaryOp, StmtId, ClassMember};
use php_parser::lexer::token::{Token, TokenKind};
use crate::compiler::chunk::{CodeChunk, UserFunc};
use crate::vm::opcode::OpCode;
use crate::core::value::{Val, Visibility};
use crate::core::interner::Interner;
use std::rc::Rc;

struct LoopInfo {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

pub struct Emitter<'src> {
    chunk: CodeChunk,
    source: &'src [u8],
    interner: &'src mut Interner,
    loop_stack: Vec<LoopInfo>,
}

impl<'src> Emitter<'src> {
    pub fn new(source: &'src [u8], interner: &'src mut Interner) -> Self {
        Self {
            chunk: CodeChunk::default(),
            source,
            interner,
            loop_stack: Vec::new(),
        }
    }

    fn get_visibility(&self, modifiers: &[Token]) -> Visibility {
        for token in modifiers {
            match token.kind {
                TokenKind::Public => return Visibility::Public,
                TokenKind::Protected => return Visibility::Protected,
                TokenKind::Private => return Visibility::Private,
                _ => {}
            }
        }
        Visibility::Public // Default
    }

    pub fn compile(mut self, stmts: &[StmtId]) -> CodeChunk {
        for stmt in stmts {
            self.emit_stmt(stmt);
        }
        // Implicit return null
        let null_idx = self.add_constant(Val::Null);
        self.chunk.code.push(OpCode::Const(null_idx as u16));
        self.chunk.code.push(OpCode::Return);
        
        self.chunk
    }

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Echo { exprs, .. } => {
                for expr in *exprs {
                    self.emit_expr(expr);
                    self.chunk.code.push(OpCode::Echo);
                }
            }
            Stmt::Expression { expr, .. } => {
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::Pop);
            }
            Stmt::Return { expr, .. } => {
                if let Some(e) = expr {
                    self.emit_expr(e);
                } else {
                    let idx = self.add_constant(Val::Null);
                    self.chunk.code.push(OpCode::Const(idx as u16));
                }
                self.chunk.code.push(OpCode::Return);
            }
            Stmt::Break { .. } => {
                if let Some(loop_info) = self.loop_stack.last_mut() {
                    let idx = self.chunk.code.len();
                    self.chunk.code.push(OpCode::Jmp(0)); // Patch later
                    loop_info.break_jumps.push(idx);
                }
            }
            Stmt::Continue { .. } => {
                if let Some(loop_info) = self.loop_stack.last_mut() {
                    let idx = self.chunk.code.len();
                    self.chunk.code.push(OpCode::Jmp(0)); // Patch later
                    loop_info.continue_jumps.push(idx);
                }
            }
            Stmt::If { condition, then_block, else_block, .. } => {
                self.emit_expr(condition);
                
                let jump_false_idx = self.chunk.code.len();
                self.chunk.code.push(OpCode::JmpIfFalse(0));
                
                for stmt in *then_block {
                    self.emit_stmt(stmt);
                }
                
                let jump_end_idx = self.chunk.code.len();
                self.chunk.code.push(OpCode::Jmp(0));
                
                let else_label = self.chunk.code.len();
                self.patch_jump(jump_false_idx, else_label);
                
                if let Some(else_stmts) = else_block {
                    for stmt in *else_stmts {
                        self.emit_stmt(stmt);
                    }
                }
                
                let end_label = self.chunk.code.len();
                self.patch_jump(jump_end_idx, end_label);
            }
            Stmt::Class { name, members, extends, .. } => {
                let class_name_str = self.get_text(name.span);
                let class_sym = self.interner.intern(class_name_str);
                
                let parent_sym = if let Some(parent_name) = extends {
                    let parent_str = self.get_text(parent_name.span);
                    Some(self.interner.intern(parent_str))
                } else {
                    None
                };

                self.chunk.code.push(OpCode::DefClass(class_sym, parent_sym));
                
                for member in *members {
                    match member {
                        ClassMember::Method { name, body, params, modifiers, .. } => {
                            let method_name_str = self.get_text(name.span);
                            let method_sym = self.interner.intern(method_name_str);
                            let visibility = self.get_visibility(modifiers);
                            
                            // Compile method body
                            let mut method_emitter = Emitter::new(self.source, self.interner);
                            let method_chunk = method_emitter.compile(body);
                            
                            // Extract params
                            let mut param_syms = Vec::new();
                            for param in *params {
                                let p_name = self.get_text(param.name.span);
                                if p_name.starts_with(b"$") {
                                    param_syms.push(self.interner.intern(&p_name[1..]));
                                }
                            }
                            
                            let user_func = UserFunc {
                                params: param_syms,
                                chunk: Rc::new(method_chunk),
                            };
                            
                            // Store in constants
                            let func_res = Val::Resource(Rc::new(user_func));
                            let const_idx = self.add_constant(func_res);
                            
                            self.chunk.code.push(OpCode::DefMethod(class_sym, method_sym, const_idx as u32, visibility));
                        }
                        ClassMember::Property { entries, modifiers, .. } => {
                            let visibility = self.get_visibility(modifiers);
                            for entry in *entries {
                                let prop_name_str = self.get_text(entry.name.span);
                                let prop_name = if prop_name_str.starts_with(b"$") {
                                    &prop_name_str[1..]
                                } else {
                                    prop_name_str
                                };
                                let prop_sym = self.interner.intern(prop_name);
                                
                                let default_idx = if let Some(default_expr) = entry.default {
                                    // TODO: Handle constant expressions properly
                                    // For now, default to Null if not simple literal
                                    let val = match self.get_literal_value(default_expr) {
                                        Some(v) => v,
                                        None => Val::Null,
                                    };
                                    self.add_constant(val)
                                } else {
                                    self.add_constant(Val::Null)
                                };
                                
                                self.chunk.code.push(OpCode::DefProp(class_sym, prop_sym, default_idx as u16, visibility));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Stmt::Foreach { expr, key_var, value_var, body, .. } => {
                self.emit_expr(expr);
                
                // IterInit(End)
                let init_idx = self.chunk.code.len();
                self.chunk.code.push(OpCode::IterInit(0)); // Patch later
                
                let start_label = self.chunk.code.len();
                
                // IterValid(End)
                let valid_idx = self.chunk.code.len();
                self.chunk.code.push(OpCode::IterValid(0)); // Patch later
                
                // IterGetVal
                if let Expr::Variable { span, .. } = value_var {
                     let name = self.get_text(*span);
                     if name.starts_with(b"$") {
                         let sym = self.interner.intern(&name[1..]);
                         self.chunk.code.push(OpCode::IterGetVal(sym));
                     }
                }
                
                // IterGetKey
                if let Some(k) = key_var {
                    if let Expr::Variable { span, .. } = k {
                         let name = self.get_text(*span);
                         if name.starts_with(b"$") {
                             let sym = self.interner.intern(&name[1..]);
                             self.chunk.code.push(OpCode::IterGetKey(sym));
                         }
                    }
                }
                
                self.loop_stack.push(LoopInfo { break_jumps: Vec::new(), continue_jumps: Vec::new() });
                
                // Body
                for stmt in *body {
                    self.emit_stmt(stmt);
                }
                
                let continue_label = self.chunk.code.len();
                // IterNext
                self.chunk.code.push(OpCode::IterNext);
                
                // Jump back to start
                self.chunk.code.push(OpCode::Jmp(start_label as u32));
                
                let end_label = self.chunk.code.len();
                
                // Patch jumps
                self.patch_jump(init_idx, end_label);
                self.patch_jump(valid_idx, end_label);
                
                let loop_info = self.loop_stack.pop().unwrap();
                for idx in loop_info.break_jumps {
                    self.patch_jump(idx, end_label);
                }
                for idx in loop_info.continue_jumps {
                    self.patch_jump(idx, continue_label);
                }
            }
            _ => {} 
        }
    }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        let op = self.chunk.code[idx];
        let new_op = match op {
            OpCode::Jmp(_) => OpCode::Jmp(target as u32),
            OpCode::JmpIfFalse(_) => OpCode::JmpIfFalse(target as u32),
            OpCode::IterInit(_) => OpCode::IterInit(target as u32),
            OpCode::IterValid(_) => OpCode::IterValid(target as u32),
            _ => panic!("Cannot patch non-jump opcode: {:?}", op),
        };
        self.chunk.code[idx] = new_op;
    }

    fn get_literal_value(&self, expr: &Expr) -> Option<Val> {
        match expr {
            Expr::Integer { value, .. } => {
                let s = std::str::from_utf8(value).ok()?;
                let i: i64 = s.parse().ok()?;
                Some(Val::Int(i))
            }
            Expr::String { value, .. } => {
                let s = if value.len() >= 2 {
                    &value[1..value.len()-1]
                } else {
                    value
                };
                Some(Val::String(s.to_vec()))
            }
            Expr::Boolean { value, .. } => Some(Val::Bool(*value)),
            Expr::Null { .. } => Some(Val::Null),
            _ => None,
        }
    }

    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Integer { value, .. } => {
                let s = std::str::from_utf8(value).unwrap_or("0");
                let i: i64 = s.parse().unwrap_or(0);
                let idx = self.add_constant(Val::Int(i));
                self.chunk.code.push(OpCode::Const(idx as u16));
            }
            Expr::String { value, .. } => {
                let s = if value.len() >= 2 {
                    &value[1..value.len()-1]
                } else {
                    value
                };
                let idx = self.add_constant(Val::String(s.to_vec()));
                self.chunk.code.push(OpCode::Const(idx as u16));
            }
            Expr::Binary { left, op, right, .. } => {
                self.emit_expr(left);
                self.emit_expr(right);
                match op {
                    BinaryOp::Plus => self.chunk.code.push(OpCode::Add),
                    BinaryOp::Minus => self.chunk.code.push(OpCode::Sub),
                    BinaryOp::Mul => self.chunk.code.push(OpCode::Mul),
                    BinaryOp::Div => self.chunk.code.push(OpCode::Div),
                    BinaryOp::Concat => self.chunk.code.push(OpCode::Concat),
                    BinaryOp::EqEq => self.chunk.code.push(OpCode::IsEqual),
                    BinaryOp::EqEqEq => self.chunk.code.push(OpCode::IsIdentical),
                    BinaryOp::NotEq => self.chunk.code.push(OpCode::IsNotEqual),
                    BinaryOp::NotEqEq => self.chunk.code.push(OpCode::IsNotIdentical),
                    BinaryOp::Gt => self.chunk.code.push(OpCode::IsGreater),
                    BinaryOp::Lt => self.chunk.code.push(OpCode::IsLess),
                    BinaryOp::GtEq => self.chunk.code.push(OpCode::IsGreaterOrEqual),
                    BinaryOp::LtEq => self.chunk.code.push(OpCode::IsLessOrEqual),
                    _ => {} 
                }
            }
            Expr::Call { func, args, .. } => {
                for arg in *args {
                    self.emit_expr(&arg.value);
                }
                
                match func {
                    Expr::Variable { span, .. } => {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            self.emit_expr(func);
                        } else {
                            let idx = self.add_constant(Val::String(name.to_vec()));
                            self.chunk.code.push(OpCode::Const(idx as u16));
                        }
                    }
                    _ => self.emit_expr(func),
                }
                
                self.chunk.code.push(OpCode::Call(args.len() as u8));
            }
            Expr::Variable { span, .. } => {
                let name = self.get_text(*span);
                if name.starts_with(b"$") {
                    let var_name = &name[1..];
                    let sym = self.interner.intern(var_name);
                    self.chunk.code.push(OpCode::LoadVar(sym));
                }
            }
            Expr::Array { items, .. } => {
                self.chunk.code.push(OpCode::InitArray);
                for item in *items {
                    if item.unpack {
                        continue;
                    }
                    if let Some(key) = item.key {
                        self.emit_expr(key);
                        self.emit_expr(item.value);
                        self.chunk.code.push(OpCode::AssignDim);
                    } else {
                        self.emit_expr(item.value);
                        self.chunk.code.push(OpCode::AppendArray);
                    }
                }
            }
            Expr::ArrayDimFetch { array, dim, .. } => {
                self.emit_expr(array);
                if let Some(d) = dim {
                    self.emit_expr(d);
                    self.chunk.code.push(OpCode::FetchDim);
                }
            }
            Expr::New { class, args, .. } => {
                if let Expr::Variable { span, .. } = class {
                    let name = self.get_text(*span);
                    if !name.starts_with(b"$") {
                        let class_sym = self.interner.intern(name);
                        
                        for arg in *args {
                            self.emit_expr(arg.value);
                        }
                        
                        self.chunk.code.push(OpCode::New(class_sym, args.len() as u8));
                    }
                }
            }
            Expr::PropertyFetch { target, property, .. } => {
                self.emit_expr(target);
                if let Expr::Variable { span, .. } = property {
                    let name = self.get_text(*span);
                    if !name.starts_with(b"$") {
                        let sym = self.interner.intern(name);
                        self.chunk.code.push(OpCode::FetchProp(sym));
                    }
                }
            }
            Expr::MethodCall { target, method, args, .. } => {
                self.emit_expr(target);
                for arg in *args {
                    self.emit_expr(arg.value);
                }
                if let Expr::Variable { span, .. } = method {
                    let name = self.get_text(*span);
                    if !name.starts_with(b"$") {
                        let sym = self.interner.intern(name);
                        self.chunk.code.push(OpCode::CallMethod(sym, args.len() as u8));
                    }
                }
            }
            Expr::Assign { var, expr, .. } => {
                match var {
                    Expr::Variable { span, .. } => {
                        self.emit_expr(expr);
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let var_name = &name[1..];
                            let sym = self.interner.intern(var_name);
                            self.chunk.code.push(OpCode::StoreVar(sym));
                            self.chunk.code.push(OpCode::LoadVar(sym));
                        }
                    }
                    Expr::PropertyFetch { target, property, .. } => {
                        self.emit_expr(target);
                        self.emit_expr(expr);
                        if let Expr::Variable { span, .. } = property {
                            let name = self.get_text(*span);
                            if !name.starts_with(b"$") {
                                let sym = self.interner.intern(name);
                                self.chunk.code.push(OpCode::AssignProp(sym));
                            }
                        }
                    }
                    Expr::ArrayDimFetch { .. } => {
                        let (base, keys) = Self::flatten_dim_fetch(var);
                        
                        self.emit_expr(base);
                        for key in &keys {
                            if let Some(k) = key {
                                self.emit_expr(k);
                            } else {
                                let idx = self.add_constant(Val::AppendPlaceholder);
                                self.chunk.code.push(OpCode::Const(idx as u16));
                            }
                        }
                        
                        self.emit_expr(expr);
                        
                        self.chunk.code.push(OpCode::StoreNestedDim(keys.len() as u8));
                        
                        if let Expr::Variable { span, .. } = base {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let var_name = &name[1..];
                                let sym = self.interner.intern(var_name);
                                self.chunk.code.push(OpCode::StoreVar(sym));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn flatten_dim_fetch<'a, 'ast>(mut expr: &'a Expr<'ast>) -> (&'a Expr<'ast>, Vec<Option<&'a Expr<'ast>>>) {
        let mut keys = Vec::new();
        while let Expr::ArrayDimFetch { array, dim, .. } = expr {
            keys.push(*dim);
            expr = array;
        }
        keys.reverse();
        (expr, keys)
    }

    fn add_constant(&mut self, val: Val) -> usize {
        self.chunk.constants.push(val);
        self.chunk.constants.len() - 1
    }
    
    fn get_text(&self, span: php_parser::span::Span) -> &'src [u8] {
        &self.source[span.start..span.end]
    }
}
