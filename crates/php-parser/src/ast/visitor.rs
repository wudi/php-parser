use super::*;

pub trait Visitor<'ast> {
    fn visit_program(&mut self, program: &'ast Program<'ast>) {
        walk_program(self, program);
    }

    fn visit_stmt(&mut self, stmt: StmtId<'ast>) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        walk_expr(self, expr);
    }

    fn visit_arg(&mut self, arg: &'ast Arg<'ast>) {
        walk_arg(self, arg);
    }

    fn visit_array_item(&mut self, item: &'ast ArrayItem<'ast>) {
        walk_array_item(self, item);
    }

    fn visit_param(&mut self, param: &'ast Param<'ast>) {
        walk_param(self, param);
    }

    fn visit_static_var(&mut self, var: &'ast StaticVar<'ast>) {
        walk_static_var(self, var);
    }

    fn visit_match_arm(&mut self, arm: &'ast MatchArm<'ast>) {
        walk_match_arm(self, arm);
    }

    fn visit_closure_use(&mut self, closure_use: &'ast ClosureUse<'ast>) {
        walk_closure_use(self, closure_use);
    }

    fn visit_name(&mut self, name: &Name<'ast>) {
        walk_name(self, name);
    }

    fn visit_type(&mut self, ty: &'ast Type<'ast>) {
        walk_type(self, ty);
    }

    fn visit_attribute_group(&mut self, group: &'ast AttributeGroup<'ast>) {
        walk_attribute_group(self, group);
    }

    fn visit_attribute(&mut self, attribute: &'ast Attribute<'ast>) {
        walk_attribute(self, attribute);
    }

    fn visit_use_item(&mut self, use_item: &'ast UseItem<'ast>) {
        walk_use_item(self, use_item);
    }

    fn visit_case(&mut self, case: &'ast Case<'ast>) {
        walk_case(self, case);
    }

    fn visit_catch(&mut self, catch: &'ast Catch<'ast>) {
        walk_catch(self, catch);
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        walk_class_member(self, member);
    }

    fn visit_class_const(&mut self, class_const: &'ast ClassConst<'ast>) {
        walk_class_const(self, class_const);
    }

    fn visit_declare_item(&mut self, item: &'ast DeclareItem<'ast>) {
        walk_declare_item(self, item);
    }

    fn visit_trait_adaptation(&mut self, adaptation: &'ast TraitAdaptation<'ast>) {
        walk_trait_adaptation(self, adaptation);
    }

    fn visit_trait_method_ref(&mut self, method_ref: &'ast TraitMethodRef<'ast>) {
        walk_trait_method_ref(self, method_ref);
    }

    fn visit_property_entry(&mut self, entry: &'ast PropertyEntry<'ast>) {
        walk_property_entry(self, entry);
    }

    fn visit_property_hook(&mut self, hook: &'ast PropertyHook<'ast>) {
        walk_property_hook(self, hook);
    }

    fn visit_property_hook_body(&mut self, body: &'ast PropertyHookBody<'ast>) {
        walk_property_hook_body(self, body);
    }

    fn visit_parse_error(&mut self, error: &'ast ParseError) {
        let _ = error;
    }
}

pub fn walk_program<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    program: &'ast Program<'ast>,
) {
    walk_statements(visitor, program.statements);

    for error in program.errors {
        visitor.visit_parse_error(error);
    }
}

