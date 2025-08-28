package ast

import "fmt"

// ASTKind 表示AST节点类型，对应PHP官方zend_ast.h的定义
type ASTKind int

// 特殊节点 (0-3) - ZEND_AST_SPECIAL
const (
	AST_ZVAL     ASTKind = 0  // 字面量值
	AST_CONSTANT ASTKind = 1  // 命名常量
	AST_ZNODE    ASTKind = 2  // 编译时节点(内部使用)
	AST_FUNC_DECL ASTKind = 3 // 函数声明(特殊处理)
)

// 声明节点 (64-73) - ZEND_AST_SPECIAL + 固定子节点数
const (
	AST_CLOSURE     ASTKind = 64 // 匿名函数/闭包
	AST_METHOD      ASTKind = 65 // 类方法
	AST_CLASS       ASTKind = 66 // 类声明
	AST_ARROW_FUNC  ASTKind = 67 // 箭头函数
	AST_ENUM        ASTKind = 68 // 枚举声明
)

// 列表节点 (128-149) - ZEND_AST_IS_LIST + 可变子节点数
const (
	AST_ARG_LIST            ASTKind = 128 // 参数列表
	AST_ARRAY               ASTKind = 129 // 数组字面量
	AST_ENCAPS_LIST         ASTKind = 130 // 字符串插值列表
	AST_EXPR_LIST           ASTKind = 131 // 表达式列表
	AST_STMT_LIST           ASTKind = 132 // 语句列表
	AST_IF                  ASTKind = 133 // if语句链
	AST_SWITCH_LIST         ASTKind = 134 // switch案例列表
	AST_CATCH_LIST          ASTKind = 135 // catch子句列表
	AST_PARAM_LIST          ASTKind = 136 // 形参列表
	AST_CLOSURE_USES        ASTKind = 137 // use变量列表
	AST_PROP_GROUP          ASTKind = 138 // 属性组
	AST_CONST_DECL          ASTKind = 139 // 常量声明列表
	AST_CLASS_CONST_GROUP   ASTKind = 140 // 类常量组
	AST_NAME_LIST           ASTKind = 141 // 名称列表
	AST_TRAIT_ADAPTATIONS   ASTKind = 142 // trait适配列表
	AST_USE                 ASTKind = 143 // use声明列表
	AST_ATTRIBUTE_GROUP     ASTKind = 144 // 属性组
	AST_MATCH_ARM_LIST      ASTKind = 145 // match分支列表
	AST_ENUM_CASE_LIST      ASTKind = 146 // 枚举案例列表
	AST_PROPERTY_HOOK_LIST  ASTKind = 147 // 属性钩子列表
)

// 表达式节点 - 0个子节点 (256-257)
const (
	AST_MAGIC_CONST ASTKind = 256 // 魔术常量 (__FILE__, __LINE__, etc.)
	AST_TYPE        ASTKind = 257 // 类型声明
)

// 表达式节点 - 1个子节点 (320-351)
const (
	AST_VAR                   ASTKind = 320 // 变量 $var
	AST_CONST                 ASTKind = 321 // 常量引用
	AST_UNPACK                ASTKind = 322 // 解包操作 ...$arr
	AST_UNARY_PLUS            ASTKind = 323 // 一元加 +$x
	AST_UNARY_MINUS           ASTKind = 324 // 一元减 -$x
	AST_CAST                  ASTKind = 325 // 类型转换 (int)$x
	AST_EMPTY                 ASTKind = 326 // empty($x)
	AST_ISSET                 ASTKind = 327 // isset($x)
	AST_SILENCE               ASTKind = 328 // 错误抑制 @$x
	AST_SHELL_EXEC            ASTKind = 329 // 反引号执行 `command`
	AST_CLONE                 ASTKind = 330 // clone $obj
	AST_EXIT                  ASTKind = 331 // exit/die
	AST_PRINT                 ASTKind = 332 // print $x
	AST_INCLUDE_OR_EVAL       ASTKind = 333 // include/require/eval
	AST_UNARY_OP              ASTKind = 334 // 通用一元运算 (!$x, ~$x)
	AST_PRE_INC               ASTKind = 335 // 前缀++ ++$x
	AST_PRE_DEC               ASTKind = 336 // 前缀-- --$x
	AST_POST_INC              ASTKind = 337 // 后缀++ $x++
	AST_POST_DEC              ASTKind = 338 // 后缀-- $x--
	AST_YIELD_FROM            ASTKind = 339 // yield from $gen
	AST_GLOBAL                ASTKind = 340 // global $x
	AST_UNSET                 ASTKind = 341 // unset($x)
	AST_RETURN                ASTKind = 342 // return $x
	AST_LABEL                 ASTKind = 343 // label:
	AST_REF                   ASTKind = 344 // 引用 &$x
	AST_HALT_COMPILER         ASTKind = 345 // __halt_compiler()
	AST_ECHO                  ASTKind = 346 // echo $x
	AST_THROW                 ASTKind = 347 // throw $e
	AST_GOTO                  ASTKind = 348 // goto label
	AST_BREAK                 ASTKind = 349 // break [expr]
	AST_CONTINUE              ASTKind = 350 // continue [expr]
)

