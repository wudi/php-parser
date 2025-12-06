use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::core::heap::Arena;
use crate::core::value::{Val, ArrayKey, Handle, ObjectData, Symbol, Visibility};
use crate::vm::stack::Stack;
use crate::vm::opcode::OpCode;
use crate::compiler::chunk::{CodeChunk, UserFunc, ClosureData, FuncParam};
use crate::vm::frame::{CallFrame, GeneratorData, GeneratorState, SubIterator, SubGenState};
use crate::runtime::context::{RequestContext, EngineContext, ClassDef};

#[derive(Debug)]
pub enum VmError {
    RuntimeError(String),
    Exception(Handle),
}

pub struct VM {
    pub arena: Arena,
    pub operand_stack: Stack,
    pub frames: Vec<CallFrame>,
    pub context: RequestContext,
    pub last_return_value: Option<Handle>,
}

impl VM {
    pub fn new(engine_context: Arc<EngineContext>) -> Self {
        Self {
            arena: Arena::new(),
            operand_stack: Stack::new(),
            frames: Vec::new(),
            context: RequestContext::new(engine_context),
            last_return_value: None,
        }
    }

    pub fn new_with_context(context: RequestContext) -> Self {
        Self {
            arena: Arena::new(),
            operand_stack: Stack::new(),
            frames: Vec::new(),
            context,
            last_return_value: None,
        }
    }

    fn find_method(&self, class_name: Symbol, method_name: Symbol) -> Option<(Rc<UserFunc>, Visibility, bool, Symbol)> {
        let mut current_class = Some(class_name);
        while let Some(name) = current_class {
            if let Some(def) = self.context.classes.get(&name) {
                if let Some((func, vis, is_static)) = def.methods.get(&method_name) {
                    return Some((func.clone(), *vis, *is_static, name));
                }
                current_class = def.parent;
            } else {
                break;
            }
        }
        None
    }

    fn collect_properties(&mut self, class_name: Symbol) -> IndexMap<Symbol, Handle> {
        let mut properties = IndexMap::new();
        let mut chain = Vec::new();
        let mut current_class = Some(class_name);
        
        while let Some(name) = current_class {
            if let Some(def) = self.context.classes.get(&name) {
                chain.push(def);
                current_class = def.parent;
            } else {
                break;
            }
        }
        
        for def in chain.iter().rev() {
            for (name, (default_val, _)) in &def.properties {
                let handle = self.arena.alloc(default_val.clone());
                properties.insert(*name, handle);
            }
        }
        
        properties
    }

    fn is_subclass_of(&self, child: Symbol, parent: Symbol) -> bool {
        if child == parent { return true; }
        
        if let Some(def) = self.context.classes.get(&child) {
            // Check parent class
            if let Some(p) = def.parent {
                if self.is_subclass_of(p, parent) {
                    return true;
                }
            }
            // Check interfaces
            for &interface in &def.interfaces {
                if self.is_subclass_of(interface, parent) {
                    return true;
                }
            }
        }
        false
    }

    fn resolve_class_name(&self, class_name: Symbol) -> Result<Symbol, VmError> {
        let name_bytes = self.context.interner.lookup(class_name).ok_or(VmError::RuntimeError("Invalid class symbol".into()))?;
        if name_bytes.eq_ignore_ascii_case(b"self") {
             let frame = self.frames.last().ok_or(VmError::RuntimeError("No active frame".into()))?;
             return frame.class_scope.ok_or(VmError::RuntimeError("Cannot access self:: when no class scope is active".into()));
        }
        if name_bytes.eq_ignore_ascii_case(b"parent") {
             let frame = self.frames.last().ok_or(VmError::RuntimeError("No active frame".into()))?;
             let scope = frame.class_scope.ok_or(VmError::RuntimeError("Cannot access parent:: when no class scope is active".into()))?;
             let class_def = self.context.classes.get(&scope).ok_or(VmError::RuntimeError("Class not found".into()))?;
             return class_def.parent.ok_or(VmError::RuntimeError("Parent not found".into()));
        }
        if name_bytes.eq_ignore_ascii_case(b"static") {
             let frame = self.frames.last().ok_or(VmError::RuntimeError("No active frame".into()))?;
             return frame.called_scope.ok_or(VmError::RuntimeError("Cannot access static:: when no called scope is active".into()));
        }
        Ok(class_name)
    }

    fn find_class_constant(&self, start_class: Symbol, const_name: Symbol) -> Result<(Val, Visibility, Symbol), VmError> {
        let mut current_class = start_class;
        loop {
            if let Some(class_def) = self.context.classes.get(&current_class) {
                if let Some((val, vis)) = class_def.constants.get(&const_name) {
                    if *vis == Visibility::Private && current_class != start_class {
                         let const_str = String::from_utf8_lossy(self.context.interner.lookup(const_name).unwrap_or(b"???"));
                         return Err(VmError::RuntimeError(format!("Undefined class constant {}", const_str)));
                    }
                    return Ok((val.clone(), *vis, current_class));
                }
                if let Some(parent) = class_def.parent {
                    current_class = parent;
                } else {
                    break;
                }
            } else {
                let class_str = String::from_utf8_lossy(self.context.interner.lookup(start_class).unwrap_or(b"???"));
                return Err(VmError::RuntimeError(format!("Class {} not found", class_str)));
            }
        }
        let const_str = String::from_utf8_lossy(self.context.interner.lookup(const_name).unwrap_or(b"???"));
        Err(VmError::RuntimeError(format!("Undefined class constant {}", const_str)))
    }

    fn find_static_prop(&self, start_class: Symbol, prop_name: Symbol) -> Result<(Val, Visibility, Symbol), VmError> {
        let mut current_class = start_class;
        loop {
            if let Some(class_def) = self.context.classes.get(&current_class) {
                if let Some((val, vis)) = class_def.static_properties.get(&prop_name) {
                    if *vis == Visibility::Private && current_class != start_class {
                         let prop_str = String::from_utf8_lossy(self.context.interner.lookup(prop_name).unwrap_or(b"???"));
                         return Err(VmError::RuntimeError(format!("Undefined static property ${}", prop_str)));
                    }
                    return Ok((val.clone(), *vis, current_class));
                }
                if let Some(parent) = class_def.parent {
                    current_class = parent;
                } else {
                    break;
                }
            } else {
                let class_str = String::from_utf8_lossy(self.context.interner.lookup(start_class).unwrap_or(b"???"));
                return Err(VmError::RuntimeError(format!("Class {} not found", class_str)));
            }
        }
        let prop_str = String::from_utf8_lossy(self.context.interner.lookup(prop_name).unwrap_or(b"???"));
        Err(VmError::RuntimeError(format!("Undefined static property ${}", prop_str)))
    }

    fn check_const_visibility(&self, defining_class: Symbol, visibility: Visibility) -> Result<(), VmError> {
        match visibility {
            Visibility::Public => Ok(()),
            Visibility::Private => {
                let frame = self.frames.last().ok_or(VmError::RuntimeError("No active frame".into()))?;
                let scope = frame.class_scope.ok_or(VmError::RuntimeError("Cannot access private constant".into()))?;
                if scope == defining_class {
                    Ok(())
                } else {
                    Err(VmError::RuntimeError("Cannot access private constant".into()))
                }
            }
            Visibility::Protected => {
                let frame = self.frames.last().ok_or(VmError::RuntimeError("No active frame".into()))?;
                let scope = frame.class_scope.ok_or(VmError::RuntimeError("Cannot access protected constant".into()))?;
                if self.is_subclass_of(scope, defining_class) || self.is_subclass_of(defining_class, scope) {
                    Ok(())
                } else {
                    Err(VmError::RuntimeError("Cannot access protected constant".into()))
                }
            }
        }
    }

    pub(crate) fn get_current_class(&self) -> Option<Symbol> {
        self.frames.last().and_then(|f| f.class_scope)
    }

    pub(crate) fn check_prop_visibility(&self, class_name: Symbol, prop_name: Symbol, current_scope: Option<Symbol>) -> Result<(), VmError> {
        let mut current = Some(class_name);
        let mut defined_vis = None;
        let mut defined_class = None;
        
        while let Some(name) = current {
            if let Some(def) = self.context.classes.get(&name) {
                if let Some((_, vis)) = def.properties.get(&prop_name) {
                    defined_vis = Some(*vis);
                    defined_class = Some(name);
                    break;
                }
                current = def.parent;
            } else {
                break;
            }
        }
        
        if let Some(vis) = defined_vis {
            match vis {
                Visibility::Public => Ok(()),
                Visibility::Private => {
                    if current_scope == defined_class {
                        Ok(())
                    } else {
                        Err(VmError::RuntimeError(format!("Cannot access private property")))
                    }
                },
                Visibility::Protected => {
                    if let Some(scope) = current_scope {
                        if self.is_subclass_of(scope, defined_class.unwrap()) || self.is_subclass_of(defined_class.unwrap(), scope) {
                             Ok(())
                        } else {
                             Err(VmError::RuntimeError(format!("Cannot access protected property")))
                        }
                    } else {
                        Err(VmError::RuntimeError(format!("Cannot access protected property")))
                    }
                }
            }
        } else {
            // Dynamic property, public by default
            Ok(())
        }
    }

    fn is_instance_of(&self, obj_handle: Handle, class_sym: Symbol) -> bool {
        let obj_val = self.arena.get(obj_handle);
        if let Val::Object(payload_handle) = obj_val.value {
            if let Val::ObjPayload(data) = &self.arena.get(payload_handle).value {
                let obj_class = data.class;
                if obj_class == class_sym {
                    return true;
                }
                return self.is_subclass_of(obj_class, class_sym);
            }
        }
        false
    }

