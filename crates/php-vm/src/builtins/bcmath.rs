use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use rust_decimal::Decimal;
use std::rc::Rc;
use std::str::FromStr;

fn get_op(vm: &mut VM, arg: Handle) -> Result<Decimal, String> {
    let val = vm.arena.get(arg);
    match &val.value {
        Val::String(s) => Decimal::from_str(&String::from_utf8_lossy(s)).map_err(|e| e.to_string()),
        Val::Int(i) => Ok(Decimal::from(*i)),
        Val::Float(f) => Decimal::from_str(&f.to_string()).map_err(|e| e.to_string()),
        _ => Err("bcmath functions expect numeric arguments or numeric strings".to_string()),
    }
}

pub fn bcadd(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("bcadd() expects exactly 2 parameters".to_string());
    }

    let left = get_op(vm, args[0])?;
    let right = get_op(vm, args[1])?;

    let result = left + right;
    let result_str = result.to_string();

    Ok(vm
        .arena
        .alloc(Val::String(Rc::new(result_str.into_bytes()))))
}

pub fn bcsub(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("bcsub() expects exactly 2 parameters".to_string());
    }

    let left = get_op(vm, args[0])?;
    let right = get_op(vm, args[1])?;

    let result = left - right;
    let result_str = result.to_string();

    Ok(vm
        .arena
        .alloc(Val::String(Rc::new(result_str.into_bytes()))))
}

pub fn bcmul(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("bcmul() expects exactly 2 parameters".to_string());
    }

    let left = get_op(vm, args[0])?;
    let right = get_op(vm, args[1])?;

    let result = left * right;
    let result_str = result.to_string();

    Ok(vm
        .arena
        .alloc(Val::String(Rc::new(result_str.into_bytes()))))
}

pub fn bcdiv(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("bcdiv() expects 2 or 3 parameters".to_string());
    }

    let left = get_op(vm, args[0])?;
    let right = get_op(vm, args[1])?;

    if right.is_zero() {
        return Err("Division by zero".to_string());
    }

    let mut scale = 0; // Default scale is 0 for bcdiv in PHP if not specified
    if args.len() == 3 {
        let scale_val = vm.arena.get(args[2]);
        if let Val::Int(s) = scale_val.value {
            scale = s as u32;
        } else {
            return Err("bcdiv() scale argument must be an integer".to_string());
        }
    }

    let result = (left / right).trunc_with_scale(scale);
    let result_str = result.to_string();

    Ok(vm
        .arena
        .alloc(Val::String(Rc::new(result_str.into_bytes()))))
}
