use php_parser::ast::{Expr, Stmt, BinaryOp, AssignOp, UnaryOp, StmtId, ClassMember, CastKind};
use php_parser::lexer::token::{Token, TokenKind};
use crate::compiler::chunk::{CodeChunk, UserFunc, CatchEntry, FuncParam};
use crate::vm::opcode::OpCode;
use crate::core::value::{Val, Visibility};
use crate::core::interner::Interner;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

struct LoopInfo {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

pub struct Emitter<'src> {
    chunk: CodeChunk,
    source: &'src [u8],
    interner: &'src mut Interner,
    loop_stack: Vec<LoopInfo>,
    is_generator: bool,
}

impl<'src> Emitter<'src> {
    pub fn new(source: &'src [u8], interner: &'src mut Interner) -> Self {
        Self {
            chunk: CodeChunk::default(),
            source,
            interner,
            loop_stack: Vec::new(),
            is_generator: false,
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

    pub fn compile(mut self, stmts: &[StmtId]) -> (CodeChunk, bool) {
        for stmt in stmts {
            self.emit_stmt(stmt);
        }
        // Implicit return null
        let null_idx = self.add_constant(Val::Null);
        self.chunk.code.push(OpCode::Const(null_idx as u16));
        self.chunk.code.push(OpCode::Return);
        
        (self.chunk, self.is_generator)
    }

    fn emit_members(&mut self, class_sym: crate::core::value::Symbol, members: &[ClassMember]) {
        for member in members {
            match member {
                ClassMember::Method { name, body, params, modifiers, .. } => {
                    let method_name_str = self.get_text(name.span);
                    let method_sym = self.interner.intern(method_name_str);
                    let visibility = self.get_visibility(modifiers);
                    let is_static = modifiers.iter().any(|t| t.kind == TokenKind::Static);
                    
                    // 1. Collect param info
                    struct ParamInfo<'a> {
                        name_span: php_parser::span::Span,
                        by_ref: bool,
                        default: Option<&'a Expr<'a>>,
                    }
                    
                    let mut param_infos = Vec::new();
                    for param in *params {
                        param_infos.push(ParamInfo {
                            name_span: param.name.span,
                            by_ref: param.by_ref,
                            default: param.default.as_ref().map(|e| *e),
                        });
                    }

                    // 2. Create emitter
                    let mut method_emitter = Emitter::new(self.source, self.interner);
                    
                    // 3. Process params
                    let mut param_syms = Vec::new();
                    for (i, info) in param_infos.iter().enumerate() {
                        let p_name = method_emitter.get_text(info.name_span);
                        if p_name.starts_with(b"$") {
                            let sym = method_emitter.interner.intern(&p_name[1..]);
                            param_syms.push(FuncParam {
                                name: sym,
                                by_ref: info.by_ref,
                            });
                            
                            if let Some(default_expr) = info.default {
                                let val = method_emitter.eval_constant_expr(default_expr);
                                let idx = method_emitter.add_constant(val);
                                method_emitter.chunk.code.push(OpCode::RecvInit(i as u32, idx as u16));
                            } else {
                                method_emitter.chunk.code.push(OpCode::Recv(i as u32));
                            }
                        }
                    }

                    let (method_chunk, is_generator) = method_emitter.compile(body);
                    
                    let user_func = UserFunc {
                        params: param_syms,
                        uses: Vec::new(),
                        chunk: Rc::new(method_chunk),
                        is_static,
                        is_generator,
                        statics: Rc::new(RefCell::new(HashMap::new())),
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
                ClassMember::TraitUse { traits, .. } => {
                    for trait_name in *traits {
                        let trait_str = self.get_text(trait_name.span);
                        let trait_sym = self.interner.intern(trait_str);
                        self.chunk.code.push(OpCode::UseTrait(class_sym, trait_sym));
                    }
                }
                _ => {}
            }
        }
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
            Stmt::Global { vars, .. } => {
                for var in *vars {
                    if let Expr::Variable { span, .. } = var {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let var_name = &name[1..];
                            let sym = self.interner.intern(var_name);
                            self.chunk.code.push(OpCode::BindGlobal(sym));
                        }
                    }
                }
            }
            Stmt::Static { vars, .. } => {
                for var in *vars {
                    // Check if var.var is Assign
                    let (target_var, default_expr) = if let Expr::Assign { var: assign_var, expr: assign_expr, .. } = var.var {
                        (*assign_var, Some(*assign_expr))
                    } else {
                        (var.var, var.default)
                    };

                    let name = if let Expr::Variable { span, .. } = target_var {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            self.interner.intern(&name[1..])
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    };
                    
                    let val = if let Some(expr) = default_expr {
                        self.eval_constant_expr(expr)
                    } else {
                        Val::Null
                    };
                    
                    let idx = self.add_constant(val);
                    self.chunk.code.push(OpCode::BindStatic(name, idx as u16));
                }
            }
            Stmt::Unset { vars, .. } => {
                for var in *vars {
                    match var {
                        Expr::Variable { span, .. } => {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let sym = self.interner.intern(&name[1..]);
                                self.chunk.code.push(OpCode::UnsetVar(sym));
                            }
                        }
                        Expr::ArrayDimFetch { array, dim, .. } => {
                            if let Expr::Variable { span, .. } = array {
                                let name = self.get_text(*span);
                                if name.starts_with(b"$") {
                                    let sym = self.interner.intern(&name[1..]);
                                    self.chunk.code.push(OpCode::LoadVar(sym));
                                    self.chunk.code.push(OpCode::Dup);
                                    
                                    if let Some(d) = dim {
                                        self.emit_expr(d);
                                    } else {
                                        let idx = self.add_constant(Val::Null);
                                        self.chunk.code.push(OpCode::Const(idx as u16));
                                    }
                                    
                                    self.chunk.code.push(OpCode::UnsetDim);
                                    self.chunk.code.push(OpCode::StoreVar(sym));
                                    self.chunk.code.push(OpCode::Pop);
                                }
                            }
                        }
                        Expr::PropertyFetch { target, property, .. } => {
                            self.emit_expr(target);
                            if let Expr::Variable { span, .. } = property {
                                let name = self.get_text(*span);
                                let idx = self.add_constant(Val::String(name.to_vec()));
                                self.chunk.code.push(OpCode::Const(idx as u16));
                                self.chunk.code.push(OpCode::UnsetObj);
                            }
                        }
                        Expr::ClassConstFetch { class, constant, .. } => {
                            let is_static_prop = if let Expr::Variable { span, .. } = constant {
                                let name = self.get_text(*span);
                                name.starts_with(b"$")
                            } else {
                                false
                            };

                            if is_static_prop {
                                if let Expr::Variable { span, .. } = class {
                                    let name = self.get_text(*span);
                                    if !name.starts_with(b"$") {
                                        let idx = self.add_constant(Val::String(name.to_vec()));
                                        self.chunk.code.push(OpCode::Const(idx as u16));
                                    } else {
                                        let sym = self.interner.intern(&name[1..]);
                                        self.chunk.code.push(OpCode::LoadVar(sym));
                                    }
                                    
                                    if let Expr::Variable { span: prop_span, .. } = constant {
                                        let prop_name = self.get_text(*prop_span);
                                        let idx = self.add_constant(Val::String(prop_name[1..].to_vec()));
                                        self.chunk.code.push(OpCode::Const(idx as u16));
                                        self.chunk.code.push(OpCode::UnsetStaticProp);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
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
                
                // 1. Collect param info to avoid borrow issues
                struct ParamInfo<'a> {
                    name_span: php_parser::span::Span,
                    by_ref: bool,
                    default: Option<&'a Expr<'a>>,
                }
                
                let mut param_infos = Vec::new();
                for param in *params {
                    param_infos.push(ParamInfo {
                        name_span: param.name.span,
                        by_ref: param.by_ref,
                        default: param.default.as_ref().map(|e| *e),
                    });
                }

                // 2. Create emitter
                let mut func_emitter = Emitter::new(self.source, self.interner);
                
                // 3. Process params using func_emitter
                let mut param_syms = Vec::new();
                for (i, info) in param_infos.iter().enumerate() {
                    let p_name = func_emitter.get_text(info.name_span);
                    if p_name.starts_with(b"$") {
                        let sym = func_emitter.interner.intern(&p_name[1..]);
                        param_syms.push(FuncParam {
                            name: sym,
                            by_ref: info.by_ref,
                        });
                        
                        if let Some(default_expr) = info.default {
                            let val = func_emitter.eval_constant_expr(default_expr);
                            let idx = func_emitter.add_constant(val);
                            func_emitter.chunk.code.push(OpCode::RecvInit(i as u32, idx as u16));
                        } else {
                            func_emitter.chunk.code.push(OpCode::Recv(i as u32));
                        }
                    }
                }

                let (mut func_chunk, is_generator) = func_emitter.compile(body);
                func_chunk.returns_ref = *by_ref;
                
                let user_func = UserFunc {
                    params: param_syms,
                    uses: Vec::new(),
                    chunk: Rc::new(func_chunk),
                    is_static: false,
                    is_generator,
                    statics: Rc::new(RefCell::new(HashMap::new())),
                };
                
                let func_res = Val::Resource(Rc::new(user_func));
                let const_idx = self.add_constant(func_res);
                
                self.chunk.code.push(OpCode::DefFunc(func_sym, const_idx as u32));
            }
            Stmt::Class { name, members, extends, implements, .. } => {
                let class_name_str = self.get_text(name.span);
                let class_sym = self.interner.intern(class_name_str);
                
                let parent_sym = if let Some(parent_name) = extends {
                    let parent_str = self.get_text(parent_name.span);
                    Some(self.interner.intern(parent_str))
                } else {
                    None
                };

                self.chunk.code.push(OpCode::DefClass(class_sym, parent_sym));
                
                for interface in *implements {
                    let interface_str = self.get_text(interface.span);
                    let interface_sym = self.interner.intern(interface_str);
                    self.chunk.code.push(OpCode::AddInterface(class_sym, interface_sym));
                }
                
                self.emit_members(class_sym, members);
            }
            Stmt::Interface { name, members, extends, .. } => {
                let name_str = self.get_text(name.span);
                let sym = self.interner.intern(name_str);
                
                self.chunk.code.push(OpCode::DefInterface(sym));
                
                for interface in *extends {
                    let interface_str = self.get_text(interface.span);
                    let interface_sym = self.interner.intern(interface_str);
                    self.chunk.code.push(OpCode::AddInterface(sym, interface_sym));
                }
                
                self.emit_members(sym, members);
            }
            Stmt::Trait { name, members, .. } => {
                let name_str = self.get_text(name.span);
                let sym = self.interner.intern(name_str);
                
                self.chunk.code.push(OpCode::DefTrait(sym));
                
                self.emit_members(sym, members);
            }

            Stmt::While { condition, body, .. } => {
                let start_label = self.chunk.code.len();
                
                self.emit_expr(condition);
                
                let end_jump = self.chunk.code.len();
                self.chunk.code.push(OpCode::JmpIfFalse(0)); // Patch later
                
                self.loop_stack.push(LoopInfo { break_jumps: Vec::new(), continue_jumps: Vec::new() });
                
                for stmt in *body {
                    self.emit_stmt(stmt);
                }
                
                self.chunk.code.push(OpCode::Jmp(start_label as u32));
                
                let end_label = self.chunk.code.len();
                self.chunk.code[end_jump] = OpCode::JmpIfFalse(end_label as u32);
                
                let loop_info = self.loop_stack.pop().unwrap();
                for idx in loop_info.break_jumps {
                    self.patch_jump(idx, end_label);
                }
                for idx in loop_info.continue_jumps {
                    self.patch_jump(idx, start_label);
                }
            }
            Stmt::DoWhile { body, condition, .. } => {
                let start_label = self.chunk.code.len();
                
                self.loop_stack.push(LoopInfo { break_jumps: Vec::new(), continue_jumps: Vec::new() });
                
                for stmt in *body {
                    self.emit_stmt(stmt);
                }
                
                let continue_label = self.chunk.code.len();
                self.emit_expr(condition);
                self.chunk.code.push(OpCode::JmpIfTrue(start_label as u32));
                
                let end_label = self.chunk.code.len();
                
                let loop_info = self.loop_stack.pop().unwrap();
                for idx in loop_info.break_jumps {
                    self.patch_jump(idx, end_label);
                }
                for idx in loop_info.continue_jumps {
                    self.patch_jump(idx, continue_label);
                }
            }
            Stmt::For { init, condition, loop_expr, body, .. } => {
                for expr in *init {
                    self.emit_expr(expr);
                    self.chunk.code.push(OpCode::Pop); // Discard result
                }
                
                let start_label = self.chunk.code.len();
                
                let mut end_jump = None;
                if !condition.is_empty() {
                    for (i, expr) in condition.iter().enumerate() {
                        self.emit_expr(expr);
                        if i < condition.len() - 1 {
                            self.chunk.code.push(OpCode::Pop);
                        }
                    }
                    end_jump = Some(self.chunk.code.len());
                    self.chunk.code.push(OpCode::JmpIfFalse(0)); // Patch later
                }
                
                self.loop_stack.push(LoopInfo { break_jumps: Vec::new(), continue_jumps: Vec::new() });
                
                for stmt in *body {
                    self.emit_stmt(stmt);
                }
                
                let continue_label = self.chunk.code.len();
                for expr in *loop_expr {
                    self.emit_expr(expr);
                    self.chunk.code.push(OpCode::Pop);
                }
                
                self.chunk.code.push(OpCode::Jmp(start_label as u32));
                
                let end_label = self.chunk.code.len();
                if let Some(idx) = end_jump {
                    self.chunk.code[idx] = OpCode::JmpIfFalse(end_label as u32);
                }
                
                let loop_info = self.loop_stack.pop().unwrap();
                for idx in loop_info.break_jumps {
                    self.patch_jump(idx, end_label);
                }
                for idx in loop_info.continue_jumps {
                    self.patch_jump(idx, continue_label);
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
            Stmt::Switch { condition, cases, .. } => {
                self.emit_expr(condition);
                
                let dispatch_jump = self.chunk.code.len();
                self.chunk.code.push(OpCode::Jmp(0)); // Patch later
                
                let mut case_labels = Vec::new();
                let mut default_label = None;
                
                self.loop_stack.push(LoopInfo { break_jumps: Vec::new(), continue_jumps: Vec::new() });
                
                for case in *cases {
                    let label = self.chunk.code.len();
                    case_labels.push(label);
                    
                    if case.condition.is_none() {
                        default_label = Some(label);
                    }
                    
                    for stmt in case.body {
                        self.emit_stmt(stmt);
                    }
                }
                
                let jump_over_dispatch = self.chunk.code.len();
                self.chunk.code.push(OpCode::Jmp(0)); // Patch to end_label
                
                let dispatch_start = self.chunk.code.len();
                self.patch_jump(dispatch_jump, dispatch_start);
                
                // Dispatch logic
                for (i, case) in cases.iter().enumerate() {
                    if let Some(cond) = case.condition {
                        self.chunk.code.push(OpCode::Dup); // Dup switch cond
                        self.emit_expr(cond);
                        self.chunk.code.push(OpCode::IsEqual); // Loose comparison
                        self.chunk.code.push(OpCode::JmpIfTrue(case_labels[i] as u32));
                    }
                }
                
                // Pop switch cond
                self.chunk.code.push(OpCode::Pop);
                
                if let Some(def_lbl) = default_label {
                    self.chunk.code.push(OpCode::Jmp(def_lbl as u32));
                } else {
                    // No default, jump to end
                    self.chunk.code.push(OpCode::Jmp(jump_over_dispatch as u32)); // Will be patched to end_label
                }
                
                let end_label = self.chunk.code.len();
                self.patch_jump(jump_over_dispatch, end_label);
                
                let loop_info = self.loop_stack.pop().unwrap();
                for idx in loop_info.break_jumps {
                    self.patch_jump(idx, end_label);
                }
                // Continue in switch acts like break
                for idx in loop_info.continue_jumps {
                    self.patch_jump(idx, end_label);
                }
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
            OpCode::JmpIfTrue(_) => OpCode::JmpIfTrue(target as u32),
            OpCode::JmpZEx(_) => OpCode::JmpZEx(target as u32),
            OpCode::JmpNzEx(_) => OpCode::JmpNzEx(target as u32),
            OpCode::Coalesce(_) => OpCode::Coalesce(target as u32),
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
            Expr::Array { items, .. } => {
                if items.is_empty() {
                    Some(Val::Array(indexmap::IndexMap::new()))
                } else {
                    None
                }
            }
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
            Expr::Float { value, .. } => {
                let s = std::str::from_utf8(value).unwrap_or("0.0");
                let f: f64 = s.parse().unwrap_or(0.0);
                let idx = self.add_constant(Val::Float(f));
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
                match op {
                    BinaryOp::And | BinaryOp::LogicalAnd => {
                        self.emit_expr(left);
                        let end_jump = self.chunk.code.len();
                        self.chunk.code.push(OpCode::JmpZEx(0));
                        self.emit_expr(right);
                        let end_label = self.chunk.code.len();
                        self.chunk.code[end_jump] = OpCode::JmpZEx(end_label as u32);
                        self.chunk.code.push(OpCode::Cast(1)); // Bool
                    }
                    BinaryOp::Or | BinaryOp::LogicalOr => {
                        self.emit_expr(left);
                        let end_jump = self.chunk.code.len();
                        self.chunk.code.push(OpCode::JmpNzEx(0));
                        self.emit_expr(right);
                        let end_label = self.chunk.code.len();
                        self.chunk.code[end_jump] = OpCode::JmpNzEx(end_label as u32);
                        self.chunk.code.push(OpCode::Cast(1)); // Bool
                    }
                    BinaryOp::Coalesce => {
                        self.emit_expr(left);
                        let end_jump = self.chunk.code.len();
                        self.chunk.code.push(OpCode::Coalesce(0));
                        self.emit_expr(right);
                        let end_label = self.chunk.code.len();
                        self.chunk.code[end_jump] = OpCode::Coalesce(end_label as u32);
                    }
                    _ => {
                        self.emit_expr(left);
                        self.emit_expr(right);
                        match op {
                            BinaryOp::Plus => self.chunk.code.push(OpCode::Add),
                            BinaryOp::Minus => self.chunk.code.push(OpCode::Sub),
                            BinaryOp::Mul => self.chunk.code.push(OpCode::Mul),
                            BinaryOp::Div => self.chunk.code.push(OpCode::Div),
                            BinaryOp::Mod => self.chunk.code.push(OpCode::Mod),
                            BinaryOp::Concat => self.chunk.code.push(OpCode::Concat),
                            BinaryOp::Pow => self.chunk.code.push(OpCode::Pow),
                            BinaryOp::BitAnd => self.chunk.code.push(OpCode::BitwiseAnd),
                            BinaryOp::BitOr => self.chunk.code.push(OpCode::BitwiseOr),
                            BinaryOp::BitXor => self.chunk.code.push(OpCode::BitwiseXor),
                            BinaryOp::ShiftLeft => self.chunk.code.push(OpCode::ShiftLeft),
                            BinaryOp::ShiftRight => self.chunk.code.push(OpCode::ShiftRight),
                            BinaryOp::EqEq => self.chunk.code.push(OpCode::IsEqual),
                            BinaryOp::EqEqEq => self.chunk.code.push(OpCode::IsIdentical),
                            BinaryOp::NotEq => self.chunk.code.push(OpCode::IsNotEqual),
                            BinaryOp::NotEqEq => self.chunk.code.push(OpCode::IsNotIdentical),
                            BinaryOp::Gt => self.chunk.code.push(OpCode::IsGreater),
                            BinaryOp::Lt => self.chunk.code.push(OpCode::IsLess),
                            BinaryOp::GtEq => self.chunk.code.push(OpCode::IsGreaterOrEqual),
                            BinaryOp::LtEq => self.chunk.code.push(OpCode::IsLessOrEqual),
                            BinaryOp::Spaceship => self.chunk.code.push(OpCode::Spaceship),
                            BinaryOp::Instanceof => self.chunk.code.push(OpCode::InstanceOf),
                            BinaryOp::LogicalXor => self.chunk.code.push(OpCode::BoolXor),
                            _ => {} 
                        }
                    }
                }
            }
            Expr::Match { condition, arms, .. } => {
                self.emit_expr(condition);
                
                let mut end_jumps = Vec::new();
                
                for arm in *arms {
                    if let Some(conds) = arm.conditions {
                        let mut body_jump_indices = Vec::new();
                        
                        for cond in conds {
                            self.chunk.code.push(OpCode::Dup);
                            self.emit_expr(cond);
                            self.chunk.code.push(OpCode::IsIdentical); // Strict
                            
                            let jump_idx = self.chunk.code.len();
                            self.chunk.code.push(OpCode::JmpIfTrue(0)); // Jump to body
                            body_jump_indices.push(jump_idx);
                        }
                        
                        // If we are here, none matched. Jump to next arm.
                        let skip_body_idx = self.chunk.code.len();
                        self.chunk.code.push(OpCode::Jmp(0)); 
                        
                        // Body start
                        let body_start = self.chunk.code.len();
                        for idx in body_jump_indices {
                            self.patch_jump(idx, body_start);
                        }
                        
                        // Pop condition before body
                        self.chunk.code.push(OpCode::Pop); 
                        self.emit_expr(arm.body);
                        
                        // Jump to end
                        end_jumps.push(self.chunk.code.len());
                        self.chunk.code.push(OpCode::Jmp(0));
                        
                        // Patch skip_body_idx to here (next arm)
                        self.patch_jump(skip_body_idx, self.chunk.code.len());
                        
                    } else {
                        // Default arm
                        self.chunk.code.push(OpCode::Pop); // Pop condition
                        self.emit_expr(arm.body);
                        end_jumps.push(self.chunk.code.len());
                        self.chunk.code.push(OpCode::Jmp(0));
                    }
                }
                
                // No match found
                self.chunk.code.push(OpCode::MatchError);
                
                let end_label = self.chunk.code.len();
                for idx in end_jumps {
                    self.patch_jump(idx, end_label);
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
                        // 0 - expr
                        let idx = self.add_constant(Val::Int(0));
                        self.chunk.code.push(OpCode::Const(idx as u16));
                        self.emit_expr(expr);
                        self.chunk.code.push(OpCode::Sub);
                    }
                    UnaryOp::Not => {
                        self.emit_expr(expr);
                        self.chunk.code.push(OpCode::BoolNot);
                    }
                    UnaryOp::BitNot => {
                        self.emit_expr(expr);
                        self.chunk.code.push(OpCode::BitwiseNot);
                    }
                    UnaryOp::PreInc => {
                        if let Expr::Variable { span, .. } = expr {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let sym = self.interner.intern(&name[1..]);
                                self.chunk.code.push(OpCode::MakeVarRef(sym));
                                self.chunk.code.push(OpCode::PreInc);
                            }
                        }
                    }
                    UnaryOp::PreDec => {
                        if let Expr::Variable { span, .. } = expr {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let sym = self.interner.intern(&name[1..]);
                                self.chunk.code.push(OpCode::MakeVarRef(sym));
                                self.chunk.code.push(OpCode::PreDec);
                            }
                        }
                    }
                    _ => {
                        self.emit_expr(expr);
                    }
                }
            }
            Expr::PostInc { var, .. } => {
                if let Expr::Variable { span, .. } = var {
                    let name = self.get_text(*span);
                    if name.starts_with(b"$") {
                        let sym = self.interner.intern(&name[1..]);
                        self.chunk.code.push(OpCode::MakeVarRef(sym));
                        self.chunk.code.push(OpCode::PostInc);
                    }
                }
            }
            Expr::PostDec { var, .. } => {
                if let Expr::Variable { span, .. } = var {
                    let name = self.get_text(*span);
                    if name.starts_with(b"$") {
                        let sym = self.interner.intern(&name[1..]);
                        self.chunk.code.push(OpCode::MakeVarRef(sym));
                        self.chunk.code.push(OpCode::PostDec);
                    }
                }
            }
            Expr::Ternary { condition, if_true, if_false, .. } => {
                self.emit_expr(condition);
                if let Some(true_expr) = if_true {
                    // cond ? true : false
                    let else_jump = self.chunk.code.len();
                    self.chunk.code.push(OpCode::JmpIfFalse(0)); // Placeholder
                    
                    self.emit_expr(true_expr);
                    let end_jump = self.chunk.code.len();
                    self.chunk.code.push(OpCode::Jmp(0)); // Placeholder
                    
                    let else_label = self.chunk.code.len();
                    self.chunk.code[else_jump] = OpCode::JmpIfFalse(else_label as u32);
                    
                    self.emit_expr(if_false);
                    let end_label = self.chunk.code.len();
                    self.chunk.code[end_jump] = OpCode::Jmp(end_label as u32);
                } else {
                    // cond ?: false (Elvis)
                    let end_jump = self.chunk.code.len();
                    self.chunk.code.push(OpCode::JmpNzEx(0)); // Placeholder
                    
                    self.chunk.code.push(OpCode::Pop); // Pop cond if false
                    self.emit_expr(if_false);
                    
                    let end_label = self.chunk.code.len();
                    self.chunk.code[end_jump] = OpCode::JmpNzEx(end_label as u32);
                }
            }
            Expr::Cast { kind, expr, .. } => {
                self.emit_expr(expr);
                // Map CastKind to OpCode::Cast(u8)
                // 0=Int, 1=Bool, 2=Float, 3=String, 4=Array, 5=Object, 6=Unset
                let cast_op = match kind {
                    CastKind::Int => 0,
                    CastKind::Bool => 1,
                    CastKind::Float => 2,
                    CastKind::String => 3,
                    CastKind::Array => 4,
                    CastKind::Object => 5,
                    CastKind::Unset => 6,
                    _ => 0, // TODO
                };
                self.chunk.code.push(OpCode::Cast(cast_op));
            }
            Expr::Clone { expr, .. } => {
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::Clone);
            }
            Expr::Exit { expr, .. } | Expr::Die { expr, .. } => {
                if let Some(e) = expr {
                    self.emit_expr(e);
                } else {
                    let idx = self.add_constant(Val::Null);
                    self.chunk.code.push(OpCode::Const(idx as u16));
                }
                self.chunk.code.push(OpCode::Exit);
            }
            Expr::Isset { vars, .. } => {
                if vars.is_empty() {
                    let idx = self.add_constant(Val::Bool(false));
                    self.chunk.code.push(OpCode::Const(idx as u16));
                } else {
                    let mut end_jumps = Vec::new();
                    
                    for (i, var) in vars.iter().enumerate() {
                        match var {
                            Expr::Variable { span, .. } => {
                                let name = self.get_text(*span);
                                if name.starts_with(b"$") {
                                    let sym = self.interner.intern(&name[1..]);
                                    self.chunk.code.push(OpCode::IssetVar(sym));
                                } else {
                                    let idx = self.add_constant(Val::Bool(false));
                                    self.chunk.code.push(OpCode::Const(idx as u16));
                                }
                            }
                            Expr::ArrayDimFetch { array, dim, .. } => {
                                self.emit_expr(array);
                                if let Some(d) = dim {
                                    self.emit_expr(d);
                                    self.chunk.code.push(OpCode::IssetDim);
                                } else {
                                     let idx = self.add_constant(Val::Bool(false));
                                     self.chunk.code.push(OpCode::Const(idx as u16));
                                }
                            }
                            Expr::PropertyFetch { target, property, .. } => {
                                self.emit_expr(target);
                                if let Expr::Variable { span, .. } = property {
                                    let name = self.get_text(*span);
                                    let sym = self.interner.intern(name);
                                    self.chunk.code.push(OpCode::IssetProp(sym));
                                } else {
                                    self.chunk.code.push(OpCode::Pop);
                                    let idx = self.add_constant(Val::Bool(false));
                                    self.chunk.code.push(OpCode::Const(idx as u16));
                                }
                            }
                            Expr::ClassConstFetch { class, constant, .. } => {
                                let is_static_prop = if let Expr::Variable { span, .. } = constant {
                                    let name = self.get_text(*span);
                                    name.starts_with(b"$")
                                } else {
                                    false
                                };

                                if is_static_prop {
                                    if let Expr::Variable { span, .. } = class {
                                        let name = self.get_text(*span);
                                        if !name.starts_with(b"$") {
                                            let idx = self.add_constant(Val::String(name.to_vec()));
                                            self.chunk.code.push(OpCode::Const(idx as u16));
                                        } else {
                                            let sym = self.interner.intern(&name[1..]);
                                            self.chunk.code.push(OpCode::LoadVar(sym));
                                        }
                                        
                                        if let Expr::Variable { span: prop_span, .. } = constant {
                                            let prop_name = self.get_text(*prop_span);
                                            let prop_sym = self.interner.intern(&prop_name[1..]);
                                            self.chunk.code.push(OpCode::IssetStaticProp(prop_sym));
                                        }
                                    } else {
                                        let idx = self.add_constant(Val::Bool(false));
                                        self.chunk.code.push(OpCode::Const(idx as u16));
                                    }
                                } else {
                                    let idx = self.add_constant(Val::Bool(false));
                                    self.chunk.code.push(OpCode::Const(idx as u16));
                                }
                            }
                            _ => {
                                let idx = self.add_constant(Val::Bool(false));
                                self.chunk.code.push(OpCode::Const(idx as u16));
                            }
                        }
                        
                        if i < vars.len() - 1 {
                            self.chunk.code.push(OpCode::Dup);
                            let jump_idx = self.chunk.code.len();
                            self.chunk.code.push(OpCode::JmpIfFalse(0));
                            self.chunk.code.push(OpCode::Pop); 
                            end_jumps.push(jump_idx);
                        }
                    }
                    
                    let end_label = self.chunk.code.len();
                    for idx in end_jumps {
                        self.patch_jump(idx, end_label);
                    }
                }
            }
            Expr::Empty { expr, .. } => {
                match expr {
                    Expr::Variable { span, .. } => {
                        let name = self.get_text(*span);
                        if name.starts_with(b"$") {
                            let sym = self.interner.intern(&name[1..]);
                            self.chunk.code.push(OpCode::IssetVar(sym));
                        } else {
                            let idx = self.add_constant(Val::Bool(false));
                            self.chunk.code.push(OpCode::Const(idx as u16));
                        }
                    }
                    Expr::ArrayDimFetch { array, dim, .. } => {
                        self.emit_expr(array);
                        if let Some(d) = dim {
                            self.emit_expr(d);
                            self.chunk.code.push(OpCode::IssetDim);
                        } else {
                             let idx = self.add_constant(Val::Bool(false));
                             self.chunk.code.push(OpCode::Const(idx as u16));
                        }
                    }
                    Expr::PropertyFetch { target, property, .. } => {
                        self.emit_expr(target);
                        if let Expr::Variable { span, .. } = property {
                            let name = self.get_text(*span);
                            let sym = self.interner.intern(name);
                            self.chunk.code.push(OpCode::IssetProp(sym));
                        } else {
                            self.chunk.code.push(OpCode::Pop);
                            let idx = self.add_constant(Val::Bool(false));
                            self.chunk.code.push(OpCode::Const(idx as u16));
                        }
                    }
                    Expr::ClassConstFetch { class, constant, .. } => {
                        let is_static_prop = if let Expr::Variable { span, .. } = constant {
                            let name = self.get_text(*span);
                            name.starts_with(b"$")
                        } else {
                            false
                        };

                        if is_static_prop {
                            if let Expr::Variable { span, .. } = class {
                                let name = self.get_text(*span);
                                if !name.starts_with(b"$") {
                                    let idx = self.add_constant(Val::String(name.to_vec()));
                                    self.chunk.code.push(OpCode::Const(idx as u16));
                                } else {
                                    let sym = self.interner.intern(&name[1..]);
                                    self.chunk.code.push(OpCode::LoadVar(sym));
                                }
                                
                                if let Expr::Variable { span: prop_span, .. } = constant {
                                    let prop_name = self.get_text(*prop_span);
                                    let prop_sym = self.interner.intern(&prop_name[1..]);
                                    self.chunk.code.push(OpCode::IssetStaticProp(prop_sym));
                                }
                            } else {
                                let idx = self.add_constant(Val::Bool(false));
                                self.chunk.code.push(OpCode::Const(idx as u16));
                            }
                        } else {
                            let idx = self.add_constant(Val::Bool(false));
                            self.chunk.code.push(OpCode::Const(idx as u16));
                        }
                    }
                    _ => {
                        self.emit_expr(expr);
                        self.chunk.code.push(OpCode::BoolNot);
                        return;
                    }
                }
                
                let jump_if_not_set = self.chunk.code.len();
                self.chunk.code.push(OpCode::JmpIfFalse(0));
                
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::BoolNot);
                
                let jump_end = self.chunk.code.len();
                self.chunk.code.push(OpCode::Jmp(0));
                
                let label_true = self.chunk.code.len();
                self.patch_jump(jump_if_not_set, label_true);
                
                self.chunk.code.push(OpCode::Pop);
                let idx = self.add_constant(Val::Bool(true));
                self.chunk.code.push(OpCode::Const(idx as u16));
                
                let label_end = self.chunk.code.len();
                self.patch_jump(jump_end, label_end);
            }
            Expr::Eval { expr, .. } => {
                self.emit_expr(expr);
                self.chunk.code.push(OpCode::Include);
            }
            Expr::Yield { key, value, from, .. } => {
                self.is_generator = true;
                if *from {
                    if let Some(v) = value {
                        self.emit_expr(v);
                    } else {
                        let idx = self.add_constant(Val::Null);
                        self.chunk.code.push(OpCode::Const(idx as u16));
                    }
                    self.chunk.code.push(OpCode::YieldFrom);
                } else {
                    let has_key = key.is_some();
                    if let Some(k) = key {
                        self.emit_expr(k);
                    }
                    
                    if let Some(v) = value {
                        self.emit_expr(v);
                    } else {
                        let idx = self.add_constant(Val::Null);
                        self.chunk.code.push(OpCode::Const(idx as u16));
                    }
                    self.chunk.code.push(OpCode::Yield(has_key));
                    self.chunk.code.push(OpCode::GetSentValue);
                }
            }
            Expr::Closure { params, uses, body, by_ref, is_static, .. } => {
                // 1. Collect param info
                struct ParamInfo<'a> {
                    name_span: php_parser::span::Span,
                    by_ref: bool,
                    default: Option<&'a Expr<'a>>,
                }
                
                let mut param_infos = Vec::new();
                for param in *params {
                    param_infos.push(ParamInfo {
                        name_span: param.name.span,
                        by_ref: param.by_ref,
                        default: param.default.as_ref().map(|e| *e),
                    });
                }

                // 2. Create emitter
                let mut func_emitter = Emitter::new(self.source, self.interner);
                
                // 3. Process params
                let mut param_syms = Vec::new();
                for (i, info) in param_infos.iter().enumerate() {
                    let p_name = func_emitter.get_text(info.name_span);
                    if p_name.starts_with(b"$") {
                        let sym = func_emitter.interner.intern(&p_name[1..]);
                        param_syms.push(FuncParam {
                            name: sym,
                            by_ref: info.by_ref,
                        });
                        
                        if let Some(default_expr) = info.default {
                            let val = func_emitter.eval_constant_expr(default_expr);
                            let idx = func_emitter.add_constant(val);
                            func_emitter.chunk.code.push(OpCode::RecvInit(i as u32, idx as u16));
                        } else {
                            func_emitter.chunk.code.push(OpCode::Recv(i as u32));
                        }
                    }
                }

                let (mut func_chunk, is_generator) = func_emitter.compile(body);
                func_chunk.returns_ref = *by_ref;
                
                // Extract uses
                let mut use_syms = Vec::new();
                for use_var in *uses {
                    let u_name = self.get_text(use_var.var.span);
                    if u_name.starts_with(b"$") {
                        let sym = self.interner.intern(&u_name[1..]);
                        use_syms.push(sym);
                        
                        if use_var.by_ref {
                            self.chunk.code.push(OpCode::LoadRef(sym));
                        } else {
                            // Emit code to push the captured variable onto the stack
                            self.chunk.code.push(OpCode::LoadVar(sym));
                            self.chunk.code.push(OpCode::Copy);
                        }
                    }
                }
                
                let user_func = UserFunc {
                    params: param_syms,
                    uses: use_syms.clone(),
                    chunk: Rc::new(func_chunk),
                    is_static: *is_static,
                    is_generator,
                    statics: Rc::new(RefCell::new(HashMap::new())),
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
                self.chunk.code.push(OpCode::InitArray(items.len() as u32));
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
                    } else {
                        // Dynamic new $var()
                        // Emit expression to get class name (string)
                        self.emit_expr(class);
                        
                        for arg in *args {
                            self.emit_expr(arg.value);
                        }
                        
                        self.chunk.code.push(OpCode::NewDynamic(args.len() as u8));
                    }
                } else {
                    // Complex expression for class name
                    self.emit_expr(class);
                    
                    for arg in *args {
                        self.emit_expr(arg.value);
                    }
                    
                    self.chunk.code.push(OpCode::NewDynamic(args.len() as u8));
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
                let mut is_class_keyword = false;
                if let Expr::Variable { span: const_span, .. } = constant {
                     let const_name = self.get_text(*const_span);
                     if const_name.eq_ignore_ascii_case(b"class") {
                         is_class_keyword = true;
                     }
                }

                if let Expr::Variable { span, .. } = class {
                    let class_name = self.get_text(*span);
                    if !class_name.starts_with(b"$") {
                        if is_class_keyword {
                            let idx = self.add_constant(Val::String(class_name.to_vec()));
                            self.chunk.code.push(OpCode::Const(idx as u16));
                            return;
                        }

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
                        return;
                    }
                }

                // Dynamic class/object access
                self.emit_expr(class);
                if is_class_keyword {
                    self.chunk.code.push(OpCode::GetClass);
                } else {
                    // TODO: Dynamic class constant fetch
                    self.chunk.code.push(OpCode::Pop);
                    let idx = self.add_constant(Val::Null);
                    self.chunk.code.push(OpCode::Const(idx as u16));
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
                        
                        if let Expr::PropertyFetch { target, property, .. } = base {
                            self.emit_expr(target);
                            self.chunk.code.push(OpCode::Dup);
                            
                            if let Expr::Variable { span, .. } = property {
                                let name = self.get_text(*span);
                                if !name.starts_with(b"$") {
                                    let sym = self.interner.intern(name);
                                    self.chunk.code.push(OpCode::FetchProp(sym));
                                    
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
                                    
                                    self.chunk.code.push(OpCode::AssignProp(sym));
                                }
                            }
                        } else {
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
                                    self.chunk.code.push(OpCode::LoadVar(sym));
                                }
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
                            
                            if let AssignOp::Coalesce = op {
                                // Check if set
                                self.chunk.code.push(OpCode::IssetVar(sym));
                                let jump_idx = self.chunk.code.len();
                                self.chunk.code.push(OpCode::JmpIfTrue(0));
                                
                                // Not set: Evaluate expr, assign, load
                                self.emit_expr(expr);
                                self.chunk.code.push(OpCode::StoreVar(sym));
                                self.chunk.code.push(OpCode::LoadVar(sym));
                                
                                let end_jump_idx = self.chunk.code.len();
                                self.chunk.code.push(OpCode::Jmp(0));
                                
                                // Set: Load var
                                let label_set = self.chunk.code.len();
                                self.chunk.code[jump_idx] = OpCode::JmpIfTrue(label_set as u32);
                                self.chunk.code.push(OpCode::LoadVar(sym));
                                
                                // End
                                let label_end = self.chunk.code.len();
                                self.chunk.code[end_jump_idx] = OpCode::Jmp(label_end as u32);
                                return;
                            }

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
                                AssignOp::Mod => self.chunk.code.push(OpCode::Mod),
                                AssignOp::Concat => self.chunk.code.push(OpCode::Concat),
                                AssignOp::Pow => self.chunk.code.push(OpCode::Pow),
                                AssignOp::BitAnd => self.chunk.code.push(OpCode::BitwiseAnd),
                                AssignOp::BitOr => self.chunk.code.push(OpCode::BitwiseOr),
                                AssignOp::BitXor => self.chunk.code.push(OpCode::BitwiseXor),
                                AssignOp::ShiftLeft => self.chunk.code.push(OpCode::ShiftLeft),
                                AssignOp::ShiftRight => self.chunk.code.push(OpCode::ShiftRight),
                                _ => {} // TODO: Implement other ops
                            }
                            
                            // Store
                            self.chunk.code.push(OpCode::StoreVar(sym));
                        }
                    }
                    Expr::PropertyFetch { target, property, .. } => {
                        self.emit_expr(target);
                        self.chunk.code.push(OpCode::Dup);
                        
                        if let Expr::Variable { span, .. } = property {
                            let name = self.get_text(*span);
                            if !name.starts_with(b"$") {
                                let sym = self.interner.intern(name);
                                
                                if let AssignOp::Coalesce = op {
                                    self.chunk.code.push(OpCode::Dup);
                                    self.chunk.code.push(OpCode::IssetProp(sym));
                                    let jump_idx = self.chunk.code.len();
                                    self.chunk.code.push(OpCode::JmpIfTrue(0));
                                    
                                    self.emit_expr(expr);
                                    self.chunk.code.push(OpCode::AssignProp(sym));
                                    
                                    let end_jump_idx = self.chunk.code.len();
                                    self.chunk.code.push(OpCode::Jmp(0));
                                    
                                    let label_set = self.chunk.code.len();
                                    self.chunk.code[jump_idx] = OpCode::JmpIfTrue(label_set as u32);
                                    self.chunk.code.push(OpCode::FetchProp(sym));
                                    
                                    let label_end = self.chunk.code.len();
                                    self.chunk.code[end_jump_idx] = OpCode::Jmp(label_end as u32);
                                    return;
                                }
                                
                                self.chunk.code.push(OpCode::FetchProp(sym));
                                
                                self.emit_expr(expr);
                                
                                match op {
                                    AssignOp::Plus => self.chunk.code.push(OpCode::Add),
                                    AssignOp::Minus => self.chunk.code.push(OpCode::Sub),
                                    AssignOp::Mul => self.chunk.code.push(OpCode::Mul),
                                    AssignOp::Div => self.chunk.code.push(OpCode::Div),
                                    AssignOp::Mod => self.chunk.code.push(OpCode::Mod),
                                    AssignOp::Concat => self.chunk.code.push(OpCode::Concat),
                                    AssignOp::Pow => self.chunk.code.push(OpCode::Pow),
                                    AssignOp::BitAnd => self.chunk.code.push(OpCode::BitwiseAnd),
                                    AssignOp::BitOr => self.chunk.code.push(OpCode::BitwiseOr),
                                    AssignOp::BitXor => self.chunk.code.push(OpCode::BitwiseXor),
                                    AssignOp::ShiftLeft => self.chunk.code.push(OpCode::ShiftLeft),
                                    AssignOp::ShiftRight => self.chunk.code.push(OpCode::ShiftRight),
                                    _ => {}
                                }
                                
                                self.chunk.code.push(OpCode::AssignProp(sym));
                            }
                        }
                    }
                    Expr::ArrayDimFetch { .. } => {
                        let (base, keys) = Self::flatten_dim_fetch(var);
                        
                        // 1. Emit base array
                        self.emit_expr(base);
                        
                        // 2. Emit keys
                        for key in &keys {
                            if let Some(k) = key {
                                self.emit_expr(k);
                            } else {
                                // Append not supported in AssignOp (e.g. $a[] += 1 is invalid)
                                // But maybe $a[] ??= 1 is valid? No, ??= is assign op.
                                // PHP Fatal error:  Cannot use [] for reading
                                // So we can assume keys are present for AssignOp (read-modify-write)
                                // But wait, $a[] = 1 is valid. $a[] += 1 is NOT valid.
                                // So we can panic or emit error if key is None.
                                // For now, push 0 or null?
                                // Actually, let's just push 0 as placeholder, but it will fail at runtime if used for reading.
                                self.chunk.code.push(OpCode::Const(0));
                            }
                        }
                        
                        // 3. Fetch value (peek array & keys, push val)
                        // Stack: [array, keys...]
                        self.chunk.code.push(OpCode::FetchNestedDim(keys.len() as u8));
                        // Stack: [array, keys..., val]
                        
                        if let AssignOp::Coalesce = op {
                            let jump_idx = self.chunk.code.len();
                            self.chunk.code.push(OpCode::Coalesce(0));
                            
                            // If null, evaluate rhs
                            self.emit_expr(expr);
                            
                            let label_store = self.chunk.code.len();
                            self.chunk.code[jump_idx] = OpCode::Coalesce(label_store as u32);
                        } else {
                            // 4. Emit expr (rhs)
                            self.emit_expr(expr);
                            // Stack: [array, keys..., val, rhs]
                            
                            // 5. Op
                            match op {
                                AssignOp::Plus => self.chunk.code.push(OpCode::Add),
                                AssignOp::Minus => self.chunk.code.push(OpCode::Sub),
                                AssignOp::Mul => self.chunk.code.push(OpCode::Mul),
                                AssignOp::Div => self.chunk.code.push(OpCode::Div),
                                AssignOp::Mod => self.chunk.code.push(OpCode::Mod),
                                AssignOp::Concat => self.chunk.code.push(OpCode::Concat),
                                AssignOp::Pow => self.chunk.code.push(OpCode::Pow),
                                AssignOp::BitAnd => self.chunk.code.push(OpCode::BitwiseAnd),
                                AssignOp::BitOr => self.chunk.code.push(OpCode::BitwiseOr),
                                AssignOp::BitXor => self.chunk.code.push(OpCode::BitwiseXor),
                                AssignOp::ShiftLeft => self.chunk.code.push(OpCode::ShiftLeft),
                                AssignOp::ShiftRight => self.chunk.code.push(OpCode::ShiftRight),
                                _ => {}
                            }
                        }
                        
                        // 6. Store result back
                        // Stack: [array, keys..., result]
                        self.chunk.code.push(OpCode::StoreNestedDim(keys.len() as u8));
                        // Stack: [new_array] (StoreNestedDim pushes the modified array back? No, wait.)
                        
                        // Wait, I checked StoreNestedDim implementation.
                        // It does NOT push anything back.
                        // But assign_nested_dim pushes new_handle back!
                        // And StoreNestedDim calls assign_nested_dim.
                        // So StoreNestedDim DOES push new_array back.
                        
                        // So Stack: [new_array]
                        
                        // 7. Update variable if base was a variable
                        if let Expr::Variable { span, .. } = base {
                            let name = self.get_text(*span);
                            if name.starts_with(b"$") {
                                let var_name = &name[1..];
                                let sym = self.interner.intern(var_name);
                                self.chunk.code.push(OpCode::StoreVar(sym));
                                // StoreVar leaves value on stack?
                                // OpCode::StoreVar implementation:
                                // let val_handle = self.operand_stack.pop()...;
                                // ...
                                // self.operand_stack.push(val_handle);
                                // Yes, it leaves value on stack.
                            }
                        }
                    }
                    _ => {} // TODO: Other targets
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

    fn eval_constant_expr(&self, expr: &Expr) -> Val {
        match expr {
            Expr::Integer { value, .. } => {
                let s_str = std::str::from_utf8(value).unwrap_or("0");
                if let Ok(i) = s_str.parse::<i64>() {
                    Val::Int(i)
                } else {
                    Val::Int(0)
                }
            }
            Expr::Float { value, .. } => {
                let s_str = std::str::from_utf8(value).unwrap_or("0.0");
                if let Ok(f) = s_str.parse::<f64>() {
                    Val::Float(f)
                } else {
                    Val::Float(0.0)
                }
            }
            Expr::String { value, .. } => {
                 let s = value;
                 if s.len() >= 2 && ((s[0] == b'"' && s[s.len()-1] == b'"') || (s[0] == b'\'' && s[s.len()-1] == b'\'')) {
                     Val::String(s[1..s.len()-1].to_vec())
                 } else {
                     Val::String(s.to_vec())
                 }
            }
            Expr::Boolean { value, .. } => Val::Bool(*value),
            Expr::Null { .. } => Val::Null,
            _ => Val::Null,
        }
    }
    
    fn get_text(&self, span: php_parser::span::Span) -> &'src [u8] {
        &self.source[span.start..span.end]
    }
}