// 表达式节点 - 2个子节点 (384-415)
const (
	AST_DIM                    ASTKind = 384 // 数组访问 $arr[$key]
	AST_PROP                   ASTKind = 385 // 属性访问 $obj->prop
	AST_NULLSAFE_PROP          ASTKind = 386 // 空安全属性 $obj?->prop
	AST_STATIC_PROP            ASTKind = 387 // 静态属性 Class::$prop
	AST_CALL                   ASTKind = 388 // 函数调用 func($args)
	AST_CLASS_CONST            ASTKind = 389 // 类常量 Class::CONST
	AST_ASSIGN                 ASTKind = 390 // 赋值 $x = $y
	AST_ASSIGN_REF             ASTKind = 391 // 引用赋值 $x =& $y
	AST_ASSIGN_OP              ASTKind = 392 // 复合赋值 $x += $y
	AST_BINARY_OP              ASTKind = 393 // 二元运算 $x + $y
	AST_ARRAY_ELEM             ASTKind = 394 // 数组元素 $key => $value
	AST_NEW                    ASTKind = 395 // new Class($args)
	AST_INSTANCEOF             ASTKind = 396 // $obj instanceof Class
	AST_YIELD                  ASTKind = 397 // yield $key => $value
	AST_COALESCE               ASTKind = 398 // 空合并 $x ?? $y
	AST_ASSIGN_COALESCE        ASTKind = 399 // 空合并赋值 $x ??= $y
	AST_STATIC                 ASTKind = 400 // static $var = $init
	AST_WHILE                  ASTKind = 401 // while ($cond) $stmt
	AST_DO_WHILE               ASTKind = 402 // do $stmt while ($cond)
	AST_IF_ELEM                ASTKind = 403 // if/elseif元素
	AST_SWITCH_CASE            ASTKind = 404 // case/default元素
	AST_CATCH                  ASTKind = 405 // catch (Exception $e) {...}
	AST_PARAM                  ASTKind = 406 // 函数参数
	AST_TYPE_UNION             ASTKind = 407 // 联合类型 Type1|Type2
	AST_TYPE_INTERSECTION      ASTKind = 408 // 交集类型 Type1&Type2
	AST_ATTRIBUTE              ASTKind = 409 // 属性 #[Attribute]
	AST_MATCH_ARM              ASTKind = 410 // match分支
	AST_ENUM_CASE              ASTKind = 411 // 枚举案例
	AST_PROPERTY_HOOK          ASTKind = 412 // 属性钩子
)

// 表达式节点 - 3个子节点 (448-463)
const (
	AST_METHOD_CALL           ASTKind = 448 // 方法调用 $obj->method($args)
	AST_NULLSAFE_METHOD_CALL  ASTKind = 449 // 空安全方法调用 $obj?->method($args)
	AST_STATIC_CALL           ASTKind = 450 // 静态方法调用 Class::method($args)
	AST_CONDITIONAL           ASTKind = 451 // 三元运算符 $cond ? $true : $false
	AST_TRY                   ASTKind = 452 // try语句
	AST_FOREACH               ASTKind = 453 // foreach循环
	AST_DECLARE               ASTKind = 454 // declare语句
)

// 表达式节点 - 4个子节点 (512-517)
const (
	AST_FOR     ASTKind = 512 // for循环
	AST_SWITCH  ASTKind = 513 // switch语句
)

