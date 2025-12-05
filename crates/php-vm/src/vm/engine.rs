use std::rc::Rc;
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::core::heap::Arena;
use crate::core::value::{Val, ArrayKey, Handle, ObjectData, Symbol, Visibility};
use crate::vm::stack::Stack;
use crate::vm::opcode::OpCode;
use crate::compiler::chunk::{CodeChunk, UserFunc, ClosureData};
use crate::vm::frame::CallFrame;
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
        let mut current = Some(child);
        while let Some(name) = current {
            if name == parent { return true; }
            if let Some(def) = self.context.classes.get(&name) {
                current = def.parent;
            } else {
                break;
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

    fn get_current_class(&self) -> Option<Symbol> {
        self.frames.last().and_then(|f| f.class_scope)
    }

    fn check_prop_visibility(&self, class_name: Symbol, prop_name: Symbol, current_scope: Option<Symbol>) -> Result<(), VmError> {
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
                println!("IP: {}, Op: {:?}, Stack: {}", frame.ip, op, self.operand_stack.len());
                frame.ip += 1;
                op
            };

            let res = (|| -> Result<(), VmError> { match op {
                OpCode::Throw => {
                    let ex_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    return Err(VmError::Exception(ex_handle));
                }
                OpCode::Const(idx) => {
                    let frame = self.frames.last().unwrap();
                    let val = frame.chunk.constants[idx as usize].clone();
                    let handle = self.arena.alloc(val);
                    self.operand_stack.push(handle);
                }
                OpCode::Pop => {
                    self.operand_stack.pop();
                }
                OpCode::Add => self.binary_op(|a, b| a + b)?,
                OpCode::Sub => self.binary_op(|a, b| a - b)?,
                OpCode::Mul => self.binary_op(|a, b| a * b)?,
                OpCode::Div => self.binary_op(|a, b| a / b)?,
                
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
                OpCode::StoreVar(sym) => {
                    let val_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.insert(sym, val_handle);
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
                    
                    let closure_data = ClosureData {
                        func: user_func,
                        captures,
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
                                    for (i, param_sym) in user_func.params.iter().enumerate() {
                                        if i < args.len() {
                                            frame.locals.insert(*param_sym, args[i]);
                                        }
                                    }
                                    self.frames.push(frame);
                                } else {
                                    return Err(VmError::RuntimeError(format!("Undefined function: {:?}", String::from_utf8_lossy(&func_name_bytes))));
                                }
                            }
                        }
                        Val::Object(payload_handle) => {
                            let payload_val = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload_val.value {
                                if let Some(internal) = &obj_data.internal {
                                    if let Ok(closure) = internal.clone().downcast::<ClosureData>() {
                                        let mut frame = CallFrame::new(closure.func.chunk.clone());
                                        
                                        for (i, param_sym) in closure.func.params.iter().enumerate() {
                                            if i < args.len() {
                                                frame.locals.insert(*param_sym, args[i]);
                                            }
                                        }
                                        
                                        for (sym, handle) in &closure.captures {
                                            frame.locals.insert(*sym, *handle);
                                        }
                                        
                                        self.frames.push(frame);
                                    } else {
                                        return Err(VmError::RuntimeError("Object is not a closure".into()));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError("Object is not a closure".into()));
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

                    if self.frames.is_empty() {
                        self.last_return_value = Some(ret_val);
                        return Ok(());
                    }

                    if popped_frame.is_constructor {
                        if let Some(this_handle) = popped_frame.this {
                            self.operand_stack.push(this_handle);
                        } else {
                             return Err(VmError::RuntimeError("Constructor frame missing 'this'".into()));
                        }
                    } else {
                        self.operand_stack.push(ret_val);
                    }
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
                    let chunk = emitter.compile(program.statements);
                    
                    let mut frame = CallFrame::new(Rc::new(chunk));
                    if let Some(current_frame) = self.frames.last() {
                        frame.locals = current_frame.locals.clone();
                    }
                    self.frames.push(frame);
                }
                
                OpCode::InitArray => {
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
                    self.assign_dim(array_handle, key_handle, val_handle)?;
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
                    // Stack: [Array]
                    // Peek array
                    let array_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_val = &self.arena.get(array_handle).value;
                    
                    let len = match array_val {
                        Val::Array(map) => map.len(),
                        _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                    };
                    
                    if len == 0 {
                        // Empty array, jump to end
                        self.operand_stack.pop(); // Pop array
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    } else {
                        // Push index 0
                        let idx_handle = self.arena.alloc(Val::Int(0));
                        self.operand_stack.push(idx_handle);
                    }
                }
                
                OpCode::IterValid(target) => {
                    // Stack: [Array, Index]
                    let idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    // println!("IterValid: Stack len={}, Array={:?}, Index={:?}", self.operand_stack.len(), self.arena.get(array_handle).value, self.arena.get(idx_handle).value);

                    let idx = match self.arena.get(idx_handle).value {
                        Val::Int(i) => i as usize,
                        _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                    };
                    
                    let array_val = &self.arena.get(array_handle).value;
                    let len = match array_val {
                        Val::Array(map) => map.len(),
                        _ => return Err(VmError::RuntimeError(format!("Foreach expects array, got {:?}", array_val).into())),
                    };
                    
                    if idx >= len {
                        // Finished
                        self.operand_stack.pop(); // Pop Index
                        self.operand_stack.pop(); // Pop Array
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }
                
                OpCode::IterNext => {
                    // Stack: [Array, Index]
                    let idx_handle = self.operand_stack.pop().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let idx = match self.arena.get(idx_handle).value {
                        Val::Int(i) => i,
                        _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                    };
                    
                    let new_idx_handle = self.arena.alloc(Val::Int(idx + 1));
                    self.operand_stack.push(new_idx_handle);
                }
                
                OpCode::IterGetVal(sym) => {
                    // Stack: [Array, Index]
                    let idx_handle = self.operand_stack.peek().ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    let array_handle = self.operand_stack.peek_at(1).ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    
                    let idx = match self.arena.get(idx_handle).value {
                        Val::Int(i) => i as usize,
                        _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                    };
                    
                    let array_val = &self.arena.get(array_handle).value;
                    if let Val::Array(map) = array_val {
                        if let Some((_, val_handle)) = map.get_index(idx) {
                            // Store in local
                            let frame = self.frames.last_mut().unwrap();
                            frame.locals.insert(sym, *val_handle);
                        } else {
                            return Err(VmError::RuntimeError("Iterator index out of bounds".into()));
                        }
                    }
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

                OpCode::DefClass(name, parent) => {
                    let class_def = ClassDef {
                        name,
                        parent,
                        methods: HashMap::new(),
                        properties: IndexMap::new(),
                        constants: HashMap::new(),
                        static_properties: HashMap::new(),
                    };
                    self.context.classes.insert(name, class_def);
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
                            frame.this = Some(obj_handle);
                            frame.is_constructor = true;
                            frame.class_scope = Some(defined_class);

                            for (i, param) in constructor.params.iter().enumerate() {
                                if i < args.len() {
                                    frame.locals.insert(*param, args[i]);
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
                    let obj_zval = self.arena.get(obj_handle);
                    if let Val::Object(payload_handle) = obj_zval.value {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            // Check visibility
                            let current_scope = self.get_current_class();
                            self.check_prop_visibility(obj_data.class, prop_name, current_scope)?;

                            if let Some(prop_handle) = obj_data.properties.get(&prop_name) {
                                self.operand_stack.push(*prop_handle);
                            } else {
                                let null = self.arena.alloc(Val::Null);
                                self.operand_stack.push(null);
                            }
                        } else {
                             return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError("Attempt to fetch property on non-object".into()));
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
                    
                    // Check visibility before modification
                    // Need to get class name from payload first
                    let class_name = if let Val::ObjPayload(obj_data) = &self.arena.get(payload_handle).value {
                        obj_data.class
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    };
                    
                    let current_scope = self.get_current_class();
                    self.check_prop_visibility(class_name, prop_name, current_scope)?;

                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, val_handle);
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                    
                    self.operand_stack.push(val_handle);
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
                    
                    if let Some((user_func, visibility, is_static, defined_class)) = self.find_method(class_name, method_name) {
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
                        if !is_static {
                            frame.this = Some(obj_handle);
                        }
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(class_name);
                        
                        for (i, param) in user_func.params.iter().enumerate() {
                            if i < args.len() {
                                frame.locals.insert(*param, args[i]);
                            }
                        }
                        
                        self.frames.push(frame);
                    } else {
                        return Err(VmError::RuntimeError("Method not found".into()));
                    }
                }
                OpCode::CallStaticMethod(class_name, method_name, arg_count) => {
                    let resolved_class = self.resolve_class_name(class_name)?;
                    
                    if let Some((user_func, visibility, is_static, defined_class)) = self.find_method(resolved_class, method_name) {
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
                        frame.this = None;
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(resolved_class);
                        
                        for (i, param) in user_func.params.iter().enumerate() {
                            if i < args.len() {
                                frame.locals.insert(*param, args[i]);
                            }
                        }
                        
                        self.frames.push(frame);
                    } else {
                        return Err(VmError::RuntimeError("Method not found".into()));
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
                // We are at the last key. Just assign val.
                map.insert(key, val_handle);
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
        chunk.code.push(OpCode::InitArray);
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
            params: vec![sym_a, sym_b],
            uses: Vec::new(),
            chunk: Rc::new(func_chunk),
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