pub fn walk_stmt<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, stmt: StmtId<'ast>) {
    match *stmt {
        Stmt::Echo { exprs, .. } => walk_exprs(visitor, exprs),
        Stmt::Return { expr, .. } => {
            if let Some(expr) = expr {
                visitor.visit_expr(expr);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            visitor.visit_expr(condition);
            walk_statements(visitor, then_block);
            if let Some(else_block) = else_block {
                walk_statements(visitor, else_block);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            visitor.visit_expr(condition);
            walk_statements(visitor, body);
        }
        Stmt::DoWhile {
            body, condition, ..
        } => {
            walk_statements(visitor, body);
            visitor.visit_expr(condition);
        }
        Stmt::For {
            init,
            condition,
            loop_expr,
            body,
            ..
        } => {
            walk_exprs(visitor, init);
            walk_exprs(visitor, condition);
            walk_exprs(visitor, loop_expr);
            walk_statements(visitor, body);
        }
        Stmt::Foreach {
            expr,
            key_var,
            value_var,
            body,
            ..
        } => {
            visitor.visit_expr(expr);
            if let Some(key_var) = key_var {
                visitor.visit_expr(key_var);
            }
            visitor.visit_expr(value_var);
            walk_statements(visitor, body);
        }
        Stmt::Block { statements, .. } => walk_statements(visitor, statements),
        Stmt::Function {
            attributes,
            params,
            return_type,
            body,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_params(visitor, params);
            if let Some(return_type) = return_type {
                visitor.visit_type(return_type);
            }
            walk_statements(visitor, body);
        }
        Stmt::Class {
            attributes,
            extends,
            implements,
            members,
            ..
        } => {
            walk_attributes(visitor, attributes);
            if let Some(extends) = extends {
                visitor.visit_name(&extends);
            }
            walk_names(visitor, implements);
            walk_class_members(visitor, members);
        }
        Stmt::Interface {
            attributes,
            extends,
            members,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_names(visitor, extends);
            walk_class_members(visitor, members);
        }
        Stmt::Trait {
            attributes,
            members,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_class_members(visitor, members);
        }
        Stmt::Enum {
            attributes,
            backed_type,
            implements,
            members,
            ..
        } => {
            walk_attributes(visitor, attributes);
            if let Some(backed_type) = backed_type {
                visitor.visit_type(backed_type);
            }
            walk_names(visitor, implements);
            walk_class_members(visitor, members);
        }
        Stmt::Namespace { name, body, .. } => {
            if let Some(name) = name {
                visitor.visit_name(&name);
            }
            if let Some(body) = body {
                walk_statements(visitor, body);
            }
        }
        Stmt::Use { uses, .. } => {
            for use_item in uses {
                visitor.visit_use_item(use_item);
            }
        }
        Stmt::Switch {
            condition, cases, ..
        } => {
            visitor.visit_expr(condition);
            for case in cases {
                visitor.visit_case(case);
            }
        }
        Stmt::Try {
            body,
            catches,
            finally,
            ..
        } => {
            walk_statements(visitor, body);
            for catch in catches {
                visitor.visit_catch(catch);
            }
            if let Some(finally) = finally {
                walk_statements(visitor, finally);
            }
        }
        Stmt::Throw { expr, .. } => visitor.visit_expr(expr),
        Stmt::Const {
            attributes, consts, ..
        } => {
            walk_attributes(visitor, attributes);
            for class_const in consts {
                visitor.visit_class_const(class_const);
            }
        }
        Stmt::Break { level, .. } | Stmt::Continue { level, .. } => {
            if let Some(level) = level {
                visitor.visit_expr(level);
            }
        }
        Stmt::Global { vars, .. } | Stmt::Unset { vars, .. } => {
            walk_exprs(visitor, vars);
        }
        Stmt::Static { vars, .. } => {
            for var in vars {
                visitor.visit_static_var(var);
            }
        }
        Stmt::Expression { expr, .. } => visitor.visit_expr(expr),
        Stmt::InlineHtml { .. }
        | Stmt::Nop { .. }
        | Stmt::Label { .. }
        | Stmt::Goto { .. }
        | Stmt::Error { .. }
        | Stmt::HaltCompiler { .. } => {}
        Stmt::Declare { declares, body, .. } => {
            for declare in declares {
                visitor.visit_declare_item(declare);
            }
            walk_statements(visitor, body);
        }
    }
}

pub fn walk_expr<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, expr: ExprId<'ast>) {
    match *expr {
        Expr::Assign { var, expr, .. } | Expr::AssignRef { var, expr, .. } => {
            visitor.visit_expr(var);
            visitor.visit_expr(expr);
        }
        Expr::AssignOp { var, expr, .. } => {
            visitor.visit_expr(var);
            visitor.visit_expr(expr);
        }
        Expr::Binary { left, right, .. } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        Expr::Unary { expr, .. }
        | Expr::PostInc { var: expr, .. }
        | Expr::PostDec { var: expr, .. }
        | Expr::Print { expr, .. }
        | Expr::Clone { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Empty { expr, .. }
        | Expr::Eval { expr, .. }
        | Expr::Die {
            expr: Some(expr), ..
        }
        | Expr::Exit {
            expr: Some(expr), ..
        } => visitor.visit_expr(expr),
        Expr::Call { func, args, .. }
        | Expr::MethodCall {
            target: func, args, ..
        }
        | Expr::StaticCall {
            class: func, args, ..
        }
        | Expr::NullsafeMethodCall {
            target: func, args, ..
        } => {
            visitor.visit_expr(func);
            for arg in args {
                visitor.visit_arg(arg);
            }
        }
        Expr::Array { items, .. } => {
            for item in items {
                visitor.visit_array_item(item);
            }
        }
        Expr::ArrayDimFetch { array, dim, .. } => {
            visitor.visit_expr(array);
            if let Some(dim) = dim {
                visitor.visit_expr(dim);
            }
        }
        Expr::PropertyFetch {
            target, property, ..
        }
        | Expr::NullsafePropertyFetch {
            target, property, ..
        }
        | Expr::ClassConstFetch {
            class: target,
            constant: property,
            ..
        } => {
            visitor.visit_expr(target);
            visitor.visit_expr(property);
        }
        Expr::New { class, args, .. } => {
            visitor.visit_expr(class);
            for arg in args {
                visitor.visit_arg(arg);
            }
        }
        Expr::InterpolatedString { parts, .. } | Expr::ShellExec { parts, .. } => {
            walk_exprs(visitor, parts);
        }
        Expr::Include { expr, .. } => visitor.visit_expr(expr),
        Expr::Ternary {
            condition,
            if_true,
            if_false,
            ..
        } => {
            visitor.visit_expr(condition);
            if let Some(if_true) = if_true {
                visitor.visit_expr(if_true);
            }
            visitor.visit_expr(if_false);
        }
        Expr::Match {
            condition, arms, ..
        } => {
            visitor.visit_expr(condition);
            for arm in arms {
                visitor.visit_match_arm(arm);
            }
        }
        Expr::AnonymousClass {
            attributes,
            args,
            extends,
            implements,
            members,
            ..
        } => {
            walk_attributes(visitor, attributes);
            for arg in args {
                visitor.visit_arg(arg);
            }
            if let Some(extends) = extends {
                visitor.visit_name(&extends);
            }
            walk_names(visitor, implements);
            walk_class_members(visitor, members);
        }
        Expr::Yield {
            key, value, from, ..
        } => {
            if let Some(key) = key {
                visitor.visit_expr(key);
            }
            if let Some(value) = value {
                visitor.visit_expr(value);
            }
            let _ = from;
        }
        Expr::Isset { vars, .. } => walk_exprs(visitor, vars),
        Expr::Closure {
            attributes,
            params,
            uses,
            return_type,
            body,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_params(visitor, params);
            for closure_use in uses {
                visitor.visit_closure_use(closure_use);
            }
            if let Some(return_type) = return_type {
                visitor.visit_type(return_type);
            }
            walk_statements(visitor, body);
        }
        Expr::ArrowFunction {
            attributes,
            params,
            return_type,
            expr,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_params(visitor, params);
            if let Some(return_type) = return_type {
                visitor.visit_type(return_type);
            }
            visitor.visit_expr(expr);
        }
        Expr::Variable { .. }
        | Expr::Integer { .. }
        | Expr::Float { .. }
        | Expr::Boolean { .. }
        | Expr::Null { .. }
        | Expr::String { .. }
        | Expr::MagicConst { .. }
        | Expr::VariadicPlaceholder { .. }
        | Expr::Error { .. } => {}
        Expr::IndirectVariable { name, .. } => {
            visitor.visit_expr(name);
        }
        Expr::Die { expr: None, .. } | Expr::Exit { expr: None, .. } => {}
    }
}

pub fn walk_arg<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, arg: &'ast Arg<'ast>) {
    visitor.visit_expr(arg.value);
}

