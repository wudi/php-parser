/// Callable invocation helpers for function/method calls.
///
/// This module handles the various forms of PHP callables:
/// - Direct function symbols: `foo()`
/// - String callables: `$var = 'strlen'; $var('hello');`
/// - Closures: `function() { ... }()`
/// - Object __invoke: `$obj()`
/// - Array callables: `[$obj, 'method']` or `['Class', 'method']`
///
/// PHP Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_call_function, zend_call_method
/// PHP Reference: $PHP_SRC_PATH/Zend/zend_closures.c - closure invocation
use super::engine::VM;
use crate::compiler::chunk::ClosureData;
use crate::core::value::{ArrayKey, Handle, ObjectData, Symbol, Val};
use crate::vm::engine::{PendingCall, VmError};
use crate::vm::frame::{ArgList, CallFrame, GeneratorData, GeneratorState};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;


impl VM {
    /// Execute a pending function/method call
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_INIT_FCALL handler
    pub(crate) fn execute_pending_call(&mut self, call: PendingCall) -> Result<(), VmError> {
        let callsite_strict_types = self
            .frames
            .last()
            .map(|f| f.chunk.strict_types)
            .unwrap_or(false);

        let PendingCall {
            func_name,
            func_handle,
            args,
            is_static: call_is_static,
            class_name,
            this_handle: call_this,
        } = call;

        if let Some(name) = func_name {
            if let Some(class_name) = class_name {
                // Method call: Class::method() or $obj->method()
                self.invoke_method_symbol(
                    class_name,
                    name,
                    args,
                    call_is_static,
                    call_this,
                    callsite_strict_types,
                )?;
            } else {
                // Function call: foo()
                self.invoke_function_symbol(name, args, callsite_strict_types)?;
            }
        } else if let Some(callable_handle) = func_handle {
            // Variable callable: $var()
            self.invoke_callable_value(callable_handle, args, callsite_strict_types)?;
        } else {
            return Err(VmError::RuntimeError(
                "Dynamic function call not supported yet".into(),
            ));
        }
        Ok(())
    }

    /// Invoke a method by class and method symbol
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_INIT_METHOD_CALL
    #[inline]
    fn invoke_method_symbol(
        &mut self,
        class_name: Symbol,
        method_name: Symbol,
        args: ArgList,
        call_is_static: bool,
        call_this: Option<Handle>,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        let method_lookup = self.find_method(class_name, method_name);
        if let Some((method, visibility, is_static, defining_class)) = method_lookup {
            // Validate static/non-static mismatch
            if is_static != call_is_static {
                if is_static {
                    // PHP allows calling static methods non-statically (with deprecation notice)
                } else {
                    if call_this.is_none() {
                        return Err(VmError::RuntimeError(
                            "Non-static method called statically".into(),
                        ));
                    }
                }
            }

            self.check_method_visibility(defining_class, visibility, Some(method_name))?;

            let mut frame = CallFrame::new(method.chunk.clone());
            frame.func = Some(method.clone());
            frame.this = call_this;
            frame.class_scope = Some(defining_class);
            frame.called_scope = Some(class_name);
            frame.args = args;
            frame.callsite_strict_types = callsite_strict_types;

            // Don't bind params here - let Recv/RecvInit opcodes handle it.
            self.push_frame(frame);
            Ok(())
        } else {
            let name_str =
                String::from_utf8_lossy(self.context.interner.lookup(method_name).unwrap_or(b""));
            let class_str =
                String::from_utf8_lossy(self.context.interner.lookup(class_name).unwrap_or(b""));
            Err(VmError::RuntimeError(format!(
                "Call to undefined method {}::{}",
                class_str, name_str
            )))
        }
    }

