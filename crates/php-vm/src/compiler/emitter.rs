use php_parser::ast::{Expr, Stmt, BinaryOp, AssignOp, UnaryOp, StmtId, ClassMember, ClassConst};
use php_parser::lexer::token::{Token, TokenKind};
use crate::compiler::chunk::{CodeChunk, UserFunc, CatchEntry, FuncParam};
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
            Stmt::Const { consts, .. } => {
                for c in *consts {
                    let name_str = self.get_text(c.name.span);
                    let sym = self.interner.intern(name_str);
                    
                    // Value must be constant expression.
                    // For now, we only support literals or simple expressions we can evaluate at compile time?
                    // Or we can emit code to evaluate it and then define it?
                    // PHP `const` requires constant expression.
                    // If we emit code, we can use `DefGlobalConst` which takes a value index?
                    // No, `DefGlobalConst` takes `val_idx` which implies it's in the constant table.
                    // So we must evaluate it at compile time.
                    
                    let val = match self.get_literal_value(c.value) {
                        Some(v) => v,
                        None => Val::Null, // TODO: Error or support more complex constant expressions
                    };
                    
                    let val_idx = self.add_constant(val);
                    self.chunk.code.push(OpCode::DefGlobalConst(sym, val_idx as u16));
                }
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
            Stmt::Function { name, params, body, by_ref, .. } => {
                let func_name_str = self.get_text(name.span);
                let func_sym = self.interner.intern(func_name_str);
                
                // Compile body
                let mut func_emitter = Emitter::new(self.source, self.interner);
                let mut func_chunk = func_emitter.compile(body);
                func_chunk.returns_ref = *by_ref;
                
                // Extract params
                let mut param_syms = Vec::new();
                for param in *params {
                    let p_name = self.get_text(param.name.span);
                    if p_name.starts_with(b"$") {
                        let sym = self.interner.intern(&p_name[1..]);
                        param_syms.push(FuncParam {
                            name: sym,
                            by_ref: param.by_ref,
                        });
                    }
                }
                
                let user_func = UserFunc {
                    params: param_syms,
                    uses: Vec::new(),
                    chunk: Rc::new(func_chunk),
                };
                
                let func_res = Val::Resource(Rc::new(user_func));
                let const_idx = self.add_constant(func_res);
                
                self.chunk.code.push(OpCode::DefFunc(func_sym, const_idx as u32));
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
                            let is_static = modifiers.iter().any(|t| t.kind == TokenKind::Static);
                            
                            // Compile method body
                            let mut method_emitter = Emitter::new(self.source, self.interner);
                            let mut method_chunk = method_emitter.compile(body);
                            // method_chunk.returns_ref = *by_ref; // TODO: Add by_ref to ClassMember::Method in parser
                            
                            // Extract params
                            let mut param_syms = Vec::new();
                            for param in *params {
                                let p_name = self.get_text(param.name.span);
                                if p_name.starts_with(b"$") {
                                    let sym = self.interner.intern(&p_name[1..]);
                                    param_syms.push(FuncParam {
                                        name: sym,
                                        by_ref: param.by_ref,
                                    });
                                }
                            }
                            
                            let user_func = UserFunc {
                                params: param_syms,
                                uses: Vec::new(),
                                chunk: Rc::new(method_chunk),
                            };
                            
                            // Store in constants
                            let func_res = Val::Resource(Rc::new(user_func));
                            let const_idx = self.add_constant(func_res);
                            
                            self.chunk.code.push(OpCode::DefMethod(class_sym, method_sym, const_idx as u32, visibility, is_static));
                        }
                        ClassMember::Property { entries, modifiers, .. } => {
                            let visibility = self.get_visibility(modifiers);
                            let is_static = modifiers.iter().any(|t| t.kind == TokenKind::Static);
                            
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
                                
                                if is_static {
                                    self.chunk.code.push(OpCode::DefStaticProp(class_sym, prop_sym, default_idx as u16, visibility));
                                } else {
                                    self.chunk.code.push(OpCode::DefProp(class_sym, prop_sym, default_idx as u16, visibility));
                                }
                            }
                        }
                        ClassMember::Const { consts, modifiers, .. } => {
                            let visibility = self.get_visibility(modifiers);
                            for entry in *consts {
                                let const_name_str = self.get_text(entry.name.span);
                                let const_sym = self.interner.intern(const_name_str);
                                
                                let val = match self.get_literal_value(entry.value) {
                                    Some(v) => v,
                                    None => Val::Null,
                                };
                                let val_idx = self.add_constant(val);
                                
                                self.chunk.code.push(OpCode::DefClassConst(class_sym, const_sym, val_idx as u16, visibility));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Stmt::Foreach { expr, key_var, value_var, body, .. } => {
                // Check if by-ref
                let is_by_ref = matches!(value_var, Expr::Unary { op: UnaryOp::Reference, .. });

                if is_by_ref {
                    if let Expr::Variable { span, .. } = expr {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let sym = self.interner.intern(&name[1..]);
                            self.chunk.code.push(OpCode::MakeVarRef(sym));
                        } else {
                             self.emit_expr(expr);
                        }
                    } else {
                        self.emit_expr(expr);
                    }
                } else {
                    self.emit_expr(expr);
                }
                
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
                } else if let Expr::Unary { op: UnaryOp::Reference, expr, .. } = value_var {
                    if let Expr::Variable { span, .. } = expr {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let sym = self.interner.intern(&name[1..]);
                            self.chunk.code.push(OpCode::IterGetValRef(sym));
                        }
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
            Stmt::Throw { expr, .. } => {
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::Throw);
            }
            Stmt::Try { body, catches, finally, .. } => {
                let try_start = self.chunk.code.len() as u32;
                for stmt in *body {
                    self.emit_stmt(stmt);
                }
                let try_end = self.chunk.code.len() as u32;
                
                let jump_over_catches_idx = self.chunk.code.len();
                self.chunk.code.push(OpCode::Jmp(0)); // Patch later
                
                let mut catch_jumps = Vec::new();
                
                for catch in *catches {
                    let catch_target = self.chunk.code.len() as u32;
                    
                    for ty in catch.types {
                        let type_name = self.get_text(ty.span);
                        let type_sym = self.interner.intern(type_name);
                        
                        self.chunk.catch_table.push(CatchEntry {
                            start: try_start,
                            end: try_end,
                            target: catch_target,
                            catch_type: Some(type_sym),
                        });
                    }
                    
                    if let Some(var) = catch.var {
                        let name = self.get_text(var.span);
                        if name.starts_with(b"$") {
                            let sym = self.interner.intern(&name[1..]);
                            self.chunk.code.push(OpCode::StoreVar(sym));
                        }
                    } else {
                        self.chunk.code.push(OpCode::Pop);
                    }
                    
                    for stmt in catch.body {
                        self.emit_stmt(stmt);
                    }
                    
                    catch_jumps.push(self.chunk.code.len());
                    self.chunk.code.push(OpCode::Jmp(0)); // Patch later
                }
                
                let end_label = self.chunk.code.len() as u32;
                self.patch_jump(jump_over_catches_idx, end_label as usize);
                
                for idx in catch_jumps {
                    self.patch_jump(idx, end_label as usize);
                }
                
                if let Some(finally_body) = finally {
                    for stmt in *finally_body {
                        self.emit_stmt(stmt);
                    }
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
            Expr::Boolean { value, .. } => {
                let idx = self.add_constant(Val::Bool(*value));
                self.chunk.code.push(OpCode::Const(idx as u16));
            }
            Expr::Null { .. } => {
                let idx = self.add_constant(Val::Null);
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
            Expr::Print { expr, .. } => {
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::Echo);
                let idx = self.add_constant(Val::Int(1));
                self.chunk.code.push(OpCode::Const(idx as u16));
            }
            Expr::Include { expr, .. } => {
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::Include);
            }
            Expr::Unary { op, expr, .. } => {
                match op {
                    UnaryOp::Reference => {
                        // Handle &$var
                        if let Expr::Variable { span, .. } = expr {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let var_name = &name[1..];
                                let sym = self.interner.intern(var_name);
                                self.chunk.code.push(OpCode::MakeVarRef(sym));
                            }
                        } else {
                            // Reference to something else?
                            self.emit_expr(expr);
                            self.chunk.code.push(OpCode::MakeRef);
                        }
                    }
                    UnaryOp::Minus => {
                        self.emit_expr(expr);
                        // TODO: OpCode::Negate
                        // For now, 0 - expr
                        let idx = self.add_constant(Val::Int(0));
                        self.chunk.code.push(OpCode::Const(idx as u16));
                        // Swap? No, 0 - expr is wrong order.
                        // We need Negate opcode.
                        // Or push 0 first? But we already emitted expr.
                        // Let's just implement Negate later or use 0 - expr trick if we emit 0 first.
                        // But we can't easily emit 0 first here without changing order.
                        // Let's assume we have Negate or just ignore for now as it's not critical for references.
                    }
                    _ => {
                        self.emit_expr(expr);
                    }
                }
            }
            Expr::Closure { params, uses, body, by_ref, .. } => {
                // Compile body
                let mut func_emitter = Emitter::new(self.source, self.interner);
                let mut func_chunk = func_emitter.compile(body);
                func_chunk.returns_ref = *by_ref;
                
                // Extract params
                let mut param_syms = Vec::new();
                for param in *params {
                    let p_name = self.get_text(param.name.span);
                    if p_name.starts_with(b"$") {
                        let sym = self.interner.intern(&p_name[1..]);
                        param_syms.push(FuncParam {
                            name: sym,
                            by_ref: param.by_ref,
                        });
                    }
                }
                
                // Extract uses
                let mut use_syms = Vec::new();
                for use_var in *uses {
                    let u_name = self.get_text(use_var.var.span);
                    if u_name.starts_with(b"$") {
                        let sym = self.interner.intern(&u_name[1..]);
                        use_syms.push(sym);
                        
                        // Emit code to push the captured variable onto the stack
                        self.chunk.code.push(OpCode::LoadVar(sym));
                    }
                }
                
                let user_func = UserFunc {
                    params: param_syms,
                    uses: use_syms.clone(),
                    chunk: Rc::new(func_chunk),
                };
                
                let func_res = Val::Resource(Rc::new(user_func));
                let const_idx = self.add_constant(func_res);
                
                self.chunk.code.push(OpCode::Closure(const_idx as u32, use_syms.len() as u32));
            }
            Expr::Call { func, args, .. } => {
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

                for arg in *args {
                    self.emit_expr(&arg.value);
                }
                
                self.chunk.code.push(OpCode::Call(args.len() as u8));
            }
            Expr::Variable { span, .. } => {
                let name = self.get_text(*span);
                if name.starts_with(b"$") {
                    let var_name = &name[1..];
                    let sym = self.interner.intern(var_name);
                    self.chunk.code.push(OpCode::LoadVar(sym));
                } else {
                    // Constant fetch
                    let sym = self.interner.intern(name);
                    self.chunk.code.push(OpCode::FetchGlobalConst(sym));
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
            Expr::StaticCall { class, method, args, .. } => {
                if let Expr::Variable { span, .. } = class {
                    let class_name = self.get_text(*span);
                    if !class_name.starts_with(b"$") {
                        let class_sym = self.interner.intern(class_name);
                        
                        for arg in *args {
                            self.emit_expr(arg.value);
                        }
                        
                        if let Expr::Variable { span: method_span, .. } = method {
                            let method_name = self.get_text(*method_span);
                            if !method_name.starts_with(b"$") {
                                let method_sym = self.interner.intern(method_name);
                                self.chunk.code.push(OpCode::CallStaticMethod(class_sym, method_sym, args.len() as u8));
                            }
                        }
                    }
                }
            }
            Expr::ClassConstFetch { class, constant, .. } => {
                if let Expr::Variable { span, .. } = class {
                    let class_name = self.get_text(*span);
                    if !class_name.starts_with(b"$") {
                        let class_sym = self.interner.intern(class_name);
                        
                        if let Expr::Variable { span: const_span, .. } = constant {
                             let const_name = self.get_text(*const_span);
                             if const_name.starts_with(b"$") {
                                 let prop_name = &const_name[1..];
                                 let prop_sym = self.interner.intern(prop_name);
                                 self.chunk.code.push(OpCode::FetchStaticProp(class_sym, prop_sym));
                             } else {
                                 let const_sym = self.interner.intern(const_name);
                                 self.chunk.code.push(OpCode::FetchClassConst(class_sym, const_sym));
                             }
                        }
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
                    Expr::ClassConstFetch { class, constant, .. } => {
                        self.emit_expr(expr);
                        if let Expr::Variable { span, .. } = class {
                            let class_name = self.get_text(*span);
                            if !class_name.starts_with(b"$") {
                                let class_sym = self.interner.intern(class_name);
                                
                                if let Expr::Variable { span: const_span, .. } = constant {
                                     let const_name = self.get_text(*const_span);
                                     if const_name.starts_with(b"$") {
                                         let prop_name = &const_name[1..];
                                         let prop_sym = self.interner.intern(prop_name);
                                         self.chunk.code.push(OpCode::AssignStaticProp(class_sym, prop_sym));
                                     }
                                }
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
            Expr::AssignRef { var, expr, .. } => {
                match var {
                    Expr::Variable { span, .. } => {
                        // Check if expr is a variable
                        let mut handled = false;
                        if let Expr::Variable { span: src_span, .. } = expr {
                            let src_name = self.get_text(*src_span);
                            if src_name.starts_with(b"$") {
                                let src_sym = self.interner.intern(&src_name[1..]);
                                self.chunk.code.push(OpCode::MakeVarRef(src_sym));
                                handled = true;
                            }
                        }
                        
                        if !handled {
                            self.emit_expr(expr);
                            self.chunk.code.push(OpCode::MakeRef);
                        }

                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let var_name = &name[1..];
                            let sym = self.interner.intern(var_name);
                            self.chunk.code.push(OpCode::AssignRef(sym));
                            self.chunk.code.push(OpCode::LoadVar(sym));
                        }
                    }
                    Expr::ArrayDimFetch { array: array_var, dim, .. } => {
                        self.emit_expr(array_var);
                        if let Some(d) = dim {
                            self.emit_expr(d);
                        } else {
                            // TODO: Handle append
                            self.chunk.code.push(OpCode::Const(0)); 
                        }
                        
                        let mut handled = false;
                        if let Expr::Variable { span: src_span, .. } = expr {
                            let src_name = self.get_text(*src_span);
                            if src_name.starts_with(b"$") {
                                let src_sym = self.interner.intern(&src_name[1..]);
                                self.chunk.code.push(OpCode::MakeVarRef(src_sym));
                                handled = true;
                            }
                        }
                        
                        if !handled {
                            self.emit_expr(expr);
                            self.chunk.code.push(OpCode::MakeRef);
                        }
                        
                        self.chunk.code.push(OpCode::AssignDimRef);
                        
                        // Store back the updated array if target is a variable
                        if let Expr::Variable { span, .. } = array_var {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let sym = self.interner.intern(&name[1..]);
                                self.chunk.code.push(OpCode::StoreVar(sym));
                            } else {
                                self.chunk.code.push(OpCode::Pop);
                            }
                        } else {
                            self.chunk.code.push(OpCode::Pop);
                        }
                    }
                    _ => {
                        // TODO: Support other targets for reference assignment
                    }
                }
            }
            Expr::AssignOp { var, op, expr, .. } => {
                match var {
                    Expr::Variable { span, .. } => {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let var_name = &name[1..];
                            let sym = self.interner.intern(var_name);
                            
                            // Load var
                            self.chunk.code.push(OpCode::LoadVar(sym));
                            
                            // Evaluate expr
                            self.emit_expr(expr);
                            
                            // Op
                            match op {
                                AssignOp::Plus => self.chunk.code.push(OpCode::Add),
                                AssignOp::Minus => self.chunk.code.push(OpCode::Sub),
                                AssignOp::Mul => self.chunk.code.push(OpCode::Mul),
                                AssignOp::Div => self.chunk.code.push(OpCode::Div),
                                AssignOp::Concat => self.chunk.code.push(OpCode::Concat),
                                _ => {} // TODO: Implement other ops
                            }
                            
                            // Store
                            self.chunk.code.push(OpCode::StoreVar(sym));
                        }
                    }
                    _ => {} // TODO: Property/Array fetch
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