pub fn walk_array_item<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    item: &'ast ArrayItem<'ast>,
) {
    if let Some(key) = item.key {
        visitor.visit_expr(key);
    }
    visitor.visit_expr(item.value);
}

pub fn walk_param<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, param: &'ast Param<'ast>) {
    walk_attributes(visitor, param.attributes);
    if let Some(ty) = param.ty {
        visitor.visit_type(ty);
    }
    if let Some(default) = param.default {
        visitor.visit_expr(default);
    }
    if let Some(hooks) = param.hooks {
        walk_property_hooks(visitor, hooks);
    }
}

pub fn walk_static_var<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    var: &'ast StaticVar<'ast>,
) {
    visitor.visit_expr(var.var);
    if let Some(default) = var.default {
        visitor.visit_expr(default);
    }
}

pub fn walk_match_arm<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, arm: &'ast MatchArm<'ast>) {
    if let Some(conditions) = arm.conditions {
        walk_exprs(visitor, conditions);
    }
    visitor.visit_expr(arm.body);
}

pub fn walk_closure_use<'ast, V: Visitor<'ast> + ?Sized>(
    _: &mut V,
    closure_use: &'ast ClosureUse<'ast>,
) {
    let _ = closure_use;
}

pub fn walk_name<'ast, V: Visitor<'ast> + ?Sized>(_: &mut V, name: &Name<'ast>) {
    let _ = name;
}

pub fn walk_type<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, ty: &'ast Type<'ast>) {
    match ty {
        Type::Simple(_) => {}
        Type::Name(name) => visitor.visit_name(name),
        Type::Union(types) | Type::Intersection(types) => walk_types(visitor, types),
        Type::Nullable(inner) => visitor.visit_type(inner),
    }
}

pub fn walk_attribute_group<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    group: &'ast AttributeGroup<'ast>,
) {
    for attribute in group.attributes {
        visitor.visit_attribute(attribute);
    }
}

pub fn walk_attribute<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    attribute: &'ast Attribute<'ast>,
) {
    visitor.visit_name(&attribute.name);
    for arg in attribute.args {
        visitor.visit_arg(arg);
    }
}

pub fn walk_use_item<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    use_item: &'ast UseItem<'ast>,
) {
    visitor.visit_name(&use_item.name);
}

pub fn walk_case<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, case: &'ast Case<'ast>) {
    if let Some(condition) = case.condition {
        visitor.visit_expr(condition);
    }
    walk_statements(visitor, case.body);
}

pub fn walk_catch<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, catch: &'ast Catch<'ast>) {
    walk_names(visitor, catch.types);
    walk_statements(visitor, catch.body);
}

pub fn walk_class_member<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    member: &'ast ClassMember<'ast>,
) {
    match member {
        ClassMember::Property {
            attributes,
            ty,
            entries,
            ..
        } => {
            walk_attributes(visitor, attributes);
            if let Some(ty) = ty {
                visitor.visit_type(ty);
            }
            for entry in entries.iter() {
                visitor.visit_property_entry(entry);
            }
        }
        ClassMember::PropertyHook {
            attributes,
            ty,
            default,
            hooks,
            ..
        } => {
            walk_attributes(visitor, attributes);
            if let Some(ty) = ty {
                visitor.visit_type(ty);
            }
            if let Some(default) = default {
                visitor.visit_expr(default);
            }
            walk_property_hooks(visitor, hooks);
        }
        ClassMember::Method {
            attributes,
            params,
            return_type,
            body,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_params(visitor, params);
            if let Some(return_type) = return_type {
                visitor.visit_type(return_type);
            }
            walk_statements(visitor, body);
        }
        ClassMember::Const {
            attributes,
            ty,
            consts,
            ..
        } => {
            walk_attributes(visitor, attributes);
            if let Some(ty) = ty {
                visitor.visit_type(ty);
            }
            for class_const in consts.iter() {
                visitor.visit_class_const(class_const);
            }
        }
        ClassMember::TraitUse {
            attributes,
            traits,
            adaptations,
            ..
        } => {
            walk_attributes(visitor, attributes);
            walk_names(visitor, traits);
            for adaptation in adaptations.iter() {
                visitor.visit_trait_adaptation(adaptation);
            }
        }
        ClassMember::Case {
            attributes, value, ..
        } => {
            walk_attributes(visitor, attributes);
            if let Some(value) = value {
                visitor.visit_expr(value);
            }
        }
    }
}

pub fn walk_class_const<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    class_const: &'ast ClassConst<'ast>,
) {
    visitor.visit_expr(class_const.value);
}

pub fn walk_declare_item<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    item: &'ast DeclareItem<'ast>,
) {
    visitor.visit_expr(item.value);
}

pub fn walk_trait_adaptation<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    adaptation: &'ast TraitAdaptation<'ast>,
) {
    match adaptation {
        TraitAdaptation::Precedence {
            method, insteadof, ..
        } => {
            visitor.visit_trait_method_ref(method);
            walk_names(visitor, insteadof);
        }
        TraitAdaptation::Alias { method, .. } => {
            visitor.visit_trait_method_ref(method);
        }
    }
}

pub fn walk_trait_method_ref<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    method_ref: &'ast TraitMethodRef<'ast>,
) {
    if let Some(trait_name) = method_ref.trait_name {
        visitor.visit_name(&trait_name);
    }
}

pub fn walk_property_entry<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    entry: &'ast PropertyEntry<'ast>,
) {
    if let Some(default) = entry.default {
        visitor.visit_expr(default);
    }
}

pub fn walk_property_hook<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    hook: &'ast PropertyHook<'ast>,
) {
    walk_attributes(visitor, hook.attributes);
    walk_params(visitor, hook.params);
    visitor.visit_property_hook_body(&hook.body);
}

pub fn walk_property_hook_body<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    body: &'ast PropertyHookBody<'ast>,
) {
    match body {
        PropertyHookBody::None => {}
        PropertyHookBody::Statements(statements) => walk_statements(visitor, statements),
        PropertyHookBody::Expr(expr) => visitor.visit_expr(expr),
    }
}

fn walk_statements<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    statements: &'ast [StmtId<'ast>],
) {
    for stmt in statements.iter().copied() {
        visitor.visit_stmt(stmt);
    }
}

fn walk_exprs<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, exprs: &'ast [ExprId<'ast>]) {
    for expr in exprs.iter().copied() {
        visitor.visit_expr(expr);
    }
}

fn walk_attributes<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    attributes: &'ast [AttributeGroup<'ast>],
) {
    for group in attributes {
        visitor.visit_attribute_group(group);
    }
}

fn walk_params<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, params: &'ast [Param<'ast>]) {
    for param in params {
        visitor.visit_param(param);
    }
}

fn walk_types<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, types: &'ast [Type<'ast>]) {
    for ty in types {
        visitor.visit_type(ty);
    }
}

fn walk_names<'ast, V: Visitor<'ast> + ?Sized>(visitor: &mut V, names: &'ast [Name<'ast>]) {
    for name in names {
        visitor.visit_name(name);
    }
}

fn walk_property_hooks<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    hooks: &'ast [PropertyHook<'ast>],
) {
    for hook in hooks {
        visitor.visit_property_hook(hook);
    }
}

fn walk_class_members<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    members: &'ast [ClassMember<'ast>],
) {
    for member in members {
        visitor.visit_class_member(member);
    }
}