    /// Invoke a function by symbol name
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_DO_FCALL
    pub(crate) fn invoke_function_symbol(
        &mut self,
        name: Symbol,
        args: ArgList,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        let name_bytes = self.context.interner.lookup(name).unwrap_or(b"");
        let lower_name = name_bytes.to_ascii_lowercase();

        // Check extension registry
        if let Some(handler) = self.context.engine.registry.get_function(&lower_name) {
            let by_ref = self
                .context
                .engine
                .registry
                .get_function_by_ref(&lower_name)
                .map(|list| list.to_vec());
            self.handle_pending_undefined_for_call(&args, by_ref.as_deref());
            if let Some(by_ref) = by_ref.as_deref() {
                for &idx in by_ref {
                    if let Some(&arg_handle) = args.get(idx) {
                        if !self.arena.get(arg_handle).is_ref {
                            self.arena.get_mut(arg_handle).is_ref = true;
                        }
                        if let Some(&sym) = self.var_handle_map.get(&arg_handle) {
                            if let Some(frame) = self.frames.last_mut() {
                                frame.locals.entry(sym).or_insert(arg_handle);
                            }
                        }
                    }
                }
            }
            // Set caller's strict_types mode for builtin parameter validation
            // Reference: $PHP_SRC_PATH/Zend/zend_compile.h - ZEND_ARG_USES_STRICT_TYPES()
            self.builtin_call_strict = callsite_strict_types;
            let res = handler(self, &args).map_err(VmError::RuntimeError)?;
            self.builtin_call_strict = false; // Reset after call
            self.operand_stack.push(res);
            return Ok(());
        }

        // User-defined function
        let func_opt = self.context.user_functions.get(&name).cloned();
        if let Some(func) = func_opt {
            self.handle_pending_undefined_for_call(&args, None);
            let mut frame = CallFrame::new(func.chunk.clone());
            frame.func = Some(func.clone());
            frame.args = args;
            frame.callsite_strict_types = callsite_strict_types;

            // Handle generator functions
            if func.is_generator {
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
                    dynamic_properties: HashSet::new(),
                };
                let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                let obj_handle = self.arena.alloc(Val::Object(payload_handle));
                self.operand_stack.push(obj_handle);
                return Ok(());
            }

            // Don't bind params here - let Recv/RecvInit opcodes handle it
            // self.bind_params_to_frame(&mut frame, &func.params)?;
            self.push_frame(frame);
            Ok(())
        } else {
            Err(VmError::RuntimeError(format!(
                "Call to undefined function: {}",
                String::from_utf8_lossy(name_bytes)
            )))
        }
    }

    /// Invoke a callable value (string, closure, __invoke object, array)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_call_function
    pub(crate) fn invoke_callable_value(
        &mut self,
        callable_handle: Handle,
        args: ArgList,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        let callable_val = self.arena.get(callable_handle).value.clone();
        match callable_val {
            // String callable: 'strlen'
            Val::String(s) => {
                let sym = self.context.interner.intern(&s);
                self.invoke_function_symbol(sym, args, callsite_strict_types)
            }
            // Object callable: closure or __invoke
            Val::Object(payload_handle) => self.invoke_object_callable(
                payload_handle,
                callable_handle,
                args,
                callsite_strict_types,
            ),
            // Array callable: [$obj, 'method'] or ['Class', 'method']
            Val::Array(map) => self.invoke_array_callable(&map.map, args, callsite_strict_types),
            _ => Err(VmError::RuntimeError(format!(
                "Call expects function name or closure (got {})",
                self.describe_handle(callable_handle)
            ))),
        }
    }

    /// Invoke an object as a callable (closure or __invoke)
    /// Reference: $PHP_SRC_PATH/Zend/zend_closures.c
    #[inline]
    fn invoke_object_callable(
        &mut self,
        payload_handle: Handle,
        obj_handle: Handle,
        args: ArgList,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        let payload_val = self.arena.get(payload_handle);
        if let Val::ObjPayload(obj_data) = &payload_val.value {
            // Try closure first
            if let Some(internal) = &obj_data.internal {
                if let Ok(closure) = internal.clone().downcast::<ClosureData>() {
                    self.push_closure_frame(&closure, args, callsite_strict_types);
                    return Ok(());
                }
            }

            // Try __invoke magic method
            let invoke_sym = self.context.interner.intern(b"__invoke");

            // Check for native __invoke first
            if let Some(native_entry) = self.find_native_method(obj_data.class, invoke_sym) {
                self.check_method_visibility(
                    native_entry.declaring_class,
                    native_entry.visibility,
                    Some(invoke_sym),
                )?;

                // Set this in current frame temporarily for native method to access
                let saved_this = self.frames.last().and_then(|f| f.this);
                if let Some(frame) = self.frames.last_mut() {
                    frame.this = Some(obj_handle);
                }

                // Set caller's strict_types mode for builtin parameter validation
                self.builtin_call_strict = callsite_strict_types;
                // Call native handler
                let result = (native_entry.handler)(self, &args).map_err(VmError::RuntimeError)?;
                self.builtin_call_strict = false; // Reset after call

                // Restore previous this
                if let Some(frame) = self.frames.last_mut() {
                    frame.this = saved_this;
                }

                self.operand_stack.push(result);
                return Ok(());
            }

            if let Some((method, visibility, _, defining_class)) =
                self.find_method(obj_data.class, invoke_sym)
            {
                self.check_method_visibility(defining_class, visibility, Some(invoke_sym))?;
                self.push_method_frame(
                    method,
                    Some(obj_handle),
                    defining_class,
                    obj_data.class,
                    args,
                    callsite_strict_types,
                );
                Ok(())
            } else {
                Err(VmError::RuntimeError(
                    "Object is not a closure and does not implement __invoke".into(),
                ))
            }
        } else {
            Err(VmError::RuntimeError("Invalid object payload".into()))
        }
    }

    /// Invoke array callable: [$obj, 'method'] or ['Class', 'method']
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - is_callable
    fn invoke_array_callable(
        &mut self,
        map: &IndexMap<ArrayKey, Handle>,
        args: ArgList,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        if map.len() != 2 {
            return Err(VmError::RuntimeError(
                "Callable array must have exactly 2 elements".into(),
            ));
        }

        let class_or_obj = map
            .get_index(0)
            .map(|(_, v)| *v)
            .ok_or(VmError::RuntimeError("Invalid callable array".into()))?;
        let method_handle = map
            .get_index(1)
            .map(|(_, v)| *v)
            .ok_or(VmError::RuntimeError("Invalid callable array".into()))?;

        let method_name_bytes = self.convert_to_string(method_handle)?;
        let method_sym = self.context.interner.intern(&method_name_bytes);

        let class_or_obj_val = self.arena.get(class_or_obj).value.clone();
        match class_or_obj_val {
            // Static method call: ['ClassName', 'method']
            Val::String(class_name_bytes) => self.invoke_static_array_callable(
                &class_name_bytes,
                method_sym,
                &method_name_bytes,
                args,
                callsite_strict_types,
            ),
            // Instance method call: [$obj, 'method']
            Val::Object(payload_handle) => self.invoke_instance_array_callable(
                payload_handle,
                class_or_obj,
                method_sym,
                &method_name_bytes,
                args,
                callsite_strict_types,
            ),
            _ => Err(VmError::RuntimeError(
                "First element of callable array must be object or class name".into(),
            )),
        }
    }

    /// Invoke static method from array callable: ['Class', 'method']
    #[inline]
    fn invoke_static_array_callable(
        &mut self,
        class_name_bytes: &[u8],
        method_sym: Symbol,
        method_name_bytes: &[u8],
        args: ArgList,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        let class_sym = self.context.interner.intern(class_name_bytes);
        let class_sym = self.resolve_class_name(class_sym)?;

        // Check for native method first
        if let Some(native_entry) = self.find_native_method(class_sym, method_sym) {
            self.check_method_visibility(
                native_entry.declaring_class,
                native_entry.visibility,
                Some(method_sym),
            )?;
            let result = (native_entry.handler)(self, &args).map_err(VmError::RuntimeError)?;
            self.operand_stack.push(result);
            return Ok(());
        }

        if let Some((method, visibility, _, defining_class)) =
            self.find_method(class_sym, method_sym)
        {
            self.check_method_visibility(defining_class, visibility, Some(method_sym))?;
            self.push_method_frame(
                method,
                None,
                defining_class,
                class_sym,
                args,
                callsite_strict_types,
            );
            Ok(())
        } else {
            let class_str = String::from_utf8_lossy(class_name_bytes);
            let method_str = String::from_utf8_lossy(method_name_bytes);
            Err(VmError::RuntimeError(format!(
                "Call to undefined method {}::{}",
                class_str, method_str
            )))
        }
    }

    /// Invoke instance method from array callable: [$obj, 'method']
    #[inline]
    fn invoke_instance_array_callable(
        &mut self,
        payload_handle: Handle,
        obj_handle: Handle,
        method_sym: Symbol,
        method_name_bytes: &[u8],
        args: ArgList,
        callsite_strict_types: bool,
    ) -> Result<(), VmError> {
        let payload_val = self.arena.get(payload_handle);
        if let Val::ObjPayload(obj_data) = &payload_val.value {
            let class_name = obj_data.class;

            // Check for native method first
            if let Some(native_entry) = self.find_native_method(class_name, method_sym) {
                self.check_method_visibility(
                    native_entry.declaring_class,
                    native_entry.visibility,
                    Some(method_sym),
                )?;

                // Set this in current frame temporarily for native method to access
                let saved_this = self.frames.last().and_then(|f| f.this);
                if let Some(frame) = self.frames.last_mut() {
                    frame.this = Some(obj_handle);
                }

                // Call native handler
                let result = (native_entry.handler)(self, &args).map_err(VmError::RuntimeError)?;

                // Restore previous this
                if let Some(frame) = self.frames.last_mut() {
                    frame.this = saved_this;
                }

                self.operand_stack.push(result);
                return Ok(());
            }

            if let Some((method, visibility, _, defining_class)) =
                self.find_method(class_name, method_sym)
            {
                self.check_method_visibility(defining_class, visibility, Some(method_sym))?;
                self.push_method_frame(
                    method,
                    Some(obj_handle),
                    defining_class,
                    class_name,
                    args,
                    callsite_strict_types,
                );
                Ok(())
            } else {
                let class_str = String::from_utf8_lossy(
                    self.context.interner.lookup(class_name).unwrap_or(b"?"),
                );
                let method_str = String::from_utf8_lossy(method_name_bytes);
                Err(VmError::RuntimeError(format!(
                    "Call to undefined method {}::{}",
                    class_str, method_str
                )))
            }
        } else {
            Err(VmError::RuntimeError(
                "Invalid object in callable array".into(),
            ))
        }
    }
}