// 声明元素节点 (768-777)
const (
	AST_PROP_ELEM         ASTKind = 768 // 属性元素
	AST_CONST_ELEM        ASTKind = 769 // 常量元素
	AST_USE_TRAIT         ASTKind = 770 // trait使用
	AST_TRAIT_PRECEDENCE  ASTKind = 771 // trait优先级
	AST_METHOD_REFERENCE  ASTKind = 772 // 方法引用
	AST_NAMESPACE         ASTKind = 773 // 命名空间
	AST_USE_ELEM          ASTKind = 774 // use元素
	AST_TRAIT_ALIAS       ASTKind = 775 // trait别名
	AST_GROUP_USE         ASTKind = 776 // 分组use
	AST_CLASS_NAME        ASTKind = 777 // 类名
)

// getChildCount 根据AST节点类型返回期望的子节点数量
func (k ASTKind) getChildCount() int {
	switch {
	// 特殊节点
	case k <= 3:
		return -1 // 特殊处理
	
	// 声明节点 - 各有不同的子节点数
	case k >= 64 && k <= 73:
		switch k {
		case AST_CLOSURE:
			return 5 // name, params, uses, stmts, return_type
		case AST_METHOD:
			return 6 // flags, name, params, return_type, stmts, doc_comment
		case AST_CLASS:
			return 5 // flags, name, extends, implements, stmts
		case AST_ARROW_FUNC:
			return 4 // params, return_type, expr, static
		case AST_ENUM:
			return 5 // flags, name, type, implements, stmts
		}
		return -1
	
	// 列表节点 - 可变长度
	case k >= 128 && k <= 149:
		return -1 // 可变长度列表
	
	// 0个子节点
	case k >= 256 && k <= 257:
		return 0
	
	// 1个子节点
	case k >= 320 && k <= 351:
		return 1
	
	// 2个子节点  
	case k >= 384 && k <= 415:
		return 2
	
	// 3个子节点
	case k >= 448 && k <= 463:
		return 3
	
	// 4个子节点
	case k >= 512 && k <= 517:
		return 4
	
	// 声明元素节点 - 各有不同的子节点数
	case k >= 768 && k <= 777:
		switch k {
		case AST_PROP_ELEM:
			return 2 // name, default
		case AST_CONST_ELEM:
			return 2 // name, value
		case AST_USE_TRAIT:
			return 2 // name, adaptations
		case AST_TRAIT_PRECEDENCE:
			return 2 // method, insteadof
		case AST_METHOD_REFERENCE:
			return 2 // class, method
		case AST_NAMESPACE:
			return 2 // name, stmts
		case AST_USE_ELEM:
			return 2 // name, alias
		case AST_TRAIT_ALIAS:
			return 3 // method, alias, modifiers
		case AST_GROUP_USE:
			return 2 // prefix, uses
		case AST_CLASS_NAME:
			return 1 // name
		}
		return -1
	
	default:
		return -1 // 未知类型
	}
}