    fn handle_exception(&mut self, ex_handle: Handle) -> bool {
        let mut frame_idx = self.frames.len();
        while frame_idx > 0 {
            frame_idx -= 1;
            
            let (ip, chunk) = {
                let frame = &self.frames[frame_idx];
                let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 } as u32;
                (ip, frame.chunk.clone())
            };
            
            for entry in &chunk.catch_table {
                if ip >= entry.start && ip < entry.end {
                    let matches = if let Some(type_sym) = entry.catch_type {
                        self.is_instance_of(ex_handle, type_sym)
                    } else {
                        true
                    };
                    
                    if matches {
                        self.frames.truncate(frame_idx + 1);
                        let frame = &mut self.frames[frame_idx];
                        frame.ip = entry.target as usize;
                        self.operand_stack.push(ex_handle);
                        return true;
                    }
                }
            }
        }
        self.frames.clear();
        false
    }

    pub fn run(&mut self, chunk: Rc<CodeChunk>) -> Result<(), VmError> {
        let initial_frame = CallFrame::new(chunk);
        self.frames.push(initial_frame);

        while !self.frames.is_empty() {
            let op = {
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.code.len() {
                    self.frames.pop();
                    continue;
                }
                let op = frame.chunk.code[frame.ip].clone();
                frame.ip += 1;
                op
            };

            let res = (|| -> Result<(), VmError> { match op {
                OpCode::Throw => {
                    let ex_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    return Err(VmError::Exception(ex_handle));
                }
                OpCode::Catch => {}
                OpCode::Const(idx) => {
                    let frame = self.frames.last().unwrap();
                    let val = frame.chunk.constants[idx as usize].clone();
                    let handle = self.arena.alloc(val);
                    self.operand_stack.push(handle);
                }
                OpCode::Pop => {
                    self.operand_stack.pop();
                }
                OpCode::Dup => {
                    let handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    self.operand_stack.push(handle);
                }
                OpCode::BitwiseNot => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle).value.clone();
                    let res = match val {
                        Val::Int(i) => Val::Int(!i),
                        _ => Val::Null, // TODO: Support other types
                    };
                    let res_handle = self.arena.alloc(res);
                    self.operand_stack.push(res_handle);
                }
                OpCode::BoolNot => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle);
                    let b = match val.value {
                        Val::Bool(v) => v,
                        Val::Int(v) => v != 0,
                        Val::Null => false,
                        _ => true, 
                    };
                    let res_handle = self.arena.alloc(Val::Bool(!b));
                    self.operand_stack.push(res_handle);
                }
                OpCode::Add => self.binary_op(|a, b| a + b)?,
                OpCode::Sub => self.binary_op(|a, b| a - b)?,
                OpCode::Mul => self.binary_op(|a, b| a * b)?,
                OpCode::Div => self.binary_op(|a, b| a / b)?,
                OpCode::Mod => self.binary_op(|a, b| a % b)?,
                OpCode::Pow => self.binary_op(|a, b| a.pow(b as u32))?,
                OpCode::BitwiseAnd => self.binary_op(|a, b| a & b)?,
                OpCode::BitwiseOr => self.binary_op(|a, b| a | b)?,
                OpCode::BitwiseXor => self.binary_op(|a, b| a ^ b)?,
                OpCode::ShiftLeft => self.binary_op(|a, b| a << b)?,
                OpCode::ShiftRight => self.binary_op(|a, b| a >> b)?,
                
                OpCode::LoadVar(sym) => {
                    let frame = self.frames.last().unwrap();
                    if let Some(&handle) = frame.locals.get(&sym) {
                        self.operand_stack.push(handle);
                    } else {
                        // Check for $this
                        let name = self.context.interner.lookup(sym);
                        if name == Some(b"this") {
                            if let Some(this_handle) = frame.this {
                                self.operand_stack.push(this_handle);
                            } else {
                                return Err(VmError::RuntimeError("Using $this when not in object context".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError(format!("Undefined variable: {:?}", sym)));
                        }
                    }
                }
                OpCode::LoadRef(sym) => {
                    let frame = self.frames.last_mut().unwrap();
                    if let Some(&handle) = frame.locals.get(&sym) {
                        if self.arena.get(handle).is_ref {
                            self.operand_stack.push(handle);
                        } else {
                            // Convert to ref. Clone to ensure uniqueness/safety.
                            let val = self.arena.get(handle).value.clone();
                            let new_handle = self.arena.alloc(val);
                            self.arena.get_mut(new_handle).is_ref = true;
                            frame.locals.insert(sym, new_handle);
                            self.operand_stack.push(new_handle);
                        }
                    } else {
                        // Undefined variable, create as Null ref
                        let handle = self.arena.alloc(Val::Null);
                        self.arena.get_mut(handle).is_ref = true;
                        frame.locals.insert(sym, handle);
                        self.operand_stack.push(handle);
                    }
                }
                OpCode::StoreVar(sym) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let frame = self.frames.last_mut().unwrap();
                    
                    // Check if the target variable is a reference
                    let mut is_target_ref = false;
                    if let Some(&old_handle) = frame.locals.get(&sym) {
                        if self.arena.get(old_handle).is_ref {
                            is_target_ref = true;
                            // Assigning to a reference: update the value in place
                            let new_val = self.arena.get(val_handle).value.clone();
                            self.arena.get_mut(old_handle).value = new_val;
                        }
                    }
                    
                    if !is_target_ref {
                        // Not assigning to a reference.
                        // Check if we need to unref (copy) the value from the stack
                        let final_handle = if self.arena.get(val_handle).is_ref {
                            let val = self.arena.get(val_handle).value.clone();
                            self.arena.alloc(val)
                        } else {
                            val_handle
                        };
                        
                        frame.locals.insert(sym, final_handle);
                    }
                }
                OpCode::AssignRef(sym) => {
                    let ref_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    // Mark the handle as a reference (idempotent if already ref)
                    self.arena.get_mut(ref_handle).is_ref = true;
                    
                    let frame = self.frames.last_mut().unwrap();
                    // Overwrite the local slot with the reference handle
                    frame.locals.insert(sym, ref_handle);
                }
                OpCode::AssignOp(op) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let var_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    if self.arena.get(var_handle).is_ref {
                        let current_val = self.arena.get(var_handle).value.clone();
                        let val = self.arena.get(val_handle).value.clone();
                        
                        let res = match op {
                            0 => match (current_val, val) { // Add
                                (Val::Int(a), Val::Int(b)) => Val::Int(a + b),
                                _ => Val::Null,
                            },
                            1 => match (current_val, val) { // Sub
                                (Val::Int(a), Val::Int(b)) => Val::Int(a - b),
                                _ => Val::Null,
                            },
                            2 => match (current_val, val) { // Mul
                                (Val::Int(a), Val::Int(b)) => Val::Int(a * b),
                                _ => Val::Null,
                            },
                            3 => match (current_val, val) { // Div
                                (Val::Int(a), Val::Int(b)) => Val::Int(a / b),
                                _ => Val::Null,
                            },
                            4 => match (current_val, val) { // Mod
                                (Val::Int(a), Val::Int(b)) => {
                                    if b == 0 {
                                        return Err(VmError::RuntimeError("Modulo by zero".into()));
                                    }
                                    Val::Int(a % b)
                                },
                                _ => Val::Null,
                            },
                            5 => match (current_val, val) { // ShiftLeft
                                (Val::Int(a), Val::Int(b)) => Val::Int(a << b),
                                _ => Val::Null,
                            },
                            6 => match (current_val, val) { // ShiftRight
                                (Val::Int(a), Val::Int(b)) => Val::Int(a >> b),
                                _ => Val::Null,
                            },
                            7 => match (current_val, val) { // Concat
                                (Val::String(a), Val::String(b)) => {
                                    let mut s = String::from_utf8_lossy(&a).to_string();
                                    s.push_str(&String::from_utf8_lossy(&b));
                                    Val::String(s.into_bytes())
                                },
                                (Val::String(a), Val::Int(b)) => {
                                    let mut s = String::from_utf8_lossy(&a).to_string();
                                    s.push_str(&b.to_string());
                                    Val::String(s.into_bytes())
                                },
                                (Val::Int(a), Val::String(b)) => {
                                    let mut s = a.to_string();
                                    s.push_str(&String::from_utf8_lossy(&b));
                                    Val::String(s.into_bytes())
                                },
                                _ => Val::Null,
                            },
                            8 => match (current_val, val) { // BitwiseOr
                                (Val::Int(a), Val::Int(b)) => Val::Int(a | b),
                                _ => Val::Null,
                            },
                            9 => match (current_val, val) { // BitwiseAnd
                                (Val::Int(a), Val::Int(b)) => Val::Int(a & b),
                                _ => Val::Null,
                            },
                            10 => match (current_val, val) { // BitwiseXor
                                (Val::Int(a), Val::Int(b)) => Val::Int(a ^ b),
                                _ => Val::Null,
                            },
                            11 => match (current_val, val) { // Pow
                                (Val::Int(a), Val::Int(b)) => {
                                    if b < 0 {
                                        return Err(VmError::RuntimeError("Negative exponent not supported for int pow".into()));
                                    }
                                    Val::Int(a.pow(b as u32))
                                },
                                _ => Val::Null,
                            },
                            _ => Val::Null,
                        };
                        
                        self.arena.get_mut(var_handle).value = res.clone();
                        let res_handle = self.arena.alloc(res);
                        self.operand_stack.push(res_handle);
                    } else {
                        return Err(VmError::RuntimeError("AssignOp on non-reference".into()));
                    }
                }
                OpCode::PreInc => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    if self.arena.get(handle).is_ref {
                        let val = &self.arena.get(handle).value;
                        let new_val = match val {
                            Val::Int(i) => Val::Int(i + 1),
                            _ => Val::Null,
                        };
                        self.arena.get_mut(handle).value = new_val.clone();
                        let res_handle = self.arena.alloc(new_val);
                        self.operand_stack.push(res_handle);
                    } else {
                         return Err(VmError::RuntimeError("PreInc on non-reference".into()));
                    }
                }
                OpCode::PreDec => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    if self.arena.get(handle).is_ref {
                        let val = &self.arena.get(handle).value;
                        let new_val = match val {
                            Val::Int(i) => Val::Int(i - 1),
                            _ => Val::Null,
                        };
                        self.arena.get_mut(handle).value = new_val.clone();
                        let res_handle = self.arena.alloc(new_val);
                        self.operand_stack.push(res_handle);
                    } else {
                         return Err(VmError::RuntimeError("PreDec on non-reference".into()));
                    }
                }
                OpCode::PostInc => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    if self.arena.get(handle).is_ref {
                        let val = self.arena.get(handle).value.clone();
                        let new_val = match &val {
                            Val::Int(i) => Val::Int(i + 1),
                            _ => Val::Null,
                        };
                        self.arena.get_mut(handle).value = new_val;
                        let res_handle = self.arena.alloc(val); // Return OLD value
                        self.operand_stack.push(res_handle);
                    } else {
                         return Err(VmError::RuntimeError("PostInc on non-reference".into()));
                    }
                }
                OpCode::PostDec => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    if self.arena.get(handle).is_ref {
                        let val = self.arena.get(handle).value.clone();
                        let new_val = match &val {
                            Val::Int(i) => Val::Int(i - 1),
                            _ => Val::Null,
                        };
                        self.arena.get_mut(handle).value = new_val;
                        let res_handle = self.arena.alloc(val); // Return OLD value
                        self.operand_stack.push(res_handle);
                    } else {
                         return Err(VmError::RuntimeError("PostDec on non-reference".into()));
                    }
                }
                OpCode::MakeVarRef(sym) => {
                    let frame = self.frames.last_mut().unwrap();
                    
                    // Get current handle or create NULL
                    let handle = if let Some(&h) = frame.locals.get(&sym) {
                        h
                    } else {
                        let null = self.arena.alloc(Val::Null);
                        frame.locals.insert(sym, null);
                        null
                    };
                    
                    // Check if it is already a ref
                    if self.arena.get(handle).is_ref {
                        self.operand_stack.push(handle);
                    } else {
                        // Not a ref. We must upgrade it.
                        // To avoid affecting other variables sharing this handle, we MUST clone.
                        let val = self.arena.get(handle).value.clone();
                        let new_handle = self.arena.alloc(val);
                        self.arena.get_mut(new_handle).is_ref = true;
                        
                        // Update the local variable to point to the new ref handle
                        let frame = self.frames.last_mut().unwrap();
                        frame.locals.insert(sym, new_handle);
                        
                        self.operand_stack.push(new_handle);
                    }
                }
                OpCode::UnsetVar(sym) => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.remove(&sym);
                }
                OpCode::BindGlobal(sym) => {
                    let global_handle = self.context.globals.get(&sym).copied();
                    
                    let handle = if let Some(h) = global_handle {
                        h
                    } else {
                        // Check main frame (frame 0) for the variable
                        let main_handle = if !self.frames.is_empty() {
                            self.frames[0].locals.get(&sym).copied()
                        } else {
                            None
                        };
                        
                        if let Some(h) = main_handle {
                            h
                        } else {
                            self.arena.alloc(Val::Null)
                        }
                    };
                    
                    // Ensure it is in globals map
                    self.context.globals.insert(sym, handle);
                    
                    // Mark as reference
                    self.arena.get_mut(handle).is_ref = true;
                    
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.insert(sym, handle);
                }
                OpCode::BindStatic(sym, default_idx) => {
                    let frame = self.frames.last_mut().unwrap();
                    
                    if let Some(func) = &frame.func {
                        let mut statics = func.statics.borrow_mut();
                        
                        let handle = if let Some(h) = statics.get(&sym) {
                            *h
                        } else {
                            // Initialize with default value
                            let val = frame.chunk.constants[default_idx as usize].clone();
                            let h = self.arena.alloc(val);
                            statics.insert(sym, h);
                            h
                        };
                        
                        // Mark as reference so StoreVar updates it in place
                        self.arena.get_mut(handle).is_ref = true;
                        
                        // Bind to local
                        frame.locals.insert(sym, handle);
                    } else {
                        return Err(VmError::RuntimeError("BindStatic called outside of function".into()));
                    }
                }
                OpCode::MakeRef => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    if self.arena.get(handle).is_ref {
                        self.operand_stack.push(handle);
                    } else {
                        // Convert to ref. Clone to ensure uniqueness/safety.
                        let val = self.arena.get(handle).value.clone();
                        let new_handle = self.arena.alloc(val);
                        self.arena.get_mut(new_handle).is_ref = true;
                        self.operand_stack.push(new_handle);
                    }
                }
                
                OpCode::Jmp(target) => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                }
                OpCode::JmpIfFalse(target) => {
                    let condition_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let condition_val = self.arena.get(condition_handle);
                    
                    let is_false = match condition_val.value {
                        Val::Bool(b) => !b,
                        Val::Int(i) => i == 0,
                        Val::Null => true,
                        _ => false, 
                    };
                    
                    if is_false {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }
                OpCode::JmpIfTrue(target) => {
                    let condition_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let condition_val = self.arena.get(condition_handle);
                    
                    let is_true = match condition_val.value {
                        Val::Bool(b) => b,
                        Val::Int(i) => i != 0,
                        Val::Null => false,
                        _ => true, 
                    };
                    
                    if is_true {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }
                OpCode::JmpZEx(target) => {
                    let condition_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let condition_val = self.arena.get(condition_handle);
                    
                    let is_false = match condition_val.value {
                        Val::Bool(b) => !b,
                        Val::Int(i) => i == 0,
                        Val::Null => true,
                        _ => false, 
                    };
                    
                    if is_false {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    } else {
                        self.operand_stack.pop();
                    }
                }
                OpCode::JmpNzEx(target) => {
                    let condition_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let condition_val = self.arena.get(condition_handle);
                    
                    let is_true = match condition_val.value {
                        Val::Bool(b) => b,
                        Val::Int(i) => i != 0,
                        Val::Null => false,
                        _ => true, 
                    };
                    
                    if is_true {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    } else {
                        self.operand_stack.pop();
                    }
                }
                OpCode::Coalesce(target) => {
                    let handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle);
                    
                    let is_null = matches!(val.value, Val::Null);
                    
                    if !is_null {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    } else {
                        self.operand_stack.pop();
                    }
                }

                OpCode::Echo => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle);
                    match &val.value {
                        Val::String(s) => {
                            let s = String::from_utf8_lossy(s);
                            print!("{}", s);
                        }
                        Val::Int(i) => print!("{}", i),
                        Val::Float(f) => print!("{}", f),
                        Val::Bool(b) => print!("{}", if *b { "1" } else { "" }),
                        Val::Null => {},
                        _ => print!("{:?}", val.value),
                    }
                }
                OpCode::Exit => {
                    if let Some(handle) = self.operand_stack.pop() {
                        let val = self.arena.get(handle);
                        match &val.value {
                            Val::String(s) => {
                                let s = String::from_utf8_lossy(s);
                                print!("{}", s);
                            }
                            Val::Int(_) => {}
                            _ => {}
                        }
                    }
                    self.frames.clear();
                    return Ok(());
                }
                OpCode::Silence(_) => {}
                OpCode::Ticks(_) => {}
                OpCode::Cast(kind) => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle).value.clone();
                    
                    // Special handling for Object -> String (3)
                    if kind == 3 {
                        if let Val::Object(h) = val {
                             let obj_zval = self.arena.get(h);
                             if let Val::ObjPayload(obj_data) = &obj_zval.value {
                                let to_string_magic = self.context.interner.intern(b"__toString");
                                if let Some((magic_func, _, _, magic_class)) = self.find_method(obj_data.class, to_string_magic) {
                                    // Found __toString
                                    let mut frame = CallFrame::new(magic_func.chunk.clone());
                                    frame.func = Some(magic_func.clone());
                                    frame.this = Some(h);
                                    frame.class_scope = Some(magic_class);
                                    frame.called_scope = Some(obj_data.class);
                                    self.frames.push(frame);
                                    return Ok(());
                                } else {
                                    return Err(VmError::RuntimeError("Object could not be converted to string".into()));
                                }
                             } else {
                                return Err(VmError::RuntimeError("Invalid object payload".into()));
                             }
                        }
                    }

                    let new_val = match kind {
                        0 => match val { // Int
                            Val::Int(i) => Val::Int(i),
                            Val::Float(f) => Val::Int(f as i64),
                            Val::Bool(b) => Val::Int(if b { 1 } else { 0 }),
                            Val::String(s) => {
                                let s = String::from_utf8_lossy(&s);
                                Val::Int(s.parse().unwrap_or(0))
                            }
                            Val::Null => Val::Int(0),
                            _ => Val::Int(0),
                        },
                        1 => match val { // Bool
                            Val::Bool(b) => Val::Bool(b),
                            Val::Int(i) => Val::Bool(i != 0),
                            Val::Null => Val::Bool(false),
                            _ => Val::Bool(true),
                        },
                        2 => match val { // Float
                            Val::Float(f) => Val::Float(f),
                            Val::Int(i) => Val::Float(i as f64),
                            Val::String(s) => {
                                let s = String::from_utf8_lossy(&s);
                                Val::Float(s.parse().unwrap_or(0.0))
                            }
                            _ => Val::Float(0.0),
                        },
                        3 => match val { // String
                            Val::String(s) => Val::String(s),
                            Val::Int(i) => Val::String(i.to_string().into_bytes()),
                            Val::Float(f) => Val::String(f.to_string().into_bytes()),
                            Val::Bool(b) => Val::String(if b { b"1".to_vec() } else { b"".to_vec() }),
                            Val::Null => Val::String(Vec::new()),
                            Val::Object(_) => unreachable!(), // Handled above
                            _ => Val::String(b"Array".to_vec()),
                        },
                        4 => match val { // Array
                            Val::Array(a) => Val::Array(a),
                            Val::Null => Val::Array(IndexMap::new()),
                            _ => {
                                let mut map = IndexMap::new();
                                map.insert(ArrayKey::Int(0), self.arena.alloc(val));
                                Val::Array(map)
                            }
                        },
                        5 => match val { // Object
                            Val::Object(h) => Val::Object(h),
                            Val::Array(a) => {
                                let mut props = IndexMap::new();
                                for (k, v) in a {
                                    let key_sym = match k {
                                        ArrayKey::Int(i) => self.context.interner.intern(i.to_string().as_bytes()),
                                        ArrayKey::Str(s) => self.context.interner.intern(&s),
                                    };
                                    props.insert(key_sym, v);
                                }
                                let obj_data = ObjectData {
                                    class: self.context.interner.intern(b"stdClass"),
                                    properties: props,
                                    internal: None,
                                };
                                let payload = self.arena.alloc(Val::ObjPayload(obj_data));
                                Val::Object(payload)
                            },
                            Val::Null => {
                                let obj_data = ObjectData {
                                    class: self.context.interner.intern(b"stdClass"),
                                    properties: IndexMap::new(),
                                    internal: None,
                                };
                                let payload = self.arena.alloc(Val::ObjPayload(obj_data));
                                Val::Object(payload)
                            },
                            _ => {
                                let mut props = IndexMap::new();
                                let key_sym = self.context.interner.intern(b"scalar");
                                props.insert(key_sym, self.arena.alloc(val));
                                let obj_data = ObjectData {
                                    class: self.context.interner.intern(b"stdClass"),
                                    properties: props,
                                    internal: None,
                                };
                                let payload = self.arena.alloc(Val::ObjPayload(obj_data));
                                Val::Object(payload)
                            }
                        },
                        6 => Val::Null, // Unset
                        _ => val,
                    };
                    let res_handle = self.arena.alloc(new_val);
                    self.operand_stack.push(res_handle);
                }
                OpCode::TypeCheck => {}
                OpCode::Defined => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = &self.arena.get(handle).value;
                    let name = match val {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("defined() expects string".into())),
                    };
                    
                    let defined = self.context.constants.contains_key(&name) || self.context.engine.constants.contains_key(&name);
                    let res_handle = self.arena.alloc(Val::Bool(defined));
                    self.operand_stack.push(res_handle);
                }
                OpCode::Match => {}
                OpCode::MatchError => {
                    return Err(VmError::RuntimeError("UnhandledMatchError".into()));
                }

                OpCode::Closure(func_idx, num_captures) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[func_idx as usize].clone()
                    };
                    
                    let user_func = if let Val::Resource(rc) = val {
                        if let Ok(func) = rc.downcast::<UserFunc>() {
                            func
                        } else {
                            return Err(VmError::RuntimeError("Invalid function constant for closure".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError("Invalid function constant for closure".into()));
                    };
                    
                    let mut captures = IndexMap::new();
                    let mut captured_vals = Vec::with_capacity(num_captures as usize);
                    for _ in 0..num_captures {
                        captured_vals.push(self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?);
                    }
                    captured_vals.reverse();
                    
                    for (i, sym) in user_func.uses.iter().enumerate() {
                        if i < captured_vals.len() {
                            captures.insert(*sym, captured_vals[i]);
                        }
                    }
                    
                    let this_handle = if user_func.is_static {
                        None
                    } else {
                        let frame = self.frames.last().unwrap();
                        frame.this
                    };
                    
                    let closure_data = ClosureData {
                        func: user_func,
                        captures,
                        this: this_handle,
                    };
                    
                    let closure_class_sym = self.context.interner.intern(b"Closure");
                    let obj_data = ObjectData {
                        class: closure_class_sym,
                        properties: IndexMap::new(),
                        internal: Some(Rc::new(closure_data)),
                    };
                    
                    let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                    let obj_handle = self.arena.alloc(Val::Object(payload_handle));
                    self.operand_stack.push(obj_handle);
                }

                OpCode::Call(arg_count) => {
                    let mut args = Vec::with_capacity(arg_count as usize);
                    for _ in 0..arg_count {
                        args.push(self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?);
                    }
                    args.reverse();
                    
                    let func_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let func_val = self.arena.get(func_handle);
                    
                    match &func_val.value {
                        Val::String(s) => {
                            let func_name_bytes = s.clone();
                            let handler = self.context.engine.functions.get(&func_name_bytes).copied();
                            
                            if let Some(handler) = handler {
                                let result_handle = handler(self, &args).map_err(VmError::RuntimeError)?;
                                self.operand_stack.push(result_handle);
                            } else {
                                let sym = self.context.interner.intern(&func_name_bytes);
                                if let Some(user_func) = self.context.user_functions.get(&sym).cloned() {
                                    if user_func.params.len() != args.len() {
                                        // return Err(VmError::RuntimeError(format!("Function expects {} args, got {}", user_func.params.len(), args.len())));
                                        // PHP allows extra args, but warns on missing. For now, ignore.
                                    }
                                    
                                    let mut frame = CallFrame::new(user_func.chunk.clone());
                                    frame.func = Some(user_func.clone());
                                    for (i, param) in user_func.params.iter().enumerate() {
                                        if i < args.len() {
                                            let arg_handle = args[i];
                                            if param.by_ref {
                                                // Pass by reference: ensure arg is a ref
                                                if !self.arena.get(arg_handle).is_ref {
                                                    // If passed value is not a ref, we must upgrade it?
                                                    // PHP Error: "Only variables can be passed by reference"
                                                    // But here we just have a handle.
                                                    // If it's a literal, we can't make it a ref to a variable.
                                                    // But for now, let's just mark it as ref if it isn't?
                                                    // Actually, if the caller passed a variable, they should have used MakeVarRef?
                                                    // No, the caller doesn't know if the function expects a ref at compile time (unless we check signature).
                                                    // In PHP, the call site must use `foo(&$a)` if it wants to be explicit, but modern PHP allows `foo($a)` if the function is defined as `function foo(&$a)`.
                                                    // So the VM must handle the upgrade.
                                                    
                                                    // We need to check if we can make it a ref.
                                                    // For now, we just mark it.
                                                    self.arena.get_mut(arg_handle).is_ref = true;
                                                }
                                                frame.locals.insert(param.name, arg_handle);
                                            } else {
                                                // Pass by value: if arg is ref, we must deref (copy value)
                                                let final_handle = if self.arena.get(arg_handle).is_ref {
                                                    let val = self.arena.get(arg_handle).value.clone();
                                                    self.arena.alloc(val)
                                                } else {
                                                    arg_handle
                                                };
                                                frame.locals.insert(param.name, final_handle);
                                            }
                                        }
                                    }
                                    
                                    if user_func.is_generator {
                                        let gen_data = GeneratorData {
                                            state: GeneratorState::Created(frame),
                                            current_val: None,
                                            current_key: None,
                                            auto_key: 0,
                                            sub_iter: None,
                                            sent_val: None,
                                        };
                                        let obj_data = ObjectData {
                                            class: self.context.interner.intern(b"Generator"),
                                            properties: IndexMap::new(),
                                            internal: Some(Rc::new(RefCell::new(gen_data))),
                                        };
                                        let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                                        let obj_handle = self.arena.alloc(Val::Object(payload_handle));
                                        self.operand_stack.push(obj_handle);
                                    } else {
                                        self.frames.push(frame);
                                    }
                                } else {
                                    return Err(VmError::RuntimeError(format!("Undefined function: {:?}", String::from_utf8_lossy(&func_name_bytes))));
                                }
                            }
                        }
                        Val::Object(payload_handle) => {
                            let mut closure_data = None;
                            let mut obj_class = None;
                            
                            {
                                let payload_val = self.arena.get(*payload_handle);
                                if let Val::ObjPayload(obj_data) = &payload_val.value {
                                    if let Some(internal) = &obj_data.internal {
                                        if let Ok(closure) = internal.clone().downcast::<ClosureData>() {
                                            closure_data = Some(closure);
                                        }
                                    }
                                    if closure_data.is_none() {
                                        obj_class = Some(obj_data.class);
                                    }
                                }
                            }
                            
                            if let Some(closure) = closure_data {
                                let mut frame = CallFrame::new(closure.func.chunk.clone());
                                frame.func = Some(closure.func.clone());
                                
                                for (i, param) in closure.func.params.iter().enumerate() {
                                    if i < args.len() {
                                        let arg_handle = args[i];
                                        if param.by_ref {
                                            if !self.arena.get(arg_handle).is_ref {
                                                self.arena.get_mut(arg_handle).is_ref = true;
                                            }
                                            frame.locals.insert(param.name, arg_handle);
                                        } else {
                                            let final_handle = if self.arena.get(arg_handle).is_ref {
                                                let val = self.arena.get(arg_handle).value.clone();
                                                self.arena.alloc(val)
                                            } else {
                                                arg_handle
                                            };
                                            frame.locals.insert(param.name, final_handle);
                                        }
                                    }
                                }
                                
                                for (sym, handle) in &closure.captures {
                                    frame.locals.insert(*sym, *handle);
                                }
                                
                                frame.this = closure.this;
                                
                                self.frames.push(frame);
                            } else if let Some(class_name) = obj_class {
                                // Check for __invoke
                                let invoke_sym = self.context.interner.intern(b"__invoke");
                                let method_lookup = self.find_method(class_name, invoke_sym);
                                
                                if let Some((method, _, _, _)) = method_lookup {
                                    let mut frame = CallFrame::new(method.chunk.clone());
                                    frame.func = Some(method.clone());
                                    frame.this = Some(*payload_handle);
                                    frame.class_scope = Some(class_name);
                                    
                                    for (i, param) in method.params.iter().enumerate() {
                                        if i < args.len() {
                                            let arg_handle = args[i];
                                            if param.by_ref {
                                                if !self.arena.get(arg_handle).is_ref {
                                                    self.arena.get_mut(arg_handle).is_ref = true;
                                                }
                                                frame.locals.insert(param.name, arg_handle);
                                            } else {
                                                let final_handle = if self.arena.get(arg_handle).is_ref {
                                                    let val = self.arena.get(arg_handle).value.clone();
                                                    self.arena.alloc(val)
                                                } else {
                                                    arg_handle
                                                };
                                                frame.locals.insert(param.name, final_handle);
                                            }
                                        }
                                    }
                                    
                                    self.frames.push(frame);
                                } else {
                                    return Err(VmError::RuntimeError("Object is not a closure and does not implement __invoke".into()));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid object payload".into()));
                            }
                        }
                        _ => return Err(VmError::RuntimeError("Call expects function name or closure".into())),
                    }
                }

                OpCode::Return => {
                    let ret_val = if self.operand_stack.is_empty() {
                        self.arena.alloc(Val::Null)
                    } else {
                        self.operand_stack.pop().unwrap()
                    };

                    let popped_frame = self.frames.pop().expect("Frame stack empty on Return");

                    if let Some(gen_handle) = popped_frame.generator {
                        let gen_val = self.arena.get(gen_handle);
                        if let Val::Object(payload_handle) = &gen_val.value {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        let mut data = gen_data.borrow_mut();
                                        data.state = GeneratorState::Finished;
                                    }
                                }
                            }
                        }
                    }

                    // Handle return by reference
                    let final_ret_val = if popped_frame.chunk.returns_ref {
                        // Function returns by reference: keep the handle as is (even if it is a ref)
                        // But we must ensure it IS a ref?
                        // PHP: "Only variable references should be returned by reference"
                        // If we return a literal, PHP notices.
                        // But here we just pass the handle.
                        // If the handle points to a value that is NOT a ref, should we make it a ref?
                        // No, usually you return a variable which might be a ref.
                        // If you return $a, and $a is not a ref, but function is &foo(), then $a becomes a ref?
                        // Yes, implicitly.
                        if !self.arena.get(ret_val).is_ref {
                             self.arena.get_mut(ret_val).is_ref = true;
                        }
                        ret_val
                    } else {
                        // Function returns by value: if ret_val is a ref, dereference (copy) it.
                        if self.arena.get(ret_val).is_ref {
                            let val = self.arena.get(ret_val).value.clone();
                            self.arena.alloc(val)
                        } else {
                            ret_val
                        }
                    };

                    if self.frames.is_empty() {
                        self.last_return_value = Some(final_ret_val);
                        return Ok(());
                    }

                    if popped_frame.discard_return {
                        // Return value is discarded
                    } else if popped_frame.is_constructor {
                        if let Some(this_handle) = popped_frame.this {
                            self.operand_stack.push(this_handle);
                        } else {
                             return Err(VmError::RuntimeError("Constructor frame missing 'this'".into()));
                        }
                    } else {
                        self.operand_stack.push(final_ret_val);
                    }
                }
                OpCode::Recv(_) => {}
                OpCode::RecvInit(_, _) => {}
                OpCode::SendVal => {} 
                OpCode::SendVar => {}
                OpCode::SendRef => {}
                OpCode::Yield(has_key) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let key_handle = if has_key {
                        Some(self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?)
                    } else {
                        None
                    };
                    
                    let mut frame = self.frames.pop().ok_or(VmError::RuntimeError("No frame to yield from".into()))?;
                    let gen_handle = frame.generator.ok_or(VmError::RuntimeError("Yield outside of generator context".into()))?;
                    
                    let gen_val = self.arena.get(gen_handle);
                    if let Val::Object(payload_handle) = &gen_val.value {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                    let mut data = gen_data.borrow_mut();
                                    data.current_val = Some(val_handle);
                                    
                                    if let Some(k) = key_handle {
                                        data.current_key = Some(k);
                                        if let Val::Int(i) = self.arena.get(k).value {
                                            data.auto_key = i + 1;
                                        }
                                    } else {
                                        let k = data.auto_key;
                                        data.auto_key += 1;
                                        let k_handle = self.arena.alloc(Val::Int(k));
                                        data.current_key = Some(k_handle);
                                    }
                                    
                                    data.state = GeneratorState::Suspended(frame);
                                }
                            }
                        }
                    }
                    
                    // Yield pauses execution of this frame. The value is stored in GeneratorData.
                    // We don't push anything to the stack here. The sent value will be retrieved
                    // by OpCode::GetSentValue when the generator is resumed.
                }
                OpCode::YieldFrom => {
                    let frame_idx = self.frames.len() - 1;
                    let frame = &mut self.frames[frame_idx];
                    let gen_handle = frame.generator.ok_or(VmError::RuntimeError("YieldFrom outside of generator context".into()))?;
                    println!("YieldFrom: Parent generator {:?}", gen_handle);
                    
                    let (mut sub_iter, is_new) = {
                        let gen_val = self.arena.get(gen_handle);
                        if let Val::Object(payload_handle) = &gen_val.value {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    println!("YieldFrom: Parent internal ptr: {:p}", internal);
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        println!("YieldFrom: Parent gen_data ptr: {:p}", gen_data);
                                        let mut data = gen_data.borrow_mut();
                                        if let Some(iter) = &data.sub_iter {
                                            (iter.clone(), false)
                                        } else {
                                            let iterable_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                                            let iter = match &self.arena.get(iterable_handle).value {
                                                Val::Array(_) => SubIterator::Array { handle: iterable_handle, index: 0 },
                                                Val::Object(_) => SubIterator::Generator { handle: iterable_handle, state: SubGenState::Initial },
                                                val => return Err(VmError::RuntimeError(format!("Yield from expects array or traversable, got {:?}", val))),
                                            };
                                            data.sub_iter = Some(iter.clone());
                                            (iter, true)
                                        }
                                    } else {
                                        return Err(VmError::RuntimeError("Invalid generator data".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Invalid generator data".into()));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid generator data".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Invalid generator data".into()));
                        }
                    };

                    match &mut sub_iter {
                        SubIterator::Array { handle, index } => {
                            if !is_new {
                                // Pop sent value (ignored for array)
                                {
                                    let gen_val = self.arena.get(gen_handle);
                                    if let Val::Object(payload_handle) = &gen_val.value {
                                        let payload = self.arena.get(*payload_handle);
                                        if let Val::ObjPayload(obj_data) = &payload.value {
                                            if let Some(internal) = &obj_data.internal {
                                                if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                    let mut data = gen_data.borrow_mut();
                                                    data.sent_val.take();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            
                            if let Val::Array(map) = &self.arena.get(*handle).value {
                                if let Some((k, v)) = map.get_index(*index) {
                                    let val_handle = *v;
                                    let key_handle = match k {
                                        ArrayKey::Int(i) => self.arena.alloc(Val::Int(*i)),
                                        ArrayKey::Str(s) => self.arena.alloc(Val::String(s.clone())),
                                    };
                                    
                                    *index += 1;
                                    
                                    let mut frame = self.frames.pop().unwrap();
                                    frame.ip -= 1; // Stay on YieldFrom
                                    
                                    {
                                        let gen_val = self.arena.get(gen_handle);
                                        if let Val::Object(payload_handle) = &gen_val.value {
                                            let payload = self.arena.get(*payload_handle);
                                            if let Val::ObjPayload(obj_data) = &payload.value {
                                                if let Some(internal) = &obj_data.internal {
                                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                        let mut data = gen_data.borrow_mut();
                                                        data.current_val = Some(val_handle);
                                                        data.current_key = Some(key_handle);
                                                        data.state = GeneratorState::Delegating(frame);
                                                        data.sub_iter = Some(sub_iter.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Do NOT push to caller stack
                                    return Ok(());
                                } else {
                                    // Finished
                                    {
                                        let gen_val = self.arena.get(gen_handle);
                                        if let Val::Object(payload_handle) = &gen_val.value {
                                            let payload = self.arena.get(*payload_handle);
                                            if let Val::ObjPayload(obj_data) = &payload.value {
                                                if let Some(internal) = &obj_data.internal {
                                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                        let mut data = gen_data.borrow_mut();
                                                        data.state = GeneratorState::Running;
                                                        data.sub_iter = None;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    let null_handle = self.arena.alloc(Val::Null);
                                    self.operand_stack.push(null_handle);
                                }
                            }
                        }
                        SubIterator::Generator { handle, state } => {
                            match state {
                                SubGenState::Initial | SubGenState::Resuming => {
                                    let gen_b_val = self.arena.get(*handle);
                                    if let Val::Object(payload_handle) = &gen_b_val.value {
                                        let payload = self.arena.get(*payload_handle);
                                        if let Val::ObjPayload(obj_data) = &payload.value {
                                            if let Some(internal) = &obj_data.internal {
                                                if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                    let mut data = gen_data.borrow_mut();
                                                    
                                                    let frame_to_push = match &data.state {
                                                        GeneratorState::Created(f) | GeneratorState::Suspended(f) => {
                                                            let mut f = f.clone();
                                                            f.generator = Some(*handle);
                                                            Some(f)
                                                        },
                                                        _ => None,
                                                    };
                                                    
                                                    if let Some(f) = frame_to_push {
                                                        data.state = GeneratorState::Running;
                                                        
                                                        // Update state to Yielded
                                                        *state = SubGenState::Yielded;
                                                        
                                                        // Decrement IP of current frame so we re-execute YieldFrom when we return
                                                        {
                                                            let frame = self.frames.last_mut().unwrap();
                                                            frame.ip -= 1;
                                                        }
                                                        
                                                        // Update GenA state (set sub_iter, but keep Running)
                                                        {
                                                            let gen_val = self.arena.get(gen_handle);
                                                            if let Val::Object(payload_handle) = &gen_val.value {
                                                                let payload = self.arena.get(*payload_handle);
                                                                if let Val::ObjPayload(obj_data) = &payload.value {
                                                                    if let Some(internal) = &obj_data.internal {
                                                                        if let Ok(parent_gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                                            let mut parent_data = parent_gen_data.borrow_mut();
                                                                            parent_data.sub_iter = Some(sub_iter.clone());
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        
                                                        let gen_handle_opt = f.generator;
                                                        self.frames.push(f);
                                                        
                                                        // If Resuming, we leave the sent value on stack for GenB
                                                        // If Initial, we push null (dummy sent value)
                                                        if is_new {
                                                            let null_handle = self.arena.alloc(Val::Null);
                                                            // Set sent_val in child generator data
                                                            data.sent_val = Some(null_handle);
                                                        }
                                                        return Ok(());
                                                    } else if let GeneratorState::Finished = data.state {
                                                        // Already finished?
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                SubGenState::Yielded => {
                                    let mut gen_b_finished = false;
                                    let mut yielded_val = None;
                                    let mut yielded_key = None;
                                    
                                    {
                                        let gen_b_val = self.arena.get(*handle);
                                        if let Val::Object(payload_handle) = &gen_b_val.value {
                                            let payload = self.arena.get(*payload_handle);
                                            if let Val::ObjPayload(obj_data) = &payload.value {
                                                if let Some(internal) = &obj_data.internal {
                                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                        let data = gen_data.borrow();
                                                        if let GeneratorState::Finished = data.state {
                                                            gen_b_finished = true;
                                                        } else {
                                                            yielded_val = data.current_val;
                                                            yielded_key = data.current_key;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    if gen_b_finished {
                                        // GenB finished, return value is on the stack (pushed by OpCode::Return)
                                        let result_handle = self.operand_stack.pop().unwrap_or_else(|| self.arena.alloc(Val::Null));
                                        
                                        // GenB finished, result_handle is return value
                                        {
                                            let gen_val = self.arena.get(gen_handle);
                                            if let Val::Object(payload_handle) = &gen_val.value {
                                                let payload = self.arena.get(*payload_handle);
                                                if let Val::ObjPayload(obj_data) = &payload.value {
                                                    if let Some(internal) = &obj_data.internal {
                                                        if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                            let mut data = gen_data.borrow_mut();
                                                            data.state = GeneratorState::Running;
                                                            data.sub_iter = None;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        self.operand_stack.push(result_handle);
                                    } else {
                                        // GenB yielded
                                        *state = SubGenState::Resuming;
                                        
                                        let mut frame = self.frames.pop().unwrap();
                                        frame.ip -= 1;
                                        
                                        {
                                            let gen_val = self.arena.get(gen_handle);
                                            if let Val::Object(payload_handle) = &gen_val.value {
                                                let payload = self.arena.get(*payload_handle);
                                                if let Val::ObjPayload(obj_data) = &payload.value {
                                                    if let Some(internal) = &obj_data.internal {
                                                        if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                            let mut data = gen_data.borrow_mut();
                                                            data.current_val = yielded_val;
                                                            data.current_key = yielded_key;
                                                            data.state = GeneratorState::Delegating(frame);
                                                            data.sub_iter = Some(sub_iter.clone());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        
                                        // Do NOT push to caller stack
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }

                OpCode::GetSentValue => {
                    let frame_idx = self.frames.len() - 1;
                    let frame = &mut self.frames[frame_idx];
                    let gen_handle = frame.generator.ok_or(VmError::RuntimeError("GetSentValue outside of generator context".into()))?;
                    
                    let sent_handle = {
                        let gen_val = self.arena.get(gen_handle);
                        if let Val::Object(payload_handle) = &gen_val.value {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        let mut data = gen_data.borrow_mut();
                                        // Get and clear sent_val
                                        data.sent_val.take().unwrap_or_else(|| self.arena.alloc(Val::Null))
                                    } else {
                                        return Err(VmError::RuntimeError("Invalid generator data".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Invalid generator data".into()));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid generator data".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Invalid generator data".into()));
                        }
                    };
                    
                    self.operand_stack.push(sent_handle);
                }

                OpCode::DefFunc(name, func_idx) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[func_idx as usize].clone()
                    };
                    if let Val::Resource(rc) = val {
                        if let Ok(func) = rc.downcast::<UserFunc>() {
                            self.context.user_functions.insert(name, func);
                        }
                    }
                }
                
                OpCode::Include => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle);
                    let filename = match &val.value {
                        Val::String(s) => String::from_utf8_lossy(s).to_string(),
                        _ => return Err(VmError::RuntimeError("Include expects string".into())),
                    };
                    
                    self.context.included_files.insert(filename.clone());
                    
                    let source = std::fs::read(&filename).map_err(|e| VmError::RuntimeError(format!("Could not read file {}: {}", filename, e)))?;
                    
                    let arena = bumpalo::Bump::new();
                    let lexer = php_parser::lexer::Lexer::new(&source);
                    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
                    let program = parser.parse_program();
                    
                    if !program.errors.is_empty() {
                        return Err(VmError::RuntimeError(format!("Parse errors: {:?}", program.errors)));
                    }
                    
                    let emitter = crate::compiler::emitter::Emitter::new(&source, &mut self.context.interner);
                    let (chunk, _) = emitter.compile(program.statements);
                    
                    let mut frame = CallFrame::new(Rc::new(chunk));
                    if let Some(current_frame) = self.frames.last() {
                        frame.locals = current_frame.locals.clone();
                    }
                    self.frames.push(frame);
                }
                
                OpCode::Nop => {},
                OpCode::InitArray(_size) => {
                    let handle = self.arena.alloc(Val::Array(indexmap::IndexMap::new()));
                    self.operand_stack.push(handle);
                }

                OpCode::FetchDim => {
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let key_val = &self.arena.get(key_handle).value;
                    let key = match key_val {
                        Val::Int(i) => ArrayKey::Int(*i),
                        Val::String(s) => ArrayKey::Str(s.clone()),
                        _ => return Err(VmError::RuntimeError("Invalid array key".into())),
                    };
                    
                    let array_val = &self.arena.get(array_handle).value;
                    match array_val {
                        Val::Array(map) => {
                            if let Some(val_handle) = map.get(&key) {
                                self.operand_stack.push(*val_handle);
                            } else {
                                // Warning: Undefined array key
                                let null_handle = self.arena.alloc(Val::Null);
                                self.operand_stack.push(null_handle);
                            }
                        }
                        _ => return Err(VmError::RuntimeError("Trying to access offset on non-array".into())),
                    }
                }

                OpCode::AssignDim => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    self.assign_dim_value(array_handle, key_handle, val_handle)?;
                }

                OpCode::AssignDimRef => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    self.assign_dim(array_handle, key_handle, val_handle)?;
                    
                    // assign_dim pushes the new array handle.
                    let new_array_handle = self.operand_stack.pop().unwrap();
                    
                    // We want to return [Val, NewArray] so that we can StoreVar(NewArray) and leave Val.
                    self.operand_stack.push(val_handle);
                    self.operand_stack.push(new_array_handle);
                }

                OpCode::StoreDim => {
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    self.assign_dim(array_handle, key_handle, val_handle)?;
                }

                OpCode::AppendArray => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    self.append_array(array_handle, val_handle)?;
                }

                OpCode::StoreAppend => {
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    self.append_array(array_handle, val_handle)?;
                }
                OpCode::UnsetDim => {
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let key_val = &self.arena.get(key_handle).value;
                    let key = match key_val {
                        Val::Int(i) => ArrayKey::Int(*i),
                        Val::String(s) => ArrayKey::Str(s.clone()),
                        _ => return Err(VmError::RuntimeError("Invalid array key".into())),
                    };
                    
                    let array_zval_mut = self.arena.get_mut(array_handle);
                    if let Val::Array(map) = &mut array_zval_mut.value {
                        map.remove(&key);
                    }
                }
                OpCode::InArray => {
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let needle_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let array_val = &self.arena.get(array_handle).value;
                    let needle_val = &self.arena.get(needle_handle).value;
                    
                    let found = if let Val::Array(map) = array_val {
                        map.values().any(|h| {
                            let v = &self.arena.get(*h).value;
                            v == needle_val 
                        })
                    } else {
                        false
                    };
                    
                    let res_handle = self.arena.alloc(Val::Bool(found));
                    self.operand_stack.push(res_handle);
                }
                OpCode::ArrayKeyExists => {
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let key_val = &self.arena.get(key_handle).value;
                    let key = match key_val {
                        Val::Int(i) => ArrayKey::Int(*i),
                        Val::String(s) => ArrayKey::Str(s.clone()),
                        _ => return Err(VmError::RuntimeError("Invalid array key".into())),
                    };
                    
                    let array_val = &self.arena.get(array_handle).value;
                    let found = if let Val::Array(map) = array_val {
                        map.contains_key(&key)
                    } else {
                        false
                    };
                    
                    let res_handle = self.arena.alloc(Val::Bool(found));
                    self.operand_stack.push(res_handle);
                }
                OpCode::Count => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = &self.arena.get(handle).value;
                    
                    let count = match val {
                        Val::Array(map) => map.len(),
                        Val::Null => 0,
                        _ => 1,
                    };
                    
                    let res_handle = self.arena.alloc(Val::Int(count as i64));
                    self.operand_stack.push(res_handle);
                }

                OpCode::StoreNestedDim(depth) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let mut keys = Vec::with_capacity(depth as usize);
                    for _ in 0..depth {
                        keys.push(self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?);
                    }
                    keys.reverse();
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    self.assign_nested_dim(array_handle, &keys, val_handle)?;
                }

                OpCode::IterInit(target) => {
                    // Stack: [Array/Object]
                    let iterable_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let iterable_val = &self.arena.get(iterable_handle).value;
                    
                    match iterable_val {
                        Val::Array(map) => {
                            let len = map.len();
                            if len == 0 {
                                self.operand_stack.pop(); // Pop array
                                let frame = self.frames.last_mut().unwrap();
                                frame.ip = target as usize;
                            } else {
                                let idx_handle = self.arena.alloc(Val::Int(0));
                                self.operand_stack.push(idx_handle);
                            }
                        }
                        Val::Object(payload_handle) => {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        let mut data = gen_data.borrow_mut();
                                        match &data.state {
                                            GeneratorState::Created(frame) => {
                                                let mut frame = frame.clone();
                                                frame.generator = Some(iterable_handle);
                                                self.frames.push(frame);
                                                data.state = GeneratorState::Running;
                                                
                                                // Push dummy index to maintain [Iterable, Index] stack shape
                                                let idx_handle = self.arena.alloc(Val::Int(0));
                                                self.operand_stack.push(idx_handle);
                                            }
                                            GeneratorState::Finished => {
                                                self.operand_stack.pop(); // Pop iterable
                                                let frame = self.frames.last_mut().unwrap();
                                                frame.ip = target as usize;
                                            }
                                            _ => return Err(VmError::RuntimeError("Cannot rewind generator".into())),
                                        }
                                    } else {
                                        return Err(VmError::RuntimeError("Object not iterable".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Object not iterable".into()));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        }
                        _ => return Err(VmError::RuntimeError("Foreach expects array or object".into())),
                    }
                }
                
                OpCode::IterValid(target) => {
                    // Stack: [Iterable, Index]
                    // Or [Iterable, DummyIndex, ReturnValue] if generator returned
                    
                    let mut idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let mut iterable_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    // Check for generator return value on stack
                    if let Val::Null = &self.arena.get(iterable_handle).value {
                        if let Some(real_iterable_handle) = self.operand_stack.peek_at(2) {
                            if let Val::Object(_) = &self.arena.get(real_iterable_handle).value {
                                // Found generator return value. Pop it.
                                self.operand_stack.pop();
                                // Re-fetch handles
                                idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                                iterable_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                            }
                        }
                    }
                    
                    let iterable_val = &self.arena.get(iterable_handle).value;
                    match iterable_val {
                        Val::Array(map) => {
                            let idx = match self.arena.get(idx_handle).value {
                                Val::Int(i) => i as usize,
                                _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                            };
                            if idx >= map.len() {
                                self.operand_stack.pop(); // Pop Index
                                self.operand_stack.pop(); // Pop Array
                                let frame = self.frames.last_mut().unwrap();
                                frame.ip = target as usize;
                            }
                        }
                        Val::Object(payload_handle) => {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        let data = gen_data.borrow();
                                        if let GeneratorState::Finished = data.state {
                                            self.operand_stack.pop(); // Pop Index
                                            self.operand_stack.pop(); // Pop Iterable
                                            let frame = self.frames.last_mut().unwrap();
                                            frame.ip = target as usize;
                                        }
                                    }
                                }
                            }
                        }
                        _ => return Err(VmError::RuntimeError("Foreach expects array or object".into())),
                    }
                }
                
                OpCode::IterNext => {
                    // Stack: [Iterable, Index]
                    let idx_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let iterable_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let iterable_val = &self.arena.get(iterable_handle).value;
                    match iterable_val {
                        Val::Array(_) => {
                            let idx = match self.arena.get(idx_handle).value {
                                Val::Int(i) => i,
                                _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                            };
                            let new_idx_handle = self.arena.alloc(Val::Int(idx + 1));
                            self.operand_stack.push(new_idx_handle);
                        }
                        Val::Object(payload_handle) => {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        let mut data = gen_data.borrow_mut();
                                        println!("IterNext: Resuming generator {:?} state: {:?}", iterable_handle, data.state);
                                        if let GeneratorState::Suspended(frame) = &data.state {
                                            let mut frame = frame.clone();
                                            frame.generator = Some(iterable_handle);
                                            self.frames.push(frame);
                                            data.state = GeneratorState::Running;
                                            // Push dummy index
                                            let idx_handle = self.arena.alloc(Val::Null);
                                            self.operand_stack.push(idx_handle);
                                            // Store sent value (null) for generator
                                            let sent_handle = self.arena.alloc(Val::Null);
                                            data.sent_val = Some(sent_handle);
                                        } else if let GeneratorState::Delegating(frame) = &data.state {
                                            let mut frame = frame.clone();
                                            frame.generator = Some(iterable_handle);
                                            self.frames.push(frame);
                                            data.state = GeneratorState::Running;
                                            // Push dummy index
                                            let idx_handle = self.arena.alloc(Val::Null);
                                            self.operand_stack.push(idx_handle);
                                            // Store sent value (null) for generator
                                            let sent_handle = self.arena.alloc(Val::Null);
                                            data.sent_val = Some(sent_handle);
                                        } else if let GeneratorState::Finished = data.state {
                                            let idx_handle = self.arena.alloc(Val::Null);
                                            self.operand_stack.push(idx_handle);
                                        } else {
                                            return Err(VmError::RuntimeError("Cannot resume running generator".into()));
                                        }
                                    } else {
                                        return Err(VmError::RuntimeError("Object not iterable".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Object not iterable".into()));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        }
                        _ => return Err(VmError::RuntimeError("Foreach expects array or object".into())),
                    }
                }
                
                OpCode::IterGetVal(sym) => {
                    // Stack: [Iterable, Index]
                    let idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let iterable_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let iterable_val = &self.arena.get(iterable_handle).value;
                    match iterable_val {
                        Val::Array(map) => {
                            let idx = match self.arena.get(idx_handle).value {
                                Val::Int(i) => i as usize,
                                _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                            };
                            if let Some((_, val_handle)) = map.get_index(idx) {
                                let val_h = *val_handle;
                                let final_handle = if self.arena.get(val_h).is_ref {
                                    let val = self.arena.get(val_h).value.clone();
                                    self.arena.alloc(val)
                                } else {
                                    val_h
                                };
                                let frame = self.frames.last_mut().unwrap();
                                frame.locals.insert(sym, final_handle);
                            } else {
                                return Err(VmError::RuntimeError("Iterator index out of bounds".into()));
                            }
                        }
                        Val::Object(payload_handle) => {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                        let data = gen_data.borrow();
                                        if let Some(val_handle) = data.current_val {
                                            let frame = self.frames.last_mut().unwrap();
                                            frame.locals.insert(sym, val_handle);
                                        } else {
                                            return Err(VmError::RuntimeError("Generator has no current value".into()));
                                        }
                                    } else {
                                        return Err(VmError::RuntimeError("Object not iterable".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Object not iterable".into()));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        }
                        _ => return Err(VmError::RuntimeError("Foreach expects array or object".into())),
                    }
                }

                OpCode::IterGetValRef(sym) => {
                    // Stack: [Array, Index]
                    let idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let idx = match self.arena.get(idx_handle).value {
                        Val::Int(i) => i as usize,
                        _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                    };
                    
                    // Check if we need to upgrade the element.
                    let (needs_upgrade, val_handle) = {
                        let array_zval = self.arena.get(array_handle);
                        if let Val::Array(map) = &array_zval.value {
                            if let Some((_, h)) = map.get_index(idx) {
                                let is_ref = self.arena.get(*h).is_ref;
                                (!is_ref, *h)
                            } else {
                                return Err(VmError::RuntimeError("Iterator index out of bounds".into()));
                            }
                        } else {
                             return Err(VmError::RuntimeError("IterGetValRef expects array".into()));
                        }
                    };
                    
                    let final_handle = if needs_upgrade {
                        // Upgrade: Clone value, make ref, update array.
                        let val = self.arena.get(val_handle).value.clone();
                        let new_handle = self.arena.alloc(val);
                        self.arena.get_mut(new_handle).is_ref = true;
                        
                        // Update array
                        let array_zval_mut = self.arena.get_mut(array_handle);
                        if let Val::Array(map) = &mut array_zval_mut.value {
                             if let Some((_, h_ref)) = map.get_index_mut(idx) {
                                 *h_ref = new_handle;
                             }
                        }
                        new_handle
                    } else {
                        val_handle
                    };
                    
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.insert(sym, final_handle);
                }
                
                OpCode::IterGetKey(sym) => {
                    // Stack: [Array, Index]
                    let idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let idx = match self.arena.get(idx_handle).value {
                        Val::Int(i) => i as usize,
                        _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                    };
                    
                    let array_val = &self.arena.get(array_handle).value;
                    if let Val::Array(map) = array_val {
                        if let Some((key, _)) = map.get_index(idx) {
                            let key_val = match key {
                                ArrayKey::Int(i) => Val::Int(*i),
                                ArrayKey::Str(s) => Val::String(s.clone()),
                            };
                            let key_handle = self.arena.alloc(key_val);
                            
                            // Store in local
                            let frame = self.frames.last_mut().unwrap();
                            frame.locals.insert(sym, key_handle);
                        } else {
                            return Err(VmError::RuntimeError("Iterator index out of bounds".into()));
                        }
                    }
                }
                OpCode::FeResetR(target) => {
                    let array_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_val = &self.arena.get(array_handle).value;
                    let len = match array_val {
                        Val::Array(map) => map.len(),
                        _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                    };
                    if len == 0 {
                        self.operand_stack.pop();
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    } else {
                        let idx_handle = self.arena.alloc(Val::Int(0));
                        self.operand_stack.push(idx_handle);
                    }
                }
                OpCode::FeFetchR(target) => {
                    let idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let idx = match self.arena.get(idx_handle).value {
                        Val::Int(i) => i as usize,
                        _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                    };
                    
                    let array_val = &self.arena.get(array_handle).value;
                    let len = match array_val {
                        Val::Array(map) => map.len(),
                        _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                    };
                    
                    if idx >= len {
                        self.operand_stack.pop();
                        self.operand_stack.pop();
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    } else {
                        if let Val::Array(map) = array_val {
                            if let Some((_, val_handle)) = map.get_index(idx) {
                                self.operand_stack.push(*val_handle);
                            }
                        }
                        self.arena.get_mut(idx_handle).value = Val::Int((idx + 1) as i64);
                    }
                }
                OpCode::FeResetRw(_) => {}
                OpCode::FeFetchRw(_) => {}
                OpCode::FeFree => {
                    self.operand_stack.pop();
                    self.operand_stack.pop();
                }

                OpCode::DefClass(name, parent) => {
                    let class_def = ClassDef {
                        name,
                        parent,
                        is_interface: false,
                        is_trait: false,
                        interfaces: Vec::new(),
                        traits: Vec::new(),
                        methods: HashMap::new(),
                        properties: IndexMap::new(),
                        constants: HashMap::new(),
                        static_properties: HashMap::new(),
                    };
                    self.context.classes.insert(name, class_def);
                }
                OpCode::DefInterface(name) => {
                    let class_def = ClassDef {
                        name,
                        parent: None,
                        is_interface: true,
                        is_trait: false,
                        interfaces: Vec::new(),
                        traits: Vec::new(),
                        methods: HashMap::new(),
                        properties: IndexMap::new(),
                        constants: HashMap::new(),
                        static_properties: HashMap::new(),
                    };
                    self.context.classes.insert(name, class_def);
                }
                OpCode::DefTrait(name) => {
                    let class_def = ClassDef {
                        name,
                        parent: None,
                        is_interface: false,
                        is_trait: true,
                        interfaces: Vec::new(),
                        traits: Vec::new(),
                        methods: HashMap::new(),
                        properties: IndexMap::new(),
                        constants: HashMap::new(),
                        static_properties: HashMap::new(),
                    };
                    self.context.classes.insert(name, class_def);
                }
                OpCode::AddInterface(class_name, interface_name) => {
                    if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                        class_def.interfaces.push(interface_name);
                    }
                }
                OpCode::UseTrait(class_name, trait_name) => {
                    let trait_methods = if let Some(trait_def) = self.context.classes.get(&trait_name) {
                        if !trait_def.is_trait {
                            return Err(VmError::RuntimeError("Not a trait".into()));
                        }
                        trait_def.methods.clone()
                    } else {
                        return Err(VmError::RuntimeError("Trait not found".into()));
                    };
                    
                    if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                        class_def.traits.push(trait_name);
                        for (name, (func, vis, is_static)) in trait_methods {
                            class_def.methods.entry(name).or_insert((func, vis, is_static));
                        }
                    }
                }
                OpCode::DefMethod(class_name, method_name, func_idx, visibility, is_static) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[func_idx as usize].clone()
                    };
                    if let Val::Resource(rc) = val {
                        if let Ok(func) = rc.downcast::<UserFunc>() {
                            if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                                class_def.methods.insert(method_name, (func, visibility, is_static));
                            }
                        }
                    }
                }
                OpCode::DefProp(class_name, prop_name, default_idx, visibility) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[default_idx as usize].clone()
                    };
                    if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                        class_def.properties.insert(prop_name, (val, visibility));
                    }
                }
                OpCode::DefClassConst(class_name, const_name, val_idx, visibility) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[val_idx as usize].clone()
                    };
                    if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                        class_def.constants.insert(const_name, (val, visibility));
                    }
                }
                OpCode::DefGlobalConst(name, val_idx) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[val_idx as usize].clone()
                    };
                    self.context.constants.insert(name, val);
                }
                OpCode::FetchGlobalConst(name) => {
                    if let Some(val) = self.context.constants.get(&name) {
                        let handle = self.arena.alloc(val.clone());
                        self.operand_stack.push(handle);
                    } else if let Some(val) = self.context.engine.constants.get(&name) {
                        let handle = self.arena.alloc(val.clone());
                        self.operand_stack.push(handle);
                    } else {
                        // If not found, PHP treats it as a string "NAME" and issues a warning.
                        let name_bytes = self.context.interner.lookup(name).unwrap_or(b"???");
                        let val = Val::String(name_bytes.to_vec());
                        let handle = self.arena.alloc(val);
                        self.operand_stack.push(handle);
                        // TODO: Issue warning
                    }
                }
                OpCode::DefStaticProp(class_name, prop_name, default_idx, visibility) => {
                    let val = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[default_idx as usize].clone()
                    };
                    if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                        class_def.static_properties.insert(prop_name, (val, visibility));
                    }
                }
                OpCode::FetchClassConst(class_name, const_name) => {
                    let resolved_class = self.resolve_class_name(class_name)?;
                    let (val, visibility, defining_class) = self.find_class_constant(resolved_class, const_name)?;
                    self.check_const_visibility(defining_class, visibility)?;
                    let handle = self.arena.alloc(val);
                    self.operand_stack.push(handle);
                }
                OpCode::FetchStaticProp(class_name, prop_name) => {
                    let resolved_class = self.resolve_class_name(class_name)?;
                    let (val, visibility, defining_class) = self.find_static_prop(resolved_class, prop_name)?;
                    self.check_const_visibility(defining_class, visibility)?;
                    let handle = self.arena.alloc(val);
                    self.operand_stack.push(handle);
                }
                OpCode::AssignStaticProp(class_name, prop_name) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(val_handle).value.clone();
                    
                    let resolved_class = self.resolve_class_name(class_name)?;
                    let (_, visibility, defining_class) = self.find_static_prop(resolved_class, prop_name)?;
                    self.check_const_visibility(defining_class, visibility)?;
                    
                    if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                        if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                            entry.0 = val.clone();
                        }
                    }
                    
                    let res_handle = self.arena.alloc(val);
                    self.operand_stack.push(res_handle);
                }
                OpCode::New(class_name, arg_count) => {
                    if self.context.classes.contains_key(&class_name) {
                        let properties = self.collect_properties(class_name);
                        
                        let obj_data = ObjectData {
                            class: class_name,
                            properties,
                            internal: None,
                        };
                        
                        let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                        let obj_val = Val::Object(payload_handle);
                        let obj_handle = self.arena.alloc(obj_val);
                        
                        // Check for constructor
                        let constructor_name = self.context.interner.intern(b"__construct");
                        if let Some((constructor, _, _, defined_class)) = self.find_method(class_name, constructor_name) {
                             // Collect args
                            let mut args = Vec::new();
                            for _ in 0..arg_count {
                                args.push(self.operand_stack.pop().unwrap());
                            }
                            args.reverse();

                            let mut frame = CallFrame::new(constructor.chunk.clone());
                            frame.func = Some(constructor.clone());
                            frame.this = Some(obj_handle);
                            frame.is_constructor = true;
                            frame.class_scope = Some(defined_class);

                            for (i, param) in constructor.params.iter().enumerate() {
                                if i < args.len() {
                                    let arg_handle = args[i];
                                    if param.by_ref {
                                        if !self.arena.get(arg_handle).is_ref {
                                            self.arena.get_mut(arg_handle).is_ref = true;
                                        }
                                        frame.locals.insert(param.name, arg_handle);
                                    } else {
                                        let final_handle = if self.arena.get(arg_handle).is_ref {
                                            let val = self.arena.get(arg_handle).value.clone();
                                            self.arena.alloc(val)
                                        } else {
                                            arg_handle
                                        };
                                        frame.locals.insert(param.name, final_handle);
                                    }
                                }
                            }
                            self.frames.push(frame);
                        } else {
                            if arg_count > 0 {
                                let class_name_bytes = self.context.interner.lookup(class_name).unwrap_or(b"<unknown>");
                                let class_name_str = String::from_utf8_lossy(class_name_bytes);
                                return Err(VmError::RuntimeError(format!("Class {} does not have a constructor, so you cannot pass any constructor arguments", class_name_str).into()));
                            }
                            self.operand_stack.push(obj_handle);
                        }
                    } else {
                        return Err(VmError::RuntimeError("Class not found".into()));
                    }
                }
                OpCode::NewDynamic(arg_count) => {
                    // Collect args first
                    let mut args = Vec::new();
                    for _ in 0..arg_count {
                        args.push(self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?);
                    }
                    args.reverse();
                    
                    let class_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let class_name = match &self.arena.get(class_handle).value {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                    };
                    
                    if self.context.classes.contains_key(&class_name) {
                        let properties = self.collect_properties(class_name);
                        
                        let obj_data = ObjectData {
                            class: class_name,
                            properties,
                            internal: None,
                        };
                        
                        let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                        let obj_val = Val::Object(payload_handle);
                        let obj_handle = self.arena.alloc(obj_val);
                        
                        // Check for constructor
                        let constructor_name = self.context.interner.intern(b"__construct");
                        if let Some((constructor, _, _, defined_class)) = self.find_method(class_name, constructor_name) {
                            let mut frame = CallFrame::new(constructor.chunk.clone());
                            frame.func = Some(constructor.clone());
                            frame.this = Some(obj_handle);
                            frame.is_constructor = true;
                            frame.class_scope = Some(defined_class);

                            for (i, param) in constructor.params.iter().enumerate() {
                                if i < args.len() {
                                    let arg_handle = args[i];
                                    if param.by_ref {
                                        if !self.arena.get(arg_handle).is_ref {
                                            self.arena.get_mut(arg_handle).is_ref = true;
                                        }
                                        frame.locals.insert(param.name, arg_handle);
                                    } else {
                                        let final_handle = if self.arena.get(arg_handle).is_ref {
                                            let val = self.arena.get(arg_handle).value.clone();
                                            self.arena.alloc(val)
                                        } else {
                                            arg_handle
                                        };
                                        frame.locals.insert(param.name, final_handle);
                                    }
                                }
                            }
                            self.frames.push(frame);
                        } else {
                            if arg_count > 0 {
                                let class_name_bytes = self.context.interner.lookup(class_name).unwrap_or(b"<unknown>");
                                let class_name_str = String::from_utf8_lossy(class_name_bytes);
                                return Err(VmError::RuntimeError(format!("Class {} does not have a constructor, so you cannot pass any constructor arguments", class_name_str).into()));
                            }
                            self.operand_stack.push(obj_handle);
                        }
                    } else {
                        return Err(VmError::RuntimeError("Class not found".into()));
                    }
                }
                OpCode::FetchProp(prop_name) => {
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    // Extract needed data to avoid holding borrow
                    let (class_name, prop_handle_opt) = {
                        let obj_zval = self.arena.get(obj_handle);
                        if let Val::Object(payload_handle) = obj_zval.value {
                            let payload_zval = self.arena.get(payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload_zval.value {
                                (obj_data.class, obj_data.properties.get(&prop_name).copied())
                            } else {
                                return Err(VmError::RuntimeError("Invalid object payload".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Attempt to fetch property on non-object".into()));
                        }
                    };

                    // Check visibility
                    let current_scope = self.get_current_class();
                    let visibility_check = self.check_prop_visibility(class_name, prop_name, current_scope);

                    let mut use_magic = false;
                    
                    if let Some(prop_handle) = prop_handle_opt {
                        if visibility_check.is_ok() {
                            self.operand_stack.push(prop_handle);
                        } else {
                            use_magic = true;
                        }
                    } else {
                        use_magic = true;
                    }
                    
                    if use_magic {
                        let magic_get = self.context.interner.intern(b"__get");
                        if let Some((method, _, _, defined_class)) = self.find_method(class_name, magic_get) {
                            let prop_name_bytes = self.context.interner.lookup(prop_name).unwrap_or(b"").to_vec();
                            let name_handle = self.arena.alloc(Val::String(prop_name_bytes));
                            
                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(defined_class);
                            frame.called_scope = Some(class_name);
                            
                            if let Some(param) = method.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }
                            
                            self.frames.push(frame);
                        } else {
                            if let Err(e) = visibility_check {
                                return Err(e);
                            }
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    }
                }
                OpCode::AssignProp(prop_name) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                        h
                    } else {
                        return Err(VmError::RuntimeError("Attempt to assign property on non-object".into()));
                    };
                    
                    // Extract data
                    let (class_name, prop_exists) = {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            (obj_data.class, obj_data.properties.contains_key(&prop_name))
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    };
                    
                    let current_scope = self.get_current_class();
                    let visibility_check = self.check_prop_visibility(class_name, prop_name, current_scope);

                    let mut use_magic = false;
                    
                    if prop_exists {
                        if visibility_check.is_err() {
                            use_magic = true;
                        }
                    } else {
                        use_magic = true;
                    }
                    
                    if use_magic {
                        let magic_set = self.context.interner.intern(b"__set");
                        if let Some((method, _, _, defined_class)) = self.find_method(class_name, magic_set) {
                            let prop_name_bytes = self.context.interner.lookup(prop_name).unwrap_or(b"").to_vec();
                            let name_handle = self.arena.alloc(Val::String(prop_name_bytes));
                            
                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(defined_class);
                            frame.called_scope = Some(class_name);
                            frame.discard_return = true;
                            
                            if let Some(param) = method.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }
                            if let Some(param) = method.params.get(1) {
                                frame.locals.insert(param.name, val_handle);
                            }
                            
                            self.frames.push(frame);
                            self.operand_stack.push(val_handle);
                        } else {
                            if let Err(e) = visibility_check {
                                return Err(e);
                            }
                            
                            let payload_zval = self.arena.get_mut(payload_handle);
                            if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                                obj_data.properties.insert(prop_name, val_handle);
                            }
                            self.operand_stack.push(val_handle);
                        }
                    } else {
                         let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, val_handle);
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                        self.operand_stack.push(val_handle);
                    }
                }
                OpCode::CallMethod(method_name, arg_count) => {
                    let obj_handle = self.operand_stack.peek_at(arg_count as usize).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let class_name = if let Val::Object(h) = self.arena.get(obj_handle).value {
                        if let Val::ObjPayload(data) = &self.arena.get(h).value {
                            data.class
                        } else {
                             return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError("Call to member function on non-object".into()));
                    };
                    
                    let method_lookup = self.find_method(class_name, method_name);

                    if let Some((user_func, visibility, is_static, defined_class)) = method_lookup {
                        // Check visibility
                        match visibility {
                            Visibility::Public => {},
                            Visibility::Private => {
                                let current_class = self.get_current_class();
                                if current_class != Some(defined_class) {
                                    return Err(VmError::RuntimeError("Cannot access private method".into()));
                                }
                            },
                            Visibility::Protected => {
                                let current_class = self.get_current_class();
                                if let Some(scope) = current_class {
                                    if !self.is_subclass_of(scope, defined_class) && !self.is_subclass_of(defined_class, scope) {
                                        return Err(VmError::RuntimeError("Cannot access protected method".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Cannot access protected method".into()));
                                }
                            }
                        }

                        let mut args = Vec::new();
                        for _ in 0..arg_count {
                            args.push(self.operand_stack.pop().unwrap());
                        }
                        args.reverse();
                        
                        let obj_handle = self.operand_stack.pop().unwrap();
                        
                        let mut frame = CallFrame::new(user_func.chunk.clone());
                        frame.func = Some(user_func.clone());
                        if !is_static {
                            frame.this = Some(obj_handle);
                        }
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(class_name);
                        
                        for (i, param) in user_func.params.iter().enumerate() {
                            if i < args.len() {
                                let arg_handle = args[i];
                                if param.by_ref {
                                    if !self.arena.get(arg_handle).is_ref {
                                        self.arena.get_mut(arg_handle).is_ref = true;
                                    }
                                    frame.locals.insert(param.name, arg_handle);
                                } else {
                                    let final_handle = if self.arena.get(arg_handle).is_ref {
                                        let val = self.arena.get(arg_handle).value.clone();
                                        self.arena.alloc(val)
                                    } else {
                                        arg_handle
                                    };
                                    frame.locals.insert(param.name, final_handle);
                                }
                            }
                        }
                        
                        self.frames.push(frame);
                    } else {
                        // Method not found. Check for __call.
                        let call_magic = self.context.interner.intern(b"__call");
                        if let Some((magic_func, _, _, magic_class)) = self.find_method(class_name, call_magic) {
                            // Found __call.
                            
                            // Pop args
                            let mut args = Vec::new();
                            for _ in 0..arg_count {
                                args.push(self.operand_stack.pop().unwrap());
                            }
                            args.reverse();
                            
                            let obj_handle = self.operand_stack.pop().unwrap();
                            
                            // Create array from args
                            let mut array_map = IndexMap::new();
                            for (i, arg) in args.into_iter().enumerate() {
                                array_map.insert(ArrayKey::Int(i as i64), arg);
                            }
                            let args_array_handle = self.arena.alloc(Val::Array(array_map));
                            
                            // Create method name string
                            let method_name_str = self.context.interner.lookup(method_name).expect("Method name should be interned").to_vec();
                            let name_handle = self.arena.alloc(Val::String(method_name_str));
                            
                            // Prepare frame for __call
                            let mut frame = CallFrame::new(magic_func.chunk.clone());
                            frame.func = Some(magic_func.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(magic_class);
                            frame.called_scope = Some(class_name);
                            
                            // Pass args: $name, $arguments
                            // Param 0: name
                            if let Some(param) = magic_func.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }
                            // Param 1: arguments
                            if let Some(param) = magic_func.params.get(1) {
                                frame.locals.insert(param.name, args_array_handle);
                            }
                            
                            self.frames.push(frame);
                        } else {
                            let method_str = String::from_utf8_lossy(self.context.interner.lookup(method_name).unwrap_or(b"<unknown>"));
                            return Err(VmError::RuntimeError(format!("Call to undefined method {}", method_str)));
                        }
                    }
                }
                OpCode::UnsetObj => {
                    let prop_name_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let prop_name = match &self.arena.get(prop_name_handle).value {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                    };
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    // Extract data to avoid borrow issues
                    let (class_name, should_unset) = {
                        let obj_zval = self.arena.get(obj_handle);
                        if let Val::Object(payload_handle) = obj_zval.value {
                            let payload_zval = self.arena.get(payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload_zval.value {
                                let current_scope = self.get_current_class();
                                if self.check_prop_visibility(obj_data.class, prop_name, current_scope).is_ok() {
                                    if obj_data.properties.contains_key(&prop_name) {
                                        (obj_data.class, true)
                                    } else {
                                        (obj_data.class, false) // Not found
                                    }
                                } else {
                                    (obj_data.class, false) // Not accessible
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid object payload".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Attempt to unset property on non-object".into()));
                        }
                    };

                    if should_unset {
                        let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                            h
                        } else {
                            unreachable!()
                        };
                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.swap_remove(&prop_name);
                        }
                    } else {
                        // Property not found or not accessible. Check for __unset.
                        let unset_magic = self.context.interner.intern(b"__unset");
                        if let Some((magic_func, _, _, magic_class)) = self.find_method(class_name, unset_magic) {
                            // Found __unset
                            
                            // Create method name string (prop name)
                            let prop_name_str = self.context.interner.lookup(prop_name).expect("Prop name should be interned").to_vec();
                            let name_handle = self.arena.alloc(Val::String(prop_name_str));
                            
                            // Prepare frame for __unset
                            let mut frame = CallFrame::new(magic_func.chunk.clone());
                            frame.func = Some(magic_func.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(magic_class);
                            frame.called_scope = Some(class_name);
                            frame.discard_return = true; // Discard return value
                            
                            // Param 0: name
                            if let Some(param) = magic_func.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }
                            
                            self.frames.push(frame);
                        }
                        // If no __unset, do nothing (standard PHP behavior)
                    }
                }
                OpCode::UnsetStaticProp => {
                    let prop_name_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let prop_name = match &self.arena.get(prop_name_handle).value {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                    };
                    let class_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let class_name = match &self.arena.get(class_handle).value {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                    };
                    
                    let mut current_class = class_name;
                    let mut found = false;
                    
                    // We need to find where it is defined to unset it?
                    // Or does unset static prop only work if it's accessible?
                    // In PHP, `unset(Foo::$prop)` unsets it.
                    // But static properties are shared. Unsetting it might mean setting it to NULL or removing it?
                    // Actually, you cannot unset static properties in PHP.
                    // `unset(Foo::$prop)` results in "Attempt to unset static property".
                    // Wait, let me check PHP behavior.
                    // `class A { public static $a = 1; } unset(A::$a);` -> Error: Attempt to unset static property
                    // So this opcode might be for internal use or I should throw error?
                    // But `ZEND_UNSET_STATIC_PROP` exists.
                    // Maybe it is used for `unset($a::$b)`?
                    // If PHP throws error, I should throw error.
                    
                    return Err(VmError::RuntimeError("Attempt to unset static property".into()));
                }
                OpCode::InstanceOf => {
                    let class_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let class_name = match &self.arena.get(class_handle).value {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                    };
                    
                    let is_instance = if let Val::Object(h) = self.arena.get(obj_handle).value {
                        if let Val::ObjPayload(data) = &self.arena.get(h).value {
                            self.is_subclass_of(data.class, class_name)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    let res_handle = self.arena.alloc(Val::Bool(is_instance));
                    self.operand_stack.push(res_handle);
                }
                OpCode::GetClass => {
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let class_name = if let Val::Object(h) = self.arena.get(obj_handle).value {
                        if let Val::ObjPayload(data) = &self.arena.get(h).value {
                            Some(data.class)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    
                    if let Some(sym) = class_name {
                        let name_bytes = self.context.interner.lookup(sym).unwrap_or(b"");
                        let res_handle = self.arena.alloc(Val::String(name_bytes.to_vec()));
                        self.operand_stack.push(res_handle);
                    } else {
                        return Err(VmError::RuntimeError("get_class() called on non-object".into()));
                    }
                }
                OpCode::GetCalledClass => {
                    let frame = self.frames.last().ok_or(VmError::RuntimeError("No active frame".into()))?;
                    if let Some(scope) = frame.called_scope {
                        let name_bytes = self.context.interner.lookup(scope).unwrap_or(b"");
                        let res_handle = self.arena.alloc(Val::String(name_bytes.to_vec()));
                        self.operand_stack.push(res_handle);
                    } else {
                        return Err(VmError::RuntimeError("get_called_class() called from outside a class".into()));
                    }
                }
                OpCode::GetType => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = &self.arena.get(handle).value;
                    let type_str = match val {
                        Val::Null => "NULL",
                        Val::Bool(_) => "boolean",
                        Val::Int(_) => "integer",
                        Val::Float(_) => "double",
                        Val::String(_) => "string",
                        Val::Array(_) => "array",
                        Val::Object(_) => "object",
                        Val::Resource(_) => "resource",
                        _ => "unknown",
                    };
                    let res_handle = self.arena.alloc(Val::String(type_str.as_bytes().to_vec()));
                    self.operand_stack.push(res_handle);
                }
                OpCode::Clone => {
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let mut new_obj_data_opt = None;
                    let mut class_name_opt = None;
                    
                    {
                        let obj_val = self.arena.get(obj_handle);
                        if let Val::Object(payload_handle) = &obj_val.value {
                            let payload_val = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload_val.value {
                                new_obj_data_opt = Some(obj_data.clone());
                                class_name_opt = Some(obj_data.class);
                            }
                        }
                    }
                    
                    if let Some(new_obj_data) = new_obj_data_opt {
                        let new_payload_handle = self.arena.alloc(Val::ObjPayload(new_obj_data));
                        let new_obj_handle = self.arena.alloc(Val::Object(new_payload_handle));
                        self.operand_stack.push(new_obj_handle);
                        
                        if let Some(class_name) = class_name_opt {
                            let clone_sym = self.context.interner.intern(b"__clone");
                            if let Some((method, _, _, _)) = self.find_method(class_name, clone_sym) {
                                let mut frame = CallFrame::new(method.chunk.clone());
                                frame.func = Some(method.clone());
                                frame.this = Some(new_obj_handle);
                                frame.class_scope = Some(class_name);
                                frame.discard_return = true;
                                
                                self.frames.push(frame);
                            }
                        }
                    } else {
                        return Err(VmError::RuntimeError("__clone method called on non-object".into()));
                    }
                }
                OpCode::Copy => {
                    let handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let val = self.arena.get(handle).value.clone();
                    let new_handle = self.arena.alloc(val);
                    self.operand_stack.push(new_handle);
                }
                OpCode::IssetVar(sym) => {
                    let frame = self.frames.last().unwrap();
                    let is_set = if let Some(&handle) = frame.locals.get(&sym) {
                        !matches!(self.arena.get(handle).value, Val::Null)
                    } else {
                        false
                    };
                    let res_handle = self.arena.alloc(Val::Bool(is_set));
                    self.operand_stack.push(res_handle);
                }
                OpCode::IssetDim => {
                    let key_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let key_val = &self.arena.get(key_handle).value;
                    let key = match key_val {
                        Val::Int(i) => ArrayKey::Int(*i),
                        Val::String(s) => ArrayKey::Str(s.clone()),
                        _ => ArrayKey::Int(0), // Should probably be error or false
                    };
                    
                    let array_zval = self.arena.get(array_handle);
                    let is_set = if let Val::Array(map) = &array_zval.value {
                        if let Some(val_handle) = map.get(&key) {
                            !matches!(self.arena.get(*val_handle).value, Val::Null)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    let res_handle = self.arena.alloc(Val::Bool(is_set));
                    self.operand_stack.push(res_handle);
                }
                OpCode::IssetProp(prop_name) => {
                    let obj_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    // Extract data to avoid borrow issues
                    let (class_name, is_set_result) = {
                        let obj_zval = self.arena.get(obj_handle);
                        if let Val::Object(payload_handle) = obj_zval.value {
                            let payload_zval = self.arena.get(payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload_zval.value {
                                let current_scope = self.get_current_class();
                                if self.check_prop_visibility(obj_data.class, prop_name, current_scope).is_ok() {
                                    if let Some(val_handle) = obj_data.properties.get(&prop_name) {
                                        (obj_data.class, Some(!matches!(self.arena.get(*val_handle).value, Val::Null)))
                                    } else {
                                        (obj_data.class, None) // Not found
                                    }
                                } else {
                                    (obj_data.class, None) // Not accessible
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid object payload".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Isset on non-object".into()));
                        }
                    };

                    if let Some(result) = is_set_result {
                        let res_handle = self.arena.alloc(Val::Bool(result));
                        self.operand_stack.push(res_handle);
                    } else {
                        // Property not found or not accessible. Check for __isset.
                        let isset_magic = self.context.interner.intern(b"__isset");
                        if let Some((magic_func, _, _, magic_class)) = self.find_method(class_name, isset_magic) {
                            // Found __isset
                            
                            // Create method name string (prop name)
                            let prop_name_str = self.context.interner.lookup(prop_name).expect("Prop name should be interned").to_vec();
                            let name_handle = self.arena.alloc(Val::String(prop_name_str));
                            
                            // Prepare frame for __isset
                            let mut frame = CallFrame::new(magic_func.chunk.clone());
                            frame.func = Some(magic_func.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(magic_class);
                            frame.called_scope = Some(class_name);
                            
                            // Param 0: name
                            if let Some(param) = magic_func.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }
                            
                            self.frames.push(frame);
                        } else {
                            // No __isset, return false
                            let res_handle = self.arena.alloc(Val::Bool(false));
                            self.operand_stack.push(res_handle);
                        }
                    }
                }
                OpCode::IssetStaticProp(prop_name) => {
                    let class_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let class_name = match &self.arena.get(class_handle).value {
                        Val::String(s) => self.context.interner.intern(s),
                        _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                    };
                    
                    let resolved_class = self.resolve_class_name(class_name)?;
                    
                    let is_set = match self.find_static_prop(resolved_class, prop_name) {
                        Ok((val, _, _)) => !matches!(val, Val::Null),
                        Err(_) => false,
                    };
                    
                    let res_handle = self.arena.alloc(Val::Bool(is_set));
                    self.operand_stack.push(res_handle);
                }
                OpCode::CallStaticMethod(class_name, method_name, arg_count) => {
                    let resolved_class = self.resolve_class_name(class_name)?;
                    
                    let method_lookup = self.find_method(resolved_class, method_name);

                    if let Some((user_func, visibility, is_static, defined_class)) = method_lookup {
                        if !is_static {
                             return Err(VmError::RuntimeError("Non-static method called statically".into()));
                        }
                        
                        self.check_const_visibility(defined_class, visibility)?;
                        
                        let mut args = Vec::new();
                        for _ in 0..arg_count {
                            args.push(self.operand_stack.pop().unwrap());
                        }
                        args.reverse();
                        
                        let mut frame = CallFrame::new(user_func.chunk.clone());
                        frame.func = Some(user_func.clone());
                        frame.this = None;
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(resolved_class);
                        
                        for (i, param) in user_func.params.iter().enumerate() {
                            if i < args.len() {
                                let arg_handle = args[i];
                                if param.by_ref {
                                    if !self.arena.get(arg_handle).is_ref {
                                        self.arena.get_mut(arg_handle).is_ref = true;
                                    }
                                    frame.locals.insert(param.name, arg_handle);
                                } else {
                                    let final_handle = if self.arena.get(arg_handle).is_ref {
                                        let val = self.arena.get(arg_handle).value.clone();
                                        self.arena.alloc(val)
                                    } else {
                                        arg_handle
                                    };
                                    frame.locals.insert(param.name, final_handle);
                                }
                            }
                        }
                        
                        self.frames.push(frame);
                    } else {
                        // Method not found. Check for __callStatic.
                        let call_static_magic = self.context.interner.intern(b"__callStatic");
                        if let Some((magic_func, _, is_static, magic_class)) = self.find_method(resolved_class, call_static_magic) {
                            if !is_static {
                                return Err(VmError::RuntimeError("__callStatic must be static".into()));
                            }
                            
                            // Pop args
                            let mut args = Vec::new();
                            for _ in 0..arg_count {
                                args.push(self.operand_stack.pop().unwrap());
                            }
                            args.reverse();
                            
                            // Create array from args
                            let mut array_map = IndexMap::new();
                            for (i, arg) in args.into_iter().enumerate() {
                                array_map.insert(ArrayKey::Int(i as i64), arg);
                            }
                            let args_array_handle = self.arena.alloc(Val::Array(array_map));
                            
                            // Create method name string
                            let method_name_str = self.context.interner.lookup(method_name).expect("Method name should be interned").to_vec();
                            let name_handle = self.arena.alloc(Val::String(method_name_str));
                            
                            // Prepare frame for __callStatic
                            let mut frame = CallFrame::new(magic_func.chunk.clone());
                            frame.func = Some(magic_func.clone());
                            frame.this = None;
                            frame.class_scope = Some(magic_class);
                            frame.called_scope = Some(resolved_class);
                            
                            // Pass args: $name, $arguments
                            // Param 0: name
                            if let Some(param) = magic_func.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }
                            // Param 1: arguments
                            if let Some(param) = magic_func.params.get(1) {
                                frame.locals.insert(param.name, args_array_handle);
                            }
                            
                            self.frames.push(frame);
                        } else {
                            let method_str = String::from_utf8_lossy(self.context.interner.lookup(method_name).unwrap_or(b"<unknown>"));
                            return Err(VmError::RuntimeError(format!("Call to undefined static method {}", method_str)));
                        }
                    }
                }
                
                OpCode::Concat => {
                    let b_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let a_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let b_val = &self.arena.get(b_handle).value;
                    let a_val = &self.arena.get(a_handle).value;
                    
                    let b_str = match b_val {
                        Val::String(s) => s.clone(),
                        Val::Int(i) => i.to_string().into_bytes(),
                        Val::Bool(b) => if *b { b"1".to_vec() } else { vec![] },
                        Val::Null => vec![],
                        _ => format!("{:?}", b_val).into_bytes(),
                    };
                    
                    let a_str = match a_val {
                        Val::String(s) => s.clone(),
                        Val::Int(i) => i.to_string().into_bytes(),
                        Val::Bool(b) => if *b { b"1".to_vec() } else { vec![] },
                        Val::Null => vec![],
                        _ => format!("{:?}", a_val).into_bytes(),
                    };
                    
                    let mut res = a_str;
                    res.extend(b_str);
                    
                    let res_handle = self.arena.alloc(Val::String(res));
                    self.operand_stack.push(res_handle);
                }
                
                OpCode::FastConcat => {
                    let b_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let a_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let b_val = &self.arena.get(b_handle).value;
                    let a_val = &self.arena.get(a_handle).value;
                    
                    let b_str = match b_val {
                        Val::String(s) => s.clone(),
                        Val::Int(i) => i.to_string().into_bytes(),
                        Val::Bool(b) => if *b { b"1".to_vec() } else { vec![] },
                        Val::Null => vec![],
                        _ => format!("{:?}", b_val).into_bytes(),
                    };
                    
                    let a_str = match a_val {
                        Val::String(s) => s.clone(),
                        Val::Int(i) => i.to_string().into_bytes(),
                        Val::Bool(b) => if *b { b"1".to_vec() } else { vec![] },
                        Val::Null => vec![],
                        _ => format!("{:?}", a_val).into_bytes(),
                    };
                    
                    let mut res = a_str;
                    res.extend(b_str);
                    
                    let res_handle = self.arena.alloc(Val::String(res));
                    self.operand_stack.push(res_handle);
                }
                
                OpCode::IsEqual => self.binary_cmp(|a, b| a == b)?,
                OpCode::IsNotEqual => self.binary_cmp(|a, b| a != b)?,
                OpCode::IsIdentical => self.binary_cmp(|a, b| a == b)?,
                OpCode::IsNotIdentical => self.binary_cmp(|a, b| a != b)?,
                OpCode::IsGreater => self.binary_cmp(|a, b| match (a, b) {
                    (Val::Int(i1), Val::Int(i2)) => i1 > i2,
                    _ => false 
                })?,
                OpCode::IsLess => self.binary_cmp(|a, b| match (a, b) {
                    (Val::Int(i1), Val::Int(i2)) => i1 < i2,
                    _ => false 
                })?,
                OpCode::IsGreaterOrEqual => self.binary_cmp(|a, b| match (a, b) {
                    (Val::Int(i1), Val::Int(i2)) => i1 >= i2,
                    _ => false 
                })?,
                OpCode::IsLessOrEqual => self.binary_cmp(|a, b| match (a, b) {
                    (Val::Int(i1), Val::Int(i2)) => i1 <= i2,
                    _ => false 
                })?,
                OpCode::Spaceship => {
                    let b_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let a_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let b_val = &self.arena.get(b_handle).value;
                    let a_val = &self.arena.get(a_handle).value;
                    let res = match (a_val, b_val) {
                        (Val::Int(a), Val::Int(b)) => if a < b { -1 } else if a > b { 1 } else { 0 },
                        _ => 0, // TODO
                    };
                    let res_handle = self.arena.alloc(Val::Int(res));
                    self.operand_stack.push(res_handle);
                }
                OpCode::BoolXor => {
                    let b_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let a_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let b_val = &self.arena.get(b_handle).value;
                    let a_val = &self.arena.get(a_handle).value;
                    
                    let to_bool = |v: &Val| match v {
                        Val::Bool(b) => *b,
                        Val::Int(i) => *i != 0,
                        Val::Null => false,
                        _ => true,
                    };
                    
                    let res = to_bool(a_val) ^ to_bool(b_val);
                    let res_handle = self.arena.alloc(Val::Bool(res));
                    self.operand_stack.push(res_handle);
                }
            }
            Ok(()) })();

            if let Err(e) = res {
                match e {
                    VmError::Exception(h) => {
                        if !self.handle_exception(h) {
                            return Err(VmError::Exception(h));
                        }
                    }
                    _ => return Err(e),
                }
            }
        }
        Ok(())
    }

    fn binary_cmp<F>(&mut self, op: F) -> Result<(), VmError> 
    where F: Fn(&Val, &Val) -> bool {
        let b_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
        let a_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;

        let b_val = &self.arena.get(b_handle).value;
        let a_val = &self.arena.get(a_handle).value;

        let res = op(a_val, b_val);
        let res_handle = self.arena.alloc(Val::Bool(res));
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn binary_op<F>(&mut self, op: F) -> Result<(), VmError> 
    where F: Fn(i64, i64) -> i64 {
        let b_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
        let a_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;

        let b_val = self.arena.get(b_handle).value.clone();
        let a_val = self.arena.get(a_handle).value.clone();

        match (a_val, b_val) {
            (Val::Int(a), Val::Int(b)) => {
                let res = op(a, b);
                let res_handle = self.arena.alloc(Val::Int(res));
                self.operand_stack.push(res_handle);
                Ok(())
            }
            _ => Err(VmError::RuntimeError("Type error: expected Ints".into())),
        }
    }

    fn assign_dim_value(&mut self, array_handle: Handle, key_handle: Handle, val_handle: Handle) -> Result<(), VmError> {
        // Check if we have a reference at the key
        let key_val = &self.arena.get(key_handle).value;
        let key = match key_val {
            Val::Int(i) => ArrayKey::Int(*i),
            Val::String(s) => ArrayKey::Str(s.clone()),
            _ => return Err(VmError::RuntimeError("Invalid array key".into())),
        };

        let array_zval = self.arena.get(array_handle);
        if let Val::Array(map) = &array_zval.value {
            if let Some(existing_handle) = map.get(&key) {
                if self.arena.get(*existing_handle).is_ref {
                    // Update the value pointed to by the reference
                    let new_val = self.arena.get(val_handle).value.clone();
                    self.arena.get_mut(*existing_handle).value = new_val;
                    
                    self.operand_stack.push(array_handle);
                    return Ok(());
                }
            }
        }
        
        self.assign_dim(array_handle, key_handle, val_handle)
    }

    fn assign_dim(&mut self, array_handle: Handle, key_handle: Handle, val_handle: Handle) -> Result<(), VmError> {
        let key_val = &self.arena.get(key_handle).value;
        let key = match key_val {
            Val::Int(i) => ArrayKey::Int(*i),
            Val::String(s) => ArrayKey::Str(s.clone()),
            _ => return Err(VmError::RuntimeError("Invalid array key".into())),
        };

        let is_ref = self.arena.get(array_handle).is_ref;
        
        if is_ref {
            let array_zval_mut = self.arena.get_mut(array_handle);
            
            if let Val::Null | Val::Bool(false) = array_zval_mut.value {
                array_zval_mut.value = Val::Array(indexmap::IndexMap::new());
            }

            if let Val::Array(map) = &mut array_zval_mut.value {
                map.insert(key, val_handle);
            } else {
                    return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            self.operand_stack.push(array_handle);
        } else {
            let array_zval = self.arena.get(array_handle);
            let mut new_val = array_zval.value.clone();
            
            if let Val::Null | Val::Bool(false) = new_val {
                new_val = Val::Array(indexmap::IndexMap::new());
            }

            if let Val::Array(ref mut map) = new_val {
                map.insert(key, val_handle);
            } else {
                    return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            
            let new_handle = self.arena.alloc(new_val);
            self.operand_stack.push(new_handle);
        }
        Ok(())
    }

    fn append_array(&mut self, array_handle: Handle, val_handle: Handle) -> Result<(), VmError> {
        let is_ref = self.arena.get(array_handle).is_ref;

        if is_ref {
            let array_zval_mut = self.arena.get_mut(array_handle);
            
            if let Val::Null | Val::Bool(false) = array_zval_mut.value {
                array_zval_mut.value = Val::Array(indexmap::IndexMap::new());
            }

            if let Val::Array(map) = &mut array_zval_mut.value {
                let next_key = map.keys().filter_map(|k| match k {
                    ArrayKey::Int(i) => Some(*i),
                    _ => None
                }).max().map(|i| i + 1).unwrap_or(0);
                
                map.insert(ArrayKey::Int(next_key), val_handle);
            } else {
                    return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            self.operand_stack.push(array_handle);
        } else {
            let array_zval = self.arena.get(array_handle);
            let mut new_val = array_zval.value.clone();
            
            if let Val::Null | Val::Bool(false) = new_val {
                new_val = Val::Array(indexmap::IndexMap::new());
            }

            if let Val::Array(ref mut map) = new_val {
                let next_key = map.keys().filter_map(|k| match k {
                    ArrayKey::Int(i) => Some(*i),
                    _ => None
                }).max().map(|i| i + 1).unwrap_or(0);
                
                map.insert(ArrayKey::Int(next_key), val_handle);
            } else {
                    return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            
            let new_handle = self.arena.alloc(new_val);
            self.operand_stack.push(new_handle);
        }
        Ok(())
    }

    fn assign_nested_dim(&mut self, array_handle: Handle, keys: &[Handle], val_handle: Handle) -> Result<(), VmError> {
        // We need to traverse down, creating copies if necessary (COW), 
        // then update the bottom, then reconstruct the path up.
        
        let new_handle = self.assign_nested_recursive(array_handle, keys, val_handle)?;
        self.operand_stack.push(new_handle);
        Ok(())
    }
    
    fn assign_nested_recursive(&mut self, current_handle: Handle, keys: &[Handle], val_handle: Handle) -> Result<Handle, VmError> {
        if keys.is_empty() {
            // Should not happen if called correctly, but if it does, it means we replace the current value?
            // Or maybe we just return val_handle?
            // If keys is empty, we are at the target.
            return Ok(val_handle);
        }
        
        let key_handle = keys[0];
        let remaining_keys = &keys[1..];
        
        // COW: Clone current array
        let current_zval = self.arena.get(current_handle);
        let mut new_val = current_zval.value.clone();
        
        if let Val::Null | Val::Bool(false) = new_val {
            new_val = Val::Array(indexmap::IndexMap::new());
        }
        
        if let Val::Array(ref mut map) = new_val {
            // Resolve key
            let key_val = &self.arena.get(key_handle).value;
            let key = if let Val::AppendPlaceholder = key_val {
                let next_key = map.keys().filter_map(|k| match k {
                    ArrayKey::Int(i) => Some(*i),
                    _ => None
                }).max().map(|i| i + 1).unwrap_or(0);
                ArrayKey::Int(next_key)
            } else {
                match key_val {
                    Val::Int(i) => ArrayKey::Int(*i),
                    Val::String(s) => ArrayKey::Str(s.clone()),
                    _ => return Err(VmError::RuntimeError("Invalid array key".into())),
                }
            };

            if remaining_keys.is_empty() {
                // We are at the last key.
                let mut updated_ref = false;
                if let Some(existing_handle) = map.get(&key) {
                    if self.arena.get(*existing_handle).is_ref {
                        // Update Ref value
                        let new_val = self.arena.get(val_handle).value.clone();
                        self.arena.get_mut(*existing_handle).value = new_val;
                        updated_ref = true;
                    }
                }
                
                if !updated_ref {
                    map.insert(key, val_handle);
                }
            } else {
                // We need to go deeper.
                let next_handle = if let Some(h) = map.get(&key) {
                    *h
                } else {
                    // Create empty array
                    self.arena.alloc(Val::Array(indexmap::IndexMap::new()))
                };
                
                let new_next_handle = self.assign_nested_recursive(next_handle, remaining_keys, val_handle)?;
                map.insert(key, new_next_handle);
            }
        } else {
            return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
        }
        
        let new_handle = self.arena.alloc(new_val);
        Ok(new_handle)
    }
}

    #[cfg(test)]

mod tests {
    use super::*;
    use crate::core::value::Symbol;
    use std::sync::Arc;
    use crate::runtime::context::EngineContext;
    use crate::compiler::chunk::UserFunc;
    use crate::builtins::stdlib::{php_strlen, php_str_repeat};

    fn create_vm() -> VM {
        let mut functions = std::collections::HashMap::new();
        functions.insert(b"strlen".to_vec(), php_strlen as crate::runtime::context::NativeHandler);
        functions.insert(b"str_repeat".to_vec(), php_str_repeat as crate::runtime::context::NativeHandler);
        
        let engine = Arc::new(EngineContext {
            functions,
            constants: std::collections::HashMap::new(),
        });
        
        VM::new(engine)
    }

    #[test]
    fn test_store_dim_stack_order() {
        // Stack: [val, key, array]
        // StoreDim should assign val to array[key].
        
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(1)); // 0: val
        chunk.constants.push(Val::Int(0)); // 1: key
        // array will be created dynamically
        
        // Create array [0]
        chunk.code.push(OpCode::InitArray(0));
        chunk.code.push(OpCode::Const(1)); // key 0
        chunk.code.push(OpCode::Const(1)); // val 0 (dummy)
        chunk.code.push(OpCode::AssignDim); // Stack: [array]
        
        // Now stack has [array].
        // We want to test StoreDim with [val, key, array].
        // But we have [array].
        // We need to push val, key, then array.
        // But array is already there.
        
        // Let's manually construct stack in VM.
        let mut vm = create_vm();
        let array_handle = vm.arena.alloc(Val::Array(indexmap::IndexMap::new()));
        let key_handle = vm.arena.alloc(Val::Int(0));
        let val_handle = vm.arena.alloc(Val::Int(99));
        
        vm.operand_stack.push(val_handle);
        vm.operand_stack.push(key_handle);
        vm.operand_stack.push(array_handle);
        
        // Stack: [val, key, array] (Top is array)
        
        let mut chunk = CodeChunk::default();
        chunk.code.push(OpCode::StoreDim);
        
        vm.run(Rc::new(chunk)).unwrap();
        
        let result_handle = vm.operand_stack.pop().unwrap();
        let result = vm.arena.get(result_handle);
        
        if let Val::Array(map) = &result.value {
            let key = ArrayKey::Int(0);
            let val = map.get(&key).unwrap();
            let val_val = vm.arena.get(*val);
            if let Val::Int(i) = val_val.value {
                assert_eq!(i, 99);
            } else {
                panic!("Expected Int(99)");
            }
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_calculator_1_plus_2_mul_3() {
        // 1 + 2 * 3 = 7
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(1)); // 0
        chunk.constants.push(Val::Int(2)); // 1
        chunk.constants.push(Val::Int(3)); // 2
        
        chunk.code.push(OpCode::Const(0));
        chunk.code.push(OpCode::Const(1));
        chunk.code.push(OpCode::Const(2));
        chunk.code.push(OpCode::Mul);
        chunk.code.push(OpCode::Add);
        
        let mut vm = create_vm();
        vm.run(Rc::new(chunk)).unwrap();
        
        let result_handle = vm.operand_stack.pop().unwrap();
        let result = vm.arena.get(result_handle);
        
        if let Val::Int(val) = result.value {
            assert_eq!(val, 7);
        } else {
            panic!("Expected Int result");
        }
    }

    #[test]
    fn test_control_flow_if_else() {
        // if (false) { $b = 10; } else { $b = 20; }
        // $b should be 20
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(0)); // 0: False
        chunk.constants.push(Val::Int(10)); // 1: 10
        chunk.constants.push(Val::Int(20)); // 2: 20
        
        let var_b = Symbol(1);

        // 0: Const(0) (False)
        chunk.code.push(OpCode::Const(0));
        // 1: JmpIfFalse(5) -> Jump to 5 (Else)
        chunk.code.push(OpCode::JmpIfFalse(5));
        // 2: Const(1) (10)
        chunk.code.push(OpCode::Const(1));
        // 3: StoreVar($b)
        chunk.code.push(OpCode::StoreVar(var_b));
        // 4: Jmp(7) -> Jump to 7 (End)
        chunk.code.push(OpCode::Jmp(7));
        // 5: Const(2) (20)
        chunk.code.push(OpCode::Const(2));
        // 6: StoreVar($b)
        chunk.code.push(OpCode::StoreVar(var_b));
        // 7: LoadVar($b)
        chunk.code.push(OpCode::LoadVar(var_b));
        
        let mut vm = create_vm();
        vm.run(Rc::new(chunk)).unwrap();
        
        let result_handle = vm.operand_stack.pop().unwrap();
        let result = vm.arena.get(result_handle);
        
        if let Val::Int(val) = result.value {
            assert_eq!(val, 20);
        } else {
            panic!("Expected Int result 20, got {:?}", result.value);
        }
    }

    #[test]
    fn test_echo_and_call() {
        // echo str_repeat("hi", 3);
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::String(b"hi".to_vec())); // 0
        chunk.constants.push(Val::Int(3)); // 1
        chunk.constants.push(Val::String(b"str_repeat".to_vec())); // 2
        
        // Push "str_repeat" (function name)
        chunk.code.push(OpCode::Const(2));
        // Push "hi"
        chunk.code.push(OpCode::Const(0));
        // Push 3
        chunk.code.push(OpCode::Const(1));
        
        // Call(2) -> pops 2 args, then pops func
        chunk.code.push(OpCode::Call(2));
        // Echo -> pops result
        chunk.code.push(OpCode::Echo);
        
        let mut vm = create_vm();
        vm.run(Rc::new(chunk)).unwrap();
        
        assert!(vm.operand_stack.is_empty());
    }

    #[test]
    fn test_user_function_call() {
        // function add($a, $b) { return $a + $b; }
        // echo add(1, 2);
        
        // Construct function chunk
        let mut func_chunk = CodeChunk::default();
        // Params: $a (Sym 0), $b (Sym 1)
        // Code: LoadVar($a), LoadVar($b), Add, Return
        let sym_a = Symbol(0);
        let sym_b = Symbol(1);
        
        func_chunk.code.push(OpCode::LoadVar(sym_a));
        func_chunk.code.push(OpCode::LoadVar(sym_b));
        func_chunk.code.push(OpCode::Add);
        func_chunk.code.push(OpCode::Return);
        
        let user_func = UserFunc {
            params: vec![
                FuncParam { name: sym_a, by_ref: false },
                FuncParam { name: sym_b, by_ref: false }
            ],
            uses: Vec::new(),
            chunk: Rc::new(func_chunk),
            is_static: false,
            is_generator: false,
            statics: Rc::new(RefCell::new(HashMap::new())),
        };
        
        // Main chunk
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(1)); // 0
        chunk.constants.push(Val::Int(2)); // 1
        chunk.constants.push(Val::String(b"add".to_vec())); // 2
        
        // Push "add"
        chunk.code.push(OpCode::Const(2));
        // Push 1
        chunk.code.push(OpCode::Const(0));
        // Push 2
        chunk.code.push(OpCode::Const(1));
        
        // Call(2)
        chunk.code.push(OpCode::Call(2));
        // Echo (result 3)
        chunk.code.push(OpCode::Echo);
        
        let mut vm = create_vm();
        
        let sym_add = vm.context.interner.intern(b"add");
        vm.context.user_functions.insert(sym_add, Rc::new(user_func));
        
        vm.run(Rc::new(chunk)).unwrap();
        
        assert!(vm.operand_stack.is_empty());
    }
}
