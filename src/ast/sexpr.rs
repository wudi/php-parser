use crate::ast::visitor::Visitor;
use crate::ast::*;

pub struct SExprFormatter<'a> {
    output: String,
    indent: usize,
    source: &'a [u8],
}

impl<'a> SExprFormatter<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            output: String::new(),
            indent: 0,
            source,
        }
    }

    pub fn finish(self) -> String {
        self.output
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
    }
}

impl<'a, 'ast> Visitor<'ast> for SExprFormatter<'a> {
    fn visit_program(&mut self, program: &'ast Program<'ast>) {
        self.write("(program");
        self.indent += 1;
        for stmt in program.statements {
            self.newline();
            self.visit_stmt(*stmt);
        }
        for error in program.errors {
            self.newline();
            self.visit_parse_error(error);
        }
        self.indent -= 1;
        self.write(")");
    }

    fn visit_stmt(&mut self, stmt: StmtId<'ast>) {
        match stmt {
            Stmt::Block { statements, .. } => {
                self.write("(block");
                self.indent += 1;
                for stmt in *statements {
                    self.newline();
                    self.visit_stmt(*stmt);
                }
                self.indent -= 1;
                self.write(")");
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.write("(if ");
                self.visit_expr(condition);
                self.indent += 1;
                self.newline();
                self.write("(then");
                self.indent += 1;
                for stmt in *then_block {
                    self.newline();
                    self.visit_stmt(*stmt);
                }
                self.indent -= 1;
                self.write(")");
                if let Some(else_block) = else_block {
                    self.newline();
                    self.write("(else");
                    self.indent += 1;
                    for stmt in *else_block {
                        self.newline();
                        self.visit_stmt(*stmt);
                    }
                    self.indent -= 1;
                    self.write(")");
                }
                self.indent -= 1;
                self.write(")");
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.write("(while ");
                self.visit_expr(condition);
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Echo { exprs, .. } => {
                self.write("(echo");
                for expr in *exprs {
                    self.write(" ");
                    self.visit_expr(*expr);
                }
                self.write(")");
            }
            Stmt::Expression { expr, .. } => {
                self.visit_expr(expr);
            }
            Stmt::DoWhile {
                body, condition, ..
            } => {
                self.write("(do-while ");
                self.visit_expr(condition);
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::For {
                init,
                condition,
                loop_expr,
                body,
                ..
            } => {
                self.write("(for");
                self.indent += 1;
                self.newline();
                self.write("(init");
                for expr in *init {
                    self.write(" ");
                    self.visit_expr(*expr);
                }
                self.write(")");
                self.newline();
                self.write("(cond");
                for expr in *condition {
                    self.write(" ");
                    self.visit_expr(*expr);
                }
                self.write(")");
                self.newline();
                self.write("(loop");
                for expr in *loop_expr {
                    self.write(" ");
                    self.visit_expr(*expr);
                }
                self.write(")");
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Foreach {
                expr,
                key_var,
                value_var,
                body,
                ..
            } => {
                self.write("(foreach ");
                self.visit_expr(expr);
                self.write(" ");
                if let Some(key) = key_var {
                    self.visit_expr(key);
                    self.write(" ");
                }
                self.visit_expr(value_var);
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Return { expr, .. } => {
                self.write("(return");
                if let Some(expr) = expr {
                    self.write(" ");
                    self.visit_expr(expr);
                }
                self.write(")");
            }
            Stmt::Break { level, .. } => {
                self.write("(break");
                if let Some(level) = level {
                    self.write(" ");
                    self.visit_expr(level);
                }
                self.write(")");
            }
            Stmt::Continue { level, .. } => {
                self.write("(continue");
                if let Some(level) = level {
                    self.write(" ");
                    self.visit_expr(level);
                }
                self.write(")");
            }
            Stmt::Switch {
                condition, cases, ..
            } => {
                self.write("(switch ");
                self.visit_expr(condition);
                self.indent += 1;
                for case in *cases {
                    self.newline();
                    self.visit_case(case);
                }
                self.indent -= 1;
                self.write(")");
            }
            Stmt::Try {
                body,
                catches,
                finally,
                ..
            } => {
                self.write("(try");
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write(")");
                for catch in *catches {
                    self.newline();
                    self.visit_catch(catch);
                }
                if let Some(finally) = finally {
                    self.newline();
                    self.write("(finally");
                    self.indent += 1;
                    for stmt in *finally {
                        self.newline();
                        self.visit_stmt(*stmt);
                    }
                    self.indent -= 1;
                    self.write(")");
                }
                self.indent -= 1;
                self.write(")");
            }
            Stmt::Throw { expr, .. } => {
                self.write("(throw ");
                self.visit_expr(expr);
                self.write(")");
            }
            Stmt::Function {
                attributes,
                name,
                params,
                return_type,
                body,
                by_ref,
                ..
            } => {
                self.write("(function");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                if *by_ref {
                    self.write(" &");
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                self.write(" (params");
                for param in *params {
                    self.write(" ");
                    self.visit_param(param);
                }
                self.write(")");
                if let Some(rt) = return_type {
                    self.write(" (return-type ");
                    self.visit_type(rt);
                    self.write(")");
                }
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Class {
                attributes,
                modifiers,
                name,
                extends,
                implements,
                members,
                ..
            } => {
                self.write("(class");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                for modifier in *modifiers {
                    self.write(" ");
                    self.write(&String::from_utf8_lossy(modifier.text(self.source)));
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                if let Some(extends) = extends {
                    self.write(" (extends ");
                    self.visit_name(&extends);
                    self.write(")");
                }
                if !implements.is_empty() {
                    self.write(" (implements");
                    for iface in *implements {
                        self.write(" ");
                        self.visit_name(iface);
                    }
                    self.write(")");
                }
                self.indent += 1;
                self.newline();
                self.write("(members");
                self.indent += 1;
                for member in *members {
                    self.newline();
                    self.visit_class_member(member);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Interface {
                attributes,
                name,
                extends,
                members,
                ..
            } => {
                self.write("(interface");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                if !extends.is_empty() {
                    self.write(" (extends");
                    for iface in *extends {
                        self.write(" ");
                        self.visit_name(iface);
                    }
                    self.write(")");
                }
                self.indent += 1;
                self.newline();
                self.write("(members");
                self.indent += 1;
                for member in *members {
                    self.newline();
                    self.visit_class_member(member);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Trait {
                attributes,
                name,
                members,
                ..
            } => {
                self.write("(trait");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                self.indent += 1;
                self.newline();
                self.write("(members");
                self.indent += 1;
                for member in *members {
                    self.newline();
                    self.visit_class_member(member);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Enum {
                attributes,
                name,
                members,
                ..
            } => {
                self.write("(enum");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                self.indent += 1;
                self.newline();
                self.write("(members");
                self.indent += 1;
                for member in *members {
                    self.newline();
                    self.visit_class_member(member);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Namespace { name, body, .. } => {
                self.write("(namespace");
                if let Some(name) = name {
                    self.write(" ");
                    self.visit_name(&name);
                }
                if let Some(body) = body {
                    self.indent += 1;
                    self.newline();
                    self.write("(body");
                    self.indent += 1;
                    for stmt in *body {
                        self.newline();
                        self.visit_stmt(*stmt);
                    }
                    self.indent -= 1;
                    self.write("))");
                    self.indent -= 1;
                }
                self.write(")");
            }
            Stmt::Use { uses, .. } => {
                self.write("(use");
                for use_item in *uses {
                    self.write(" ");
                    self.visit_use_item(use_item);
                }
                self.write(")");
            }
            Stmt::Global { vars, .. } => {
                self.write("(global");
                for var in *vars {
                    self.write(" ");
                    self.visit_expr(var);
                }
                self.write(")");
            }
            Stmt::Static { vars, .. } => {
                self.write("(static");
                for var in *vars {
                    self.write(" ");
                    self.visit_static_var(var);
                }
                self.write(")");
            }
            Stmt::Unset { vars, .. } => {
                self.write("(unset");
                for var in *vars {
                    self.write(" ");
                    self.visit_expr(var);
                }
                self.write(")");
            }
            Stmt::Goto { label, .. } => {
                self.write("(goto \"");
                self.write(&String::from_utf8_lossy(label.text(self.source)));
                self.write("\")");
            }
            Stmt::Label { name, .. } => {
                self.write("(label \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\")");
            }
            Stmt::HaltCompiler { .. } => self.write("(halt-compiler)"),
            Stmt::Declare { declares, body, .. } => {
                self.write("(declare");
                for declare in *declares {
                    self.write(" ");
                    self.visit_declare_item(declare);
                }
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
                self.write(")");
            }
            Stmt::Const {
                attributes,
                consts,
                ..
            } => {
                self.write("(const");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                for c in *consts {
                    self.write(" ");
                    self.visit_class_const(c);
                }
                self.write(")");
            }
            Stmt::InlineHtml { value, .. } => {
                self.write("(inline-html \"");
                self.write(&String::from_utf8_lossy(value));
                self.write("\")");
            }
            Stmt::Error { .. } => self.write("(error)"),
            Stmt::Nop { .. } => self.write("(nop)"),
        }
    }

    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        match expr {
            Expr::Assign { var, expr, .. } => {
                self.write("(assign ");
                self.visit_expr(var);
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Integer { value, .. } => {
                self.write("(integer ");
                self.write(&String::from_utf8_lossy(value));
                self.write(")");
            }
            Expr::String { value, .. } => {
                self.write("(string \"");
                self.write(&String::from_utf8_lossy(value).replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t"));
                self.write("\")");
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                self.write("(");
                self.write(match op {
                    BinaryOp::Plus => "+",
                    BinaryOp::Minus => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::Concat => ".",
                    BinaryOp::Eq => "=",
                    BinaryOp::EqEq => "==",
                    BinaryOp::EqEqEq => "===",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::NotEqEq => "!==",
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    BinaryOp::BitXor => "^",
                    BinaryOp::Coalesce => "??",
                    BinaryOp::Spaceship => "<=>",
                    BinaryOp::Pow => "**",
                    BinaryOp::ShiftLeft => "<<",
                    BinaryOp::ShiftRight => ">>",
                    BinaryOp::LogicalAnd => "and",
                    BinaryOp::LogicalOr => "or",
                    BinaryOp::LogicalXor => "xor",
                    BinaryOp::Instanceof => "instanceof",
                });
                self.write(" ");
                self.visit_expr(left);
                self.write(" ");
                self.visit_expr(right);
                self.write(")");
            }
            Expr::Variable { name, .. } => {
                self.write("(variable \"");
                self.write(&String::from_utf8_lossy(name.as_str(self.source)));
                self.write("\")");
            }
            Expr::Unary { op, expr, .. } => {
                self.write("(");
                self.write(match op {
                    UnaryOp::Plus => "+",
                    UnaryOp::Minus => "-",
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "~",
                    UnaryOp::PreInc => "++",
                    UnaryOp::PreDec => "--",
                    UnaryOp::ErrorSuppress => "@",
                    UnaryOp::Reference => "&",
                });
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::PostInc { var, .. } => {
                self.write("(post-inc ");
                self.visit_expr(var);
                self.write(")");
            }
            Expr::PostDec { var, .. } => {
                self.write("(post-dec ");
                self.visit_expr(var);
                self.write(")");
            }
            Expr::AssignOp { var, op, expr, .. } => {
                self.write("(assign-op ");
                self.write(match op {
                    AssignOp::Plus => "+=",
                    AssignOp::Minus => "-=",
                    AssignOp::Mul => "*=",
                    AssignOp::Div => "/=",
                    AssignOp::Mod => "%=",
                    AssignOp::Concat => ".=",
                    AssignOp::BitAnd => "&=",
                    AssignOp::BitOr => "|=",
                    AssignOp::BitXor => "^=",
                    AssignOp::ShiftLeft => "<<=",
                    AssignOp::ShiftRight => ">>=",
                    AssignOp::Pow => "**=",
                    AssignOp::Coalesce => "??=",
                });
                self.write(" ");
                self.visit_expr(var);
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::AssignRef { var, expr, .. } => {
                self.write("(assign-ref ");
                self.visit_expr(var);
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Call { func, args, .. } => {
                self.write("(call ");
                self.visit_expr(func);
                self.write(" (args");
                for arg in *args {
                    self.write(" ");
                    self.visit_arg(arg);
                }
                self.write("))");
            }
            Expr::MethodCall {
                target,
                method,
                args,
                ..
            } => {
                self.write("(method-call ");
                self.visit_expr(target);
                self.write("->");
                self.visit_expr(method);
                self.write(" (args");
                for arg in *args {
                    self.write(" ");
                    self.visit_arg(arg);
                }
                self.write("))");
            }
            Expr::StaticCall {
                class,
                method,
                args,
                ..
            } => {
                self.write("(static-call ");
                self.visit_expr(class);
                self.write("::");
                self.visit_expr(method);
                self.write(" (args");
                for arg in *args {
                    self.write(" ");
                    self.visit_arg(arg);
                }
                self.write("))");
            }
            Expr::Array { items, .. } => {
                self.write("(array");
                for item in *items {
                    self.write(" ");
                    self.visit_array_item(item);
                }
                self.write(")");
            }
            Expr::ArrayDimFetch { array, dim, .. } => {
                self.write("(array-dim-fetch ");
                self.visit_expr(array);
                if let Some(dim) = dim {
                    self.write(" ");
                    self.visit_expr(dim);
                }
                self.write(")");
            }
            Expr::PropertyFetch {
                target, property, ..
            } => {
                self.write("(property-fetch ");
                self.visit_expr(target);
                self.write("->");
                self.visit_expr(property);
                self.write(")");
            }
            Expr::ClassConstFetch {
                class, constant, ..
            } => {
                self.write("(class-const-fetch ");
                self.visit_expr(class);
                self.write("::");
                self.visit_expr(constant);
                self.write(")");
            }
            Expr::New { class, args, .. } => {
                self.write("(new ");
                self.visit_expr(class);
                self.write(" (args");
                for arg in *args {
                    self.write(" ");
                    self.visit_arg(arg);
                }
                self.write("))");
            }
            Expr::Clone { expr, .. } => {
                self.write("(clone ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Ternary {
                condition,
                if_true,
                if_false,
                ..
            } => {
                self.write("(ternary ");
                self.visit_expr(condition);
                self.write(" ");
                if let Some(t) = if_true {
                    self.visit_expr(t);
                } else {
                    self.write("?");
                }
                self.write(" ");
                self.visit_expr(if_false);
                self.write(")");
            }
            Expr::Match {
                condition, arms, ..
            } => {
                self.write("(match ");
                self.visit_expr(condition);
                self.indent += 1;
                for arm in *arms {
                    self.newline();
                    self.visit_match_arm(arm);
                }
                self.indent -= 1;
                self.write(")");
            }
            Expr::Closure {
                attributes,
                is_static,
                by_ref,
                params,
                return_type,
                body,
                uses,
                ..
            } => {
                self.write("(closure");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                if *is_static {
                    self.write(" static");
                }
                if *by_ref {
                    self.write(" &");
                }
                self.write(" (params");
                for param in *params {
                    self.write(" ");
                    self.visit_param(param);
                }
                self.write(")");
                if !uses.is_empty() {
                    self.write(" (uses");
                    for u in *uses {
                        self.write(" ");
                        self.visit_closure_use(u);
                    }
                    self.write(")");
                }
                if let Some(rt) = return_type {
                    self.write(" (return-type ");
                    self.visit_type(rt);
                    self.write(")");
                }
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Expr::ArrowFunction {
                attributes,
                is_static,
                by_ref,
                params,
                return_type,
                expr,
                ..
            } => {
                self.write("(arrow-function");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                if *is_static {
                    self.write(" static");
                }
                if *by_ref {
                    self.write(" &");
                }
                self.write(" (params");
                for param in *params {
                    self.write(" ");
                    self.visit_param(param);
                }
                self.write(")");
                if let Some(rt) = return_type {
                    self.write(" (return-type ");
                    self.visit_type(rt);
                    self.write(")");
                }
                self.write(" => ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Empty { expr, .. } => {
                self.write("(empty ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Isset { vars, .. } => {
                self.write("(isset");
                for var in *vars {
                    self.write(" ");
                    self.visit_expr(var);
                }
                self.write(")");
            }
            Expr::Eval { expr, .. } => {
                self.write("(eval ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Die { expr, .. } => {
                self.write("(die");
                if let Some(expr) = expr {
                    self.write(" ");
                    self.visit_expr(expr);
                }
                self.write(")");
            }
            Expr::Exit { expr, .. } => {
                self.write("(exit");
                if let Some(expr) = expr {
                    self.write(" ");
                    self.visit_expr(expr);
                }
                self.write(")");
            }
            Expr::Cast { kind, expr, .. } => {
                self.write("(cast ");
                self.write(match kind {
                    CastKind::Int => "int",
                    CastKind::Bool => "bool",
                    CastKind::Float => "float",
                    CastKind::String => "string",
                    CastKind::Array => "array",
                    CastKind::Object => "object",
                    CastKind::Unset => "unset",
                    CastKind::Void => "void",
                });
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Yield {
                key, value, from, ..
            } => {
                self.write("(yield");
                if *from {
                    self.write("-from");
                }
                if let Some(key) = key {
                    self.write(" ");
                    self.visit_expr(key);
                    self.write(" =>");
                }
                if let Some(value) = value {
                    self.write(" ");
                    self.visit_expr(value);
                }
                self.write(")");
            }
            Expr::Print { expr, .. } => {
                self.write("(print ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Include { kind, expr, .. } => {
                self.write("(");
                self.write(match kind {
                    IncludeKind::Include => "include",
                    IncludeKind::IncludeOnce => "include_once",
                    IncludeKind::Require => "require",
                    IncludeKind::RequireOnce => "require_once",
                });
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::ShellExec { parts, .. } => {
                self.write("(shell-exec");
                for part in *parts {
                    self.write(" ");
                    self.visit_expr(part);
                }
                self.write(")");
            }
            Expr::InterpolatedString { parts, .. } => {
                self.write("(interpolated-string");
                for part in *parts {
                    self.write(" ");
                    self.visit_expr(part);
                }
                self.write(")");
            }
            Expr::MagicConst { kind, .. } => {
                self.write("(magic-const ");
                self.write(match kind {
                    MagicConstKind::Line => "__LINE__",
                    MagicConstKind::File => "__FILE__",
                    MagicConstKind::Dir => "__DIR__",
                    MagicConstKind::Function => "__FUNCTION__",
                    MagicConstKind::Class => "__CLASS__",
                    MagicConstKind::Trait => "__TRAIT__",
                    MagicConstKind::Method => "__METHOD__",
                    MagicConstKind::Namespace => "__NAMESPACE__",
                    MagicConstKind::Property => "__PROPERTY__",
                });
                self.write(")");
            }
            Expr::Boolean { value, .. } => {
                self.write(if *value { "(true)" } else { "(false)" });
            }
            Expr::Null { .. } => self.write("(null)"),
            Expr::Float { value, .. } => {
                self.write("(float ");
                self.write(&String::from_utf8_lossy(value));
                self.write(")");
            }
            Expr::AnonymousClass {
                args,
                extends,
                implements,
                members,
                ..
            } => {
                self.write("(anonymous-class");
                if !args.is_empty() {
                    self.write(" (args");
                    for arg in *args {
                        self.write(" ");
                        self.visit_arg(arg);
                    }
                    self.write(")");
                }
                if let Some(extends) = extends {
                    self.write(" (extends ");
                    self.visit_name(&extends);
                    self.write(")");
                }
                if !implements.is_empty() {
                    self.write(" (implements");
                    for iface in *implements {
                        self.write(" ");
                        self.visit_name(iface);
                    }
                    self.write(")");
                }
                self.indent += 1;
                self.newline();
                self.write("(members");
                self.indent += 1;
                for member in *members {
                    self.newline();
                    self.visit_class_member(member);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
                self.write(")");
            }
            Expr::NullsafePropertyFetch {
                target, property, ..
            } => {
                self.write("(nullsafe-property-fetch ");
                self.visit_expr(target);
                self.write("?->");
                self.visit_expr(property);
                self.write(")");
            }
            Expr::NullsafeMethodCall {
                target,
                method,
                args,
                ..
            } => {
                self.write("(nullsafe-method-call ");
                self.visit_expr(target);
                self.write("?->");
                self.visit_expr(method);
                self.write(" (args");
                for arg in *args {
                    self.write(" ");
                    self.visit_arg(arg);
                }
                self.write("))");
            }
            Expr::VariadicPlaceholder { .. } => self.write("(...)"),
            Expr::Error { .. } => self.write("(error)"),
        }
    }

    fn visit_name(&mut self, name: &Name<'ast>) {
        for (i, part) in name.parts.iter().enumerate() {
            if i > 0 {
                self.write("\\");
            }
            self.write(&String::from_utf8_lossy(part.text(self.source)));
        }
    }

    fn visit_case(&mut self, case: &'ast Case<'ast>) {
        self.write("(case");
        if let Some(cond) = case.condition {
            self.write(" ");
            self.visit_expr(cond);
        } else {
            self.write(" default");
        }
        self.indent += 1;
        for stmt in case.body {
            self.newline();
            self.visit_stmt(*stmt);
        }
        self.indent -= 1;
        self.write(")");
    }

    fn visit_catch(&mut self, catch: &'ast Catch<'ast>) {
        self.write("(catch (");
        for (i, ty) in catch.types.iter().enumerate() {
            if i > 0 {
                self.write("|");
            }
            self.visit_name(ty);
        }
        self.write(")");
        if let Some(var) = catch.var {
            self.write(" ");
            self.write(&String::from_utf8_lossy(var.text(self.source)));
        }
        self.indent += 1;
        for stmt in catch.body {
            self.newline();
            self.visit_stmt(*stmt);
        }
        self.indent -= 1;
        self.write(")");
    }

    fn visit_param(&mut self, param: &'ast Param<'ast>) {
        self.write("(");
        for attr in param.attributes {
            self.write(" ");
            self.visit_attribute_group(attr);
        }
        for modifier in param.modifiers {
            self.write(" ");
            self.write(&String::from_utf8_lossy(modifier.text(self.source)));
        }
        if let Some(ty) = param.ty {
            self.write(" ");
            self.visit_type(ty);
            self.write(" ");
        }
        if param.variadic {
            self.write("...");
        }
        if param.by_ref {
            self.write("&");
        }
        self.write(&String::from_utf8_lossy(param.name.text(self.source)));
        if let Some(default) = param.default {
            self.write(" = ");
            self.visit_expr(default);
        }
        if let Some(hooks) = param.hooks {
             self.write(" (hooks");
             for hook in hooks {
                 self.write(" ");
                 self.visit_property_hook(hook);
             }
             self.write(")");
        }
        self.write(")");
    }

    fn visit_type(&mut self, ty: &'ast Type<'ast>) {
        match ty {
            Type::Simple(t) => self.write(&String::from_utf8_lossy(t.text(self.source))),
            Type::Name(n) => self.visit_name(&n),
            Type::Union(types) => {
                self.write("(union");
                for t in *types {
                    self.write(" ");
                    self.visit_type(t);
                }
                self.write(")");
            }
            Type::Intersection(types) => {
                self.write("(intersection");
                for t in *types {
                    self.write(" ");
                    self.visit_type(t);
                }
                self.write(")");
            }
            Type::Nullable(t) => {
                self.write("?");
                self.visit_type(t);
            }
        }
    }

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        match member {
            ClassMember::Property {
                attributes,
                modifiers,
                ty,
                entries,
                ..
            } => {
                self.write("(property");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                for modifier in *modifiers {
                    self.write(" ");
                    self.write(&String::from_utf8_lossy(modifier.text(self.source)));
                }
                if let Some(ty) = ty {
                    self.write(" ");
                    self.visit_type(ty);
                }
                for entry in *entries {
                    self.write(" ");
                    self.visit_property_entry(entry);
                }
                self.write(")");
            }
            ClassMember::Method {
                attributes,
                modifiers,
                name,
                params,
                return_type,
                body,
                ..
            } => {
                self.write("(method");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                for modifier in *modifiers {
                    self.write(" ");
                    self.write(&String::from_utf8_lossy(modifier.text(self.source)));
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                self.write(" (params");
                for param in *params {
                    self.write(" ");
                    self.visit_param(param);
                }
                self.write(")");
                if let Some(rt) = return_type {
                    self.write(" (return-type ");
                    self.visit_type(rt);
                    self.write(")");
                }
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            ClassMember::Const {
                attributes,
                modifiers,
                ty,
                consts,
                ..
            } => {
                self.write("(const");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                for modifier in *modifiers {
                    self.write(" ");
                    self.write(&String::from_utf8_lossy(modifier.text(self.source)));
                }
                if let Some(ty) = ty {
                    self.write(" ");
                    self.visit_type(ty);
                }
                for c in *consts {
                    self.write(" ");
                    self.visit_class_const(c);
                }
                self.write(")");
            }
            ClassMember::TraitUse {
                traits,
                adaptations,
                ..
            } => {
                self.write("(trait-use");
                for t in *traits {
                    self.write(" ");
                    self.visit_name(t);
                }
                if !adaptations.is_empty() {
                    self.write(" (adaptations");
                    for a in *adaptations {
                        self.write(" ");
                        self.visit_trait_adaptation(a);
                    }
                    self.write(")");
                }
                self.write(")");
            }
            ClassMember::Case {
                attributes,
                name,
                value,
                ..
            } => {
                self.write("(enum-case");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                self.write(" \"");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                self.write("\"");
                if let Some(v) = value {
                    self.write(" = ");
                    self.visit_expr(v);
                }
                self.write(")");
            }
            ClassMember::PropertyHook {
                attributes,
                modifiers,
                ty,
                name,
                default,
                hooks,
                ..
            } => {
                self.write("(property-hook-def");
                for attr in *attributes {
                    self.write(" ");
                    self.visit_attribute_group(attr);
                }
                for modifier in *modifiers {
                    self.write(" ");
                    self.write(&String::from_utf8_lossy(modifier.text(self.source)));
                }
                if let Some(ty) = ty {
                    self.write(" ");
                    self.visit_type(ty);
                }
                self.write(" ");
                self.write(&String::from_utf8_lossy(name.text(self.source)));
                if let Some(default) = default {
                    self.write(" = ");
                    self.visit_expr(default);
                }
                if !hooks.is_empty() {
                    self.indent += 1;
                    self.newline();
                    self.write("(hooks");
                    self.indent += 1;
                    for hook in *hooks {
                        self.newline();
                        self.visit_property_hook(hook);
                    }
                    self.indent -= 1;
                    self.write(")");
                    self.indent -= 1;
                }
                self.write(")");
            }
        }
    }

    fn visit_arg(&mut self, arg: &'ast Arg<'ast>) {
        if let Some(name) = arg.name {
            self.write(&String::from_utf8_lossy(name.text(self.source)));
            self.write(": ");
        }
        if arg.unpack {
            self.write("...");
        }
        self.visit_expr(arg.value);
    }

    fn visit_array_item(&mut self, item: &'ast ArrayItem<'ast>) {
        self.write("(");
        if let Some(key) = item.key {
            self.visit_expr(key);
            self.write(" => ");
        }
        if item.by_ref {
            self.write("&");
        }
        if item.unpack {
            self.write("...");
        }
        self.visit_expr(item.value);
        self.write(")");
    }

    fn visit_match_arm(&mut self, arm: &'ast MatchArm<'ast>) {
        self.write("(arm");
        if let Some(conds) = arm.conditions {
            self.write(" (conds");
            for cond in conds {
                self.write(" ");
                self.visit_expr(*cond);
            }
            self.write(")");
        } else {
            self.write(" default");
        }
        self.write(" => ");
        self.visit_expr(arm.body);
        self.write(")");
    }

    fn visit_closure_use(&mut self, u: &'ast ClosureUse<'ast>) {
        if u.by_ref {
            self.write("&");
        }
        self.write(&String::from_utf8_lossy(u.var.text(self.source)));
    }

    fn visit_trait_adaptation(&mut self, adaptation: &'ast TraitAdaptation<'ast>) {
        match adaptation {
            TraitAdaptation::Precedence {
                method, insteadof, ..
            } => {
                self.write("(precedence ");
                self.visit_trait_method_ref(method);
                self.write(" insteadof");
                for n in *insteadof {
                    self.write(" ");
                    self.visit_name(n);
                }
                self.write(")");
            }
            TraitAdaptation::Alias { method, alias, visibility, .. } => {
                self.write("(alias ");
                self.visit_trait_method_ref(method);
                self.write(" as");
                if let Some(vis) = visibility {
                    self.write(" ");
                    self.write(&String::from_utf8_lossy(vis.text(self.source)));
                }
                if let Some(alias) = alias {
                    self.write(" \"");
                    self.write(&String::from_utf8_lossy(alias.text(self.source)));
                    self.write("\"");
                }
                self.write(")");
            }
        }
    }

    fn visit_trait_method_ref(&mut self, method: &'ast TraitMethodRef<'ast>) {
        if let Some(trait_name) = method.trait_name {
            self.visit_name(&trait_name);
            self.write("::");
        }
        self.write(&String::from_utf8_lossy(method.method.text(self.source)));
    }

    fn visit_attribute_group(&mut self, group: &'ast AttributeGroup<'ast>) {
        self.write("(attribute-group");
        for attr in group.attributes {
            self.write(" ");
            self.visit_attribute(attr);
        }
        self.write(")");
    }

    fn visit_attribute(&mut self, attribute: &'ast Attribute<'ast>) {
        self.write("(attribute ");
        self.visit_name(&attribute.name);
        if !attribute.args.is_empty() {
            self.write(" (args");
            for arg in attribute.args {
                self.write(" ");
                self.visit_arg(arg);
            }
            self.write(")");
        }
        self.write(")");
    }

    fn visit_static_var(&mut self, var: &'ast StaticVar<'ast>) {
        self.visit_expr(var.var);
        if let Some(default) = var.default {
            self.write(" = ");
            self.visit_expr(default);
        }
    }

    fn visit_use_item(&mut self, use_item: &'ast UseItem<'ast>) {
        match use_item.kind {
            UseKind::Normal => {},
            UseKind::Function => self.write("function "),
            UseKind::Const => self.write("const "),
        }
        self.visit_name(&use_item.name);
        if let Some(alias) = use_item.alias {
            self.write(" as ");
            self.write(&String::from_utf8_lossy(alias.text(self.source)));
        }
    }

    fn visit_class_const(&mut self, c: &'ast ClassConst<'ast>) {
        self.write(&String::from_utf8_lossy(c.name.text(self.source)));
        self.write(" = ");
        self.visit_expr(c.value);
    }

    fn visit_declare_item(&mut self, declare: &'ast DeclareItem<'ast>) {
        self.write(&String::from_utf8_lossy(declare.key.text(self.source)));
        self.write("=");
        self.visit_expr(declare.value);
    }

    fn visit_property_entry(&mut self, entry: &'ast PropertyEntry<'ast>) {
        self.write(&String::from_utf8_lossy(entry.name.text(self.source)));
        if let Some(default) = entry.default {
            self.write(" = ");
            self.visit_expr(default);
        }
    }

    fn visit_parse_error(&mut self, error: &'ast ParseError) {
        self.write("(parse-error \"");
        self.write(&error.message);
        self.write("\")");
    }

    fn visit_property_hook(&mut self, hook: &'ast PropertyHook<'ast>) {
        self.write("(hook");
        for attr in hook.attributes {
            self.write(" ");
            self.visit_attribute_group(attr);
        }
        for modifier in hook.modifiers {
            self.write(" ");
            self.write(&String::from_utf8_lossy(modifier.text(self.source)));
        }
        if hook.by_ref {
            self.write(" &");
        }
        self.write(" ");
        self.write(&String::from_utf8_lossy(hook.name.text(self.source)));
        
        if !hook.params.is_empty() {
            self.write(" (params");
            for param in hook.params {
                self.write(" ");
                self.visit_param(param);
            }
            self.write(")");
        }
        
        self.write(" ");
        self.visit_property_hook_body(&hook.body);
        self.write(")");
    }

    fn visit_property_hook_body(&mut self, body: &'ast PropertyHookBody<'ast>) {
        match body {
            PropertyHookBody::None => self.write("(none)"),
            PropertyHookBody::Expr(expr) => {
                self.write(" => ");
                self.visit_expr(expr);
            }
            PropertyHookBody::Statements(stmts) => {
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *stmts {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write(")");
                self.indent -= 1;
            }
        }
    }
}