// String 返回AST节点类型的字符串表示
func (k ASTKind) String() string {
	switch k {
	// 特殊节点
	case AST_ZVAL:
		return "AST_ZVAL"
	case AST_CONSTANT:
		return "AST_CONSTANT"
	case AST_ZNODE:
		return "AST_ZNODE"
	case AST_FUNC_DECL:
		return "AST_FUNC_DECL"
	
	// 声明节点
	case AST_CLOSURE:
		return "AST_CLOSURE"
	case AST_METHOD:
		return "AST_METHOD"
	case AST_CLASS:
		return "AST_CLASS"
	case AST_ARROW_FUNC:
		return "AST_ARROW_FUNC"
	case AST_ENUM:
		return "AST_ENUM"
	
	// 列表节点
	case AST_ARG_LIST:
		return "AST_ARG_LIST"
	case AST_ARRAY:
		return "AST_ARRAY"
	case AST_ENCAPS_LIST:
		return "AST_ENCAPS_LIST"
	case AST_EXPR_LIST:
		return "AST_EXPR_LIST"
	case AST_STMT_LIST:
		return "AST_STMT_LIST"
	case AST_IF:
		return "AST_IF"
	case AST_SWITCH_LIST:
		return "AST_SWITCH_LIST"
	case AST_CATCH_LIST:
		return "AST_CATCH_LIST"
	case AST_PARAM_LIST:
		return "AST_PARAM_LIST"
	case AST_CLOSURE_USES:
		return "AST_CLOSURE_USES"
	case AST_PROP_GROUP:
		return "AST_PROP_GROUP"
	case AST_CONST_DECL:
		return "AST_CONST_DECL"
	case AST_CLASS_CONST_GROUP:
		return "AST_CLASS_CONST_GROUP"
	case AST_NAME_LIST:
		return "AST_NAME_LIST"
	case AST_TRAIT_ADAPTATIONS:
		return "AST_TRAIT_ADAPTATIONS"
	case AST_USE:
		return "AST_USE"
	case AST_ATTRIBUTE_GROUP:
		return "AST_ATTRIBUTE_GROUP"
	case AST_MATCH_ARM_LIST:
		return "AST_MATCH_ARM_LIST"
	case AST_ENUM_CASE_LIST:
		return "AST_ENUM_CASE_LIST"
	case AST_PROPERTY_HOOK_LIST:
		return "AST_PROPERTY_HOOK_LIST"
	
	// 0个子节点的表达式
	case AST_MAGIC_CONST:
		return "AST_MAGIC_CONST"
	case AST_TYPE:
		return "AST_TYPE"
	
	// 1个子节点的表达式
	case AST_VAR:
		return "AST_VAR"
	case AST_CONST:
		return "AST_CONST"
	case AST_UNPACK:
		return "AST_UNPACK"
	case AST_UNARY_PLUS:
		return "AST_UNARY_PLUS"
	case AST_UNARY_MINUS:
		return "AST_UNARY_MINUS"
	case AST_CAST:
		return "AST_CAST"
	case AST_EMPTY:
		return "AST_EMPTY"
	case AST_ISSET:
		return "AST_ISSET"
	case AST_SILENCE:
		return "AST_SILENCE"
	case AST_SHELL_EXEC:
		return "AST_SHELL_EXEC"
	case AST_CLONE:
		return "AST_CLONE"
	case AST_EXIT:
		return "AST_EXIT"
	case AST_PRINT:
		return "AST_PRINT"
	case AST_INCLUDE_OR_EVAL:
		return "AST_INCLUDE_OR_EVAL"
	case AST_UNARY_OP:
		return "AST_UNARY_OP"
	case AST_PRE_INC:
		return "AST_PRE_INC"
	case AST_PRE_DEC:
		return "AST_PRE_DEC"
	case AST_POST_INC:
		return "AST_POST_INC"
	case AST_POST_DEC:
		return "AST_POST_DEC"
	case AST_YIELD_FROM:
		return "AST_YIELD_FROM"
	case AST_GLOBAL:
		return "AST_GLOBAL"
	case AST_UNSET:
		return "AST_UNSET"
	case AST_RETURN:
		return "AST_RETURN"
	case AST_LABEL:
		return "AST_LABEL"
	case AST_REF:
		return "AST_REF"
	case AST_HALT_COMPILER:
		return "AST_HALT_COMPILER"
	case AST_ECHO:
		return "AST_ECHO"
	case AST_THROW:
		return "AST_THROW"
	case AST_GOTO:
		return "AST_GOTO"
	case AST_BREAK:
		return "AST_BREAK"
	case AST_CONTINUE:
		return "AST_CONTINUE"
	
	// 2个子节点的表达式
	case AST_DIM:
		return "AST_DIM"
	case AST_PROP:
		return "AST_PROP"
	case AST_NULLSAFE_PROP:
		return "AST_NULLSAFE_PROP"
	case AST_STATIC_PROP:
		return "AST_STATIC_PROP"
	case AST_CALL:
		return "AST_CALL"
	case AST_CLASS_CONST:
		return "AST_CLASS_CONST"
	case AST_ASSIGN:
		return "AST_ASSIGN"
	case AST_ASSIGN_REF:
		return "AST_ASSIGN_REF"
	case AST_ASSIGN_OP:
		return "AST_ASSIGN_OP"
	case AST_BINARY_OP:
		return "AST_BINARY_OP"
	case AST_ARRAY_ELEM:
		return "AST_ARRAY_ELEM"
	case AST_NEW:
		return "AST_NEW"
	case AST_INSTANCEOF:
		return "AST_INSTANCEOF"
	case AST_YIELD:
		return "AST_YIELD"
	case AST_COALESCE:
		return "AST_COALESCE"
	case AST_ASSIGN_COALESCE:
		return "AST_ASSIGN_COALESCE"
	case AST_STATIC:
		return "AST_STATIC"
	case AST_WHILE:
		return "AST_WHILE"
	case AST_DO_WHILE:
		return "AST_DO_WHILE"
	case AST_IF_ELEM:
		return "AST_IF_ELEM"
	case AST_SWITCH_CASE:
		return "AST_SWITCH_CASE"
	case AST_CATCH:
		return "AST_CATCH"
	case AST_PARAM:
		return "AST_PARAM"
	case AST_TYPE_UNION:
		return "AST_TYPE_UNION"
	case AST_TYPE_INTERSECTION:
		return "AST_TYPE_INTERSECTION"
	case AST_ATTRIBUTE:
		return "AST_ATTRIBUTE"
	case AST_MATCH_ARM:
		return "AST_MATCH_ARM"
	case AST_ENUM_CASE:
		return "AST_ENUM_CASE"
	case AST_PROPERTY_HOOK:
		return "AST_PROPERTY_HOOK"
	
	// 3个子节点的表达式
	case AST_METHOD_CALL:
		return "AST_METHOD_CALL"
	case AST_NULLSAFE_METHOD_CALL:
		return "AST_NULLSAFE_METHOD_CALL"
	case AST_STATIC_CALL:
		return "AST_STATIC_CALL"
	case AST_CONDITIONAL:
		return "AST_CONDITIONAL"
	case AST_TRY:
		return "AST_TRY"
	case AST_FOREACH:
		return "AST_FOREACH"
	case AST_DECLARE:
		return "AST_DECLARE"
	
	// 4个子节点的表达式
	case AST_FOR:
		return "AST_FOR"
	case AST_SWITCH:
		return "AST_SWITCH"
	
	// 声明元素节点
	case AST_PROP_ELEM:
		return "AST_PROP_ELEM"
	case AST_CONST_ELEM:
		return "AST_CONST_ELEM"
	case AST_USE_TRAIT:
		return "AST_USE_TRAIT"
	case AST_TRAIT_PRECEDENCE:
		return "AST_TRAIT_PRECEDENCE"
	case AST_METHOD_REFERENCE:
		return "AST_METHOD_REFERENCE"
	case AST_NAMESPACE:
		return "AST_NAMESPACE"
	case AST_USE_ELEM:
		return "AST_USE_ELEM"
	case AST_TRAIT_ALIAS:
		return "AST_TRAIT_ALIAS"
	case AST_GROUP_USE:
		return "AST_GROUP_USE"
	case AST_CLASS_NAME:
		return "AST_CLASS_NAME"
	
	default:
		return fmt.Sprintf("UNKNOWN_AST_KIND_%d", int(k))
	}
}

// IsSpecial 检查是否为特殊节点
func (k ASTKind) IsSpecial() bool {
	return k <= 3 || (k >= 64 && k <= 73)
}

// IsList 检查是否为列表节点
func (k ASTKind) IsList() bool {
	return k >= 128 && k <= 149
}

// IsExpression 检查是否为表达式节点
func (k ASTKind) IsExpression() bool {
	return (k >= 256 && k <= 257) || 
		   (k >= 320 && k <= 351) ||
		   (k >= 384 && k <= 415) ||
		   (k >= 448 && k <= 463) ||
		   (k >= 512 && k <= 517)
}

// IsStatement 检查是否为语句节点 
func (k ASTKind) IsStatement() bool {
	// 大部分语句节点在列表节点和表达式节点中
	switch k {
	case AST_STMT_LIST, AST_IF, AST_SWITCH_LIST, 
		 AST_WHILE, AST_DO_WHILE, AST_FOR, AST_FOREACH,
		 AST_TRY, AST_DECLARE, AST_RETURN, AST_BREAK,
		 AST_CONTINUE, AST_ECHO, AST_GLOBAL, AST_STATIC,
		 AST_UNSET, AST_GOTO, AST_LABEL:
		return true
	default:
		return false
	}
}

// IsDeclaration 检查是否为声明节点
func (k ASTKind) IsDeclaration() bool {
	return (k >= 64 && k <= 73) || k == AST_FUNC_DECL ||
		   (k >= 768 && k <= 777) || k == AST_CONST_DECL ||
		   k == AST_PROP_GROUP || k == AST_CLASS_CONST_GROUP
}