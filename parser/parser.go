package parser

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/wudi/php-parser/ast"
	"github.com/wudi/php-parser/lexer"
	"github.com/wudi/php-parser/errors"
)

// Parser 递归下降解析器，基于PHP官方语法规则
type Parser struct {
	lexer        *lexer.Lexer
	currentToken lexer.Token
	peekToken    lexer.Token
	errors       []error
}

// New 创建新的解析器实例
func New(l *lexer.Lexer) *Parser {
	p := &Parser{
		lexer:  l,
		errors: []error{},
	}
	
	// 读取两个token，初始化currentToken和peekToken
	p.nextToken()
	p.nextToken()
	
	return p
}

// nextToken 推进token流
func (p *Parser) nextToken() {
	p.currentToken = p.peekToken
	p.peekToken = p.lexer.NextToken()
}

// Errors 返回解析错误列表
func (p *Parser) Errors() []error {
	return p.errors
}

// addError 添加解析错误
func (p *Parser) addError(msg string) {
	pos := lexer.Position{
		Line:   p.currentToken.Position.Line,
		Column: p.currentToken.Position.Column,
		Offset: p.currentToken.Position.Offset,
	}
	p.errors = append(p.errors, 
		errors.NewSyntaxError(msg, pos))
}

// Parse 解析入口点 - 对应 start 规则
func (p *Parser) Parse() ast.Node {
	return p.parseTopStatementList()
}

// parseTopStatementList 对应 top_statement_list 规则
func (p *Parser) parseTopStatementList() ast.Node {
	statements := []ast.Node{}
	
	for p.currentToken.Type != lexer.T_EOF {
		stmt := p.parseTopStatement()
		if stmt != nil {
			statements = append(statements, stmt)
		}
		
		// 如果解析出错，尝试恢复
		if p.currentToken.Type == lexer.T_UNKNOWN {
			p.nextToken()
			continue
		}
	}
	
	return ast.NewStatementList(statements)
}

// parseTopStatement 对应 top_statement 规则
func (p *Parser) parseTopStatement() ast.Node {
	switch p.currentToken.Type {
	// T_HALT_COMPILER '(' ')' ';'
	case lexer.T_HALT_COMPILER:
		return p.parseHaltCompiler()
		
	// T_NAMESPACE namespace_declaration_name ';' | '{' ... '}'  
	case lexer.T_NAMESPACE:
		return p.parseNamespace()
		
	// T_USE declarations
	case lexer.T_USE:
		return p.parseUse()
		
	// T_CONST const_list ';'
	case lexer.T_CONST:
		return p.parseConstDeclaration()
		
	// attributes
	case lexer.T_ATTRIBUTE:
		return p.parseAttributedTopStatement()
		
	// attributed_top_statement (function, class, etc.)
	case lexer.T_FUNCTION, lexer.T_CLASS, lexer.T_INTERFACE, 
		 lexer.T_TRAIT, lexer.T_ENUM, lexer.T_ABSTRACT, lexer.T_FINAL:
		return p.parseAttributedTopStatement()
		
	default:
		// 普通语句
		return p.parseStatement()
	}
}

// parseStatement 对应 statement 规则
func (p *Parser) parseStatement() ast.Node {
	switch p.currentToken.Type {
	case lexer.TOKEN_LBRACE: // '{'
		return p.parseBlock()
		
	case lexer.T_IF:
		return p.parseIf()
		
	case lexer.T_WHILE:
		return p.parseWhile()
		
	case lexer.T_DO:
		return p.parseDoWhile()
		
	case lexer.T_FOR:
		return p.parseFor()
		
	case lexer.T_FOREACH:
		return p.parseForeach()
		
	case lexer.T_SWITCH:
		return p.parseSwitch()
		
	case lexer.T_TRY:
		return p.parseTry()
		
	case lexer.T_BREAK:
		return p.parseBreak()
		
	case lexer.T_CONTINUE:
		return p.parseContinue()
		
	case lexer.T_RETURN:
		return p.parseReturn()
		
	case lexer.T_ECHO:
		return p.parseEcho()
		
	case lexer.T_GLOBAL:
		return p.parseGlobal()
		
	case lexer.T_STATIC:
		return p.parseStatic()
		
	case lexer.T_UNSET:
		return p.parseUnset()
		
	case lexer.T_GOTO:
		return p.parseGoto()
		
	case lexer.T_DECLARE:
		return p.parseDeclare()
		
	case lexer.T_STRING:
		// 可能是标签 T_STRING ':'
		if p.peekToken.Type == lexer.TOKEN_COLON {
			return p.parseLabel()
		}
		fallthrough
		
	case lexer.TOKEN_SEMICOLON:
		// 空语句
		p.nextToken()
		return nil
		
	default:
		// 表达式语句
		expr := p.parseExpression()
		p.expectToken(lexer.TOKEN_SEMICOLON)
		return expr
	}
}

// parseExpression 表达式解析入口
func (p *Parser) parseExpression() ast.Node {
	return p.parseExpressionPrecedence(LOWEST)
}

// Precedence 表达式优先级，严格按照PHP官方规则
type Precedence int

const (
	LOWEST Precedence = iota
	THROW              // throw
	ARROW_FUNCTION     // fn =>
	INCLUDE_REQUIRE    // include, include_once, require, require_once
	LOGICAL_OR_LOW     // or
	LOGICAL_XOR_LOW    // xor  
	LOGICAL_AND_LOW    // and
	PRINT              // print
	YIELD              // yield, yield from
	ASSIGNMENT         // = += -= etc.
	TERNARY           // ? :
	COALESCE          // ??
	LOGICAL_OR        // ||
	LOGICAL_AND       // &&
	BITWISE_OR        // |
	BITWISE_XOR       // ^
	BITWISE_AND       // &
	EQUALITY          // == != === !==
	RELATIONAL        // < <= > >= <=>
	PIPE              // |> (PHP 8.4)
	CONCAT            // .
	SHIFT             // << >>
	ADDITIVE          // + -
	MULTIPLICATIVE    // * / %
	LOGICAL_NOT       // !
	INSTANCEOF        // instanceof
	UNARY             // ~ casts @
	EXPONENT          // **
	CLONE             // clone
)

// precedenceMap 操作符优先级映射表
var precedenceMap = map[lexer.TokenType]Precedence{
	// throw
	lexer.T_THROW: THROW,
	
	// include/require
	lexer.T_INCLUDE:      INCLUDE_REQUIRE,
	lexer.T_INCLUDE_ONCE: INCLUDE_REQUIRE,
	lexer.T_REQUIRE:      INCLUDE_REQUIRE,
	lexer.T_REQUIRE_ONCE: INCLUDE_REQUIRE,
	
	// logical operators (low precedence)
	lexer.T_LOGICAL_OR:  LOGICAL_OR_LOW,
	lexer.T_LOGICAL_XOR: LOGICAL_XOR_LOW,
	lexer.T_LOGICAL_AND: LOGICAL_AND_LOW,
	
	// print
	lexer.T_PRINT: PRINT,
	
	// yield
	lexer.T_YIELD:      YIELD,
	lexer.T_YIELD_FROM: YIELD,
	
	// assignment operators
	lexer.TOKEN_EQUAL:        ASSIGNMENT,
	lexer.T_PLUS_EQUAL:       ASSIGNMENT,
	lexer.T_MINUS_EQUAL:      ASSIGNMENT,
	lexer.T_MUL_EQUAL:        ASSIGNMENT,
	lexer.T_DIV_EQUAL:        ASSIGNMENT,
	lexer.T_CONCAT_EQUAL:     ASSIGNMENT,
	lexer.T_MOD_EQUAL:        ASSIGNMENT,
	lexer.T_AND_EQUAL:        ASSIGNMENT,
	lexer.T_OR_EQUAL:         ASSIGNMENT,
	lexer.T_XOR_EQUAL:        ASSIGNMENT,
	lexer.T_SL_EQUAL:         ASSIGNMENT,
	lexer.T_SR_EQUAL:         ASSIGNMENT,
	lexer.T_POW_EQUAL:        ASSIGNMENT,
	lexer.T_COALESCE_EQUAL:   ASSIGNMENT,
	
	// ternary
	lexer.TOKEN_QUESTION: TERNARY,
	
	// coalesce
	lexer.T_COALESCE: COALESCE,
	
	// logical operators (high precedence)
	lexer.T_BOOLEAN_OR:  LOGICAL_OR,
	lexer.T_BOOLEAN_AND: LOGICAL_AND,
	
	// bitwise operators
	lexer.TOKEN_PIPE:      BITWISE_OR,
	lexer.TOKEN_CARET:     BITWISE_XOR,
	lexer.TOKEN_AMPERSAND: BITWISE_AND,
	
	// equality operators
	lexer.T_IS_EQUAL:         EQUALITY,
	lexer.T_IS_NOT_EQUAL:     EQUALITY,
	lexer.T_IS_IDENTICAL:     EQUALITY,
	lexer.T_IS_NOT_IDENTICAL: EQUALITY,
	lexer.T_SPACESHIP:        EQUALITY,
	
	// relational operators
	lexer.TOKEN_LT:               RELATIONAL,
	lexer.TOKEN_GT:               RELATIONAL,
	lexer.T_IS_SMALLER_OR_EQUAL:  RELATIONAL,
	lexer.T_IS_GREATER_OR_EQUAL:  RELATIONAL,
	
	// instanceof
	lexer.T_INSTANCEOF: INSTANCEOF,
	
	// pipe operator (PHP 8.4)
	lexer.T_PIPE: PIPE,
	
	// string concatenation
	lexer.TOKEN_DOT: CONCAT,
	
	// shift operators
	lexer.T_SL: SHIFT,
	lexer.T_SR: SHIFT,
	
	// additive operators
	lexer.TOKEN_PLUS:  ADDITIVE,
	lexer.TOKEN_MINUS: ADDITIVE,
	
	// multiplicative operators
	lexer.TOKEN_MULTIPLY: MULTIPLICATIVE,
	lexer.TOKEN_DIVIDE:   MULTIPLICATIVE,
	lexer.TOKEN_MODULO:   MULTIPLICATIVE,
	
	// exponentiation
	lexer.T_POW: EXPONENT,
}

// parseExpressionPrecedence Pratt解析器核心
func (p *Parser) parseExpressionPrecedence(precedence Precedence) ast.Node {
	// 解析前缀表达式
	left := p.parsePrefixExpression()
	if left == nil {
		return nil
	}
	
	// 解析中缀表达式
	for precedence < p.peekPrecedence() {
		left = p.parseInfixExpression(left)
		if left == nil {
			break
		}
	}
	
	return left
}

// parsePrefixExpression 解析前缀表达式
func (p *Parser) parsePrefixExpression() ast.Node {
	switch p.currentToken.Type {
	case lexer.T_VARIABLE:
		return p.parseVariable()
		
	case lexer.T_LNUMBER:
		return p.parseIntegerLiteral()
		
	case lexer.T_DNUMBER:
		return p.parseFloatLiteral()
		
	case lexer.T_CONSTANT_ENCAPSED_STRING:
		return p.parseStringLiteral()
		
	case lexer.T_STRING:
		// 检查是否为true/false/null关键字
		if strings.ToLower(p.currentToken.Value) == "true" || 
		   strings.ToLower(p.currentToken.Value) == "false" {
			return p.parseBooleanLiteral()
		}
		if strings.ToLower(p.currentToken.Value) == "null" {
			return p.parseNullLiteral()
		}
		return p.parseIdentifier()
		
	case lexer.TOKEN_LPAREN:
		return p.parseGroupedExpression()
		
	case lexer.TOKEN_MINUS, lexer.TOKEN_PLUS:
		return p.parseUnaryExpression()
		
	case lexer.TOKEN_EXCLAMATION, lexer.TOKEN_TILDE:
		return p.parseUnaryExpression()
		
	case lexer.T_INC, lexer.T_DEC:
		return p.parsePreIncrementDecrement()
		
	case lexer.T_CLONE:
		return p.parseCloneExpression()
		
	case lexer.T_NEW:
		return p.parseNewExpression()
		
	case lexer.T_ARRAY:
		return p.parseArrayExpression()
		
	case lexer.TOKEN_LBRACKET:
		return p.parseArrayLiteral()
		
	case lexer.T_FUNCTION:
		return p.parseAnonymousFunction()
		
	case lexer.T_FN:
		return p.parseArrowFunction()
		
	case lexer.T_MATCH:
		return p.parseMatchExpression()
		
	case lexer.T_THROW:
		return p.parseThrowExpression()
		
	case lexer.T_YIELD:
		return p.parseYieldExpression()
		
	case lexer.T_YIELD_FROM:
		return p.parseYieldFromExpression()
		
	case lexer.T_INCLUDE, lexer.T_INCLUDE_ONCE, lexer.T_REQUIRE, lexer.T_REQUIRE_ONCE:
		return p.parseIncludeExpression()
		
	case lexer.T_ISSET:
		return p.parseIssetExpression()
		
	case lexer.T_EMPTY:
		return p.parseEmptyExpression()
		
	case lexer.T_EVAL:
		return p.parseEvalExpression()
		
	case lexer.T_EXIT:
		return p.parseExitExpression()
		
	case lexer.T_PRINT:
		return p.parsePrintExpression()
		
	case lexer.T_LIST:
		return p.parseListExpression()
		
	// 类型转换
	case lexer.T_INT_CAST, lexer.T_DOUBLE_CAST, lexer.T_STRING_CAST,
		 lexer.T_ARRAY_CAST, lexer.T_OBJECT_CAST, lexer.T_BOOL_CAST, lexer.T_UNSET_CAST:
		return p.parseCastExpression()
		
	// @ 错误抑制
	case lexer.TOKEN_AT:
		return p.parseErrorSuppressionExpression()
		
	// 魔术常量
	case lexer.T_LINE, lexer.T_FILE, lexer.T_DIR, lexer.T_CLASS_C,
		 lexer.T_TRAIT_C, lexer.T_METHOD_C, lexer.T_FUNC_C, lexer.T_NS_C:
		return p.parseMagicConstant()
		
	// heredoc/nowdoc
	case lexer.T_START_HEREDOC:
		return p.parseHeredocExpression()
		
		
	default:
		p.addError(fmt.Sprintf("no prefix parse function for %s found", p.currentToken.Type))
		return nil
	}
}

// parseInfixExpression 解析中缀表达式
func (p *Parser) parseInfixExpression(left ast.Node) ast.Node {
	switch p.currentToken.Type {
	// 二元运算符
	case lexer.TOKEN_PLUS, lexer.TOKEN_MINUS, lexer.TOKEN_MULTIPLY,
		 lexer.TOKEN_DIVIDE, lexer.TOKEN_MODULO, lexer.T_POW:
		return p.parseBinaryExpression(left)
		
	case lexer.TOKEN_DOT:
		return p.parseConcatExpression(left)
		
	case lexer.TOKEN_LT, lexer.TOKEN_GT, lexer.T_IS_SMALLER_OR_EQUAL,
		 lexer.T_IS_GREATER_OR_EQUAL, lexer.T_SPACESHIP:
		return p.parseComparisonExpression(left)
		
	case lexer.T_IS_EQUAL, lexer.T_IS_NOT_EQUAL,
		 lexer.T_IS_IDENTICAL, lexer.T_IS_NOT_IDENTICAL:
		return p.parseEqualityExpression(left)
		
	case lexer.TOKEN_AMPERSAND, lexer.TOKEN_PIPE, lexer.TOKEN_CARET,
		 lexer.T_SL, lexer.T_SR:
		return p.parseBitwiseExpression(left)
		
	case lexer.T_BOOLEAN_AND, lexer.T_BOOLEAN_OR,
		 lexer.T_LOGICAL_AND, lexer.T_LOGICAL_OR, lexer.T_LOGICAL_XOR:
		return p.parseLogicalExpression(left)
		
	// 赋值运算符
	case lexer.TOKEN_EQUAL:
		return p.parseAssignmentExpression(left)
		
	case lexer.T_PLUS_EQUAL, lexer.T_MINUS_EQUAL, lexer.T_MUL_EQUAL,
		 lexer.T_DIV_EQUAL, lexer.T_CONCAT_EQUAL, lexer.T_MOD_EQUAL,
		 lexer.T_AND_EQUAL, lexer.T_OR_EQUAL, lexer.T_XOR_EQUAL,
		 lexer.T_SL_EQUAL, lexer.T_SR_EQUAL, lexer.T_POW_EQUAL:
		return p.parseCompoundAssignmentExpression(left)
		
	case lexer.T_COALESCE_EQUAL:
		return p.parseCoalesceAssignmentExpression(left)
		
	// 三元运算符
	case lexer.TOKEN_QUESTION:
		return p.parseTernaryExpression(left)
		
	// 空合并运算符
	case lexer.T_COALESCE:
		return p.parseCoalesceExpression(left)
		
	// instanceof
	case lexer.T_INSTANCEOF:
		return p.parseInstanceofExpression(left)
		
	// 管道运算符 (PHP 8.4)
	case lexer.T_PIPE:
		return p.parsePipeExpression(left)
		
	// 函数/方法调用
	case lexer.TOKEN_LPAREN:
		return p.parseCallExpression(left)
		
	// 属性访问
	case lexer.T_OBJECT_OPERATOR:
		return p.parsePropertyAccessExpression(left)
		
	case lexer.T_NULLSAFE_OBJECT_OPERATOR:
		return p.parseNullsafePropertyAccessExpression(left)
		
	// 静态访问
	case lexer.T_PAAMAYIM_NEKUDOTAYIM:
		return p.parseStaticAccessExpression(left)
		
	// 数组访问
	case lexer.TOKEN_LBRACKET:
		return p.parseArrayAccessExpression(left)
		
	// 后缀自增/自减
	case lexer.T_INC, lexer.T_DEC:
		return p.parsePostIncrementDecrement(left)
		
	default:
		p.addError(fmt.Sprintf("no infix parse function for %s found", p.currentToken.Type))
		return left
	}
}

// peekPrecedence 获取下一个token的优先级
func (p *Parser) peekPrecedence() Precedence {
	if precedence, ok := precedenceMap[p.peekToken.Type]; ok {
		return precedence
	}
	return LOWEST
}

// currentPrecedence 获取当前token的优先级
func (p *Parser) currentPrecedence() Precedence {
	if precedence, ok := precedenceMap[p.currentToken.Type]; ok {
		return precedence
	}
	return LOWEST
}

// expectToken 期望特定token类型
func (p *Parser) expectToken(tokenType lexer.TokenType) bool {
	if p.peekToken.Type == tokenType {
		p.nextToken()
		return true
	}
	
	p.addError(fmt.Sprintf("expected %s, got %s", tokenType, p.peekToken.Type))
	return false
}

// parseVariable 解析变量
func (p *Parser) parseVariable() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	// 变量名去掉$前缀
	name := token.Value[1:] 
	// 转换Position类型
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewVariable(name, pos)
}

// parseIntegerLiteral 解析整数字面量
func (p *Parser) parseIntegerLiteral() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	value, err := strconv.ParseInt(token.Value, 0, 64)
	if err != nil {
		p.addError(fmt.Sprintf("could not parse %q as integer", token.Value))
		return nil
	}
	
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewIntegerLiteral(value, pos)
}

// parseFloatLiteral 解析浮点数字面量
func (p *Parser) parseFloatLiteral() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	value, err := strconv.ParseFloat(token.Value, 64)
	if err != nil {
		p.addError(fmt.Sprintf("could not parse %q as float", token.Value))
		return nil
	}
	
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewFloatLiteral(value, pos)
}

// parseStringLiteral 解析字符串字面量
func (p *Parser) parseStringLiteral() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	// 去掉引号
	value := token.Value[1 : len(token.Value)-1]
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewStringLiteral(value, pos)
}

// parseIdentifier 解析标识符
func (p *Parser) parseIdentifier() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewIdentifier(token.Value, pos)
}

// parseBooleanLiteral 解析布尔字面量  
func (p *Parser) parseBooleanLiteral() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	value := strings.ToLower(token.Value) == "true"
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewBooleanLiteral(value, pos)
}

// parseNullLiteral 解析null字面量
func (p *Parser) parseNullLiteral() ast.Node {
	token := p.currentToken
	p.nextToken()
	
	pos := ast.Position{
		Line:   token.Position.Line,
		Column: token.Position.Column,
		Offset: token.Position.Offset,
	}
	return ast.NewNullLiteral(pos)
}

// 占位函数，具体实现需要根据AST结构来完成
func (p *Parser) parseHaltCompiler() ast.Node { panic("not implemented") }
func (p *Parser) parseNamespace() ast.Node { panic("not implemented") }
func (p *Parser) parseUse() ast.Node { panic("not implemented") }
func (p *Parser) parseConstDeclaration() ast.Node { panic("not implemented") }
func (p *Parser) parseAttributedTopStatement() ast.Node { panic("not implemented") }
func (p *Parser) parseBlock() ast.Node { panic("not implemented") }
func (p *Parser) parseIf() ast.Node { panic("not implemented") }
func (p *Parser) parseWhile() ast.Node { panic("not implemented") }
func (p *Parser) parseDoWhile() ast.Node { panic("not implemented") }
func (p *Parser) parseFor() ast.Node { panic("not implemented") }
func (p *Parser) parseForeach() ast.Node { panic("not implemented") }
func (p *Parser) parseSwitch() ast.Node { panic("not implemented") }
func (p *Parser) parseTry() ast.Node { panic("not implemented") }
func (p *Parser) parseBreak() ast.Node { panic("not implemented") }
func (p *Parser) parseContinue() ast.Node { panic("not implemented") }
func (p *Parser) parseReturn() ast.Node { panic("not implemented") }
func (p *Parser) parseEcho() ast.Node { panic("not implemented") }
func (p *Parser) parseGlobal() ast.Node { panic("not implemented") }
func (p *Parser) parseStatic() ast.Node { panic("not implemented") }
func (p *Parser) parseUnset() ast.Node { panic("not implemented") }
func (p *Parser) parseGoto() ast.Node { panic("not implemented") }
func (p *Parser) parseDeclare() ast.Node { panic("not implemented") }
func (p *Parser) parseLabel() ast.Node { panic("not implemented") }
func (p *Parser) parseGroupedExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseUnaryExpression() ast.Node { panic("not implemented") }
func (p *Parser) parsePreIncrementDecrement() ast.Node { panic("not implemented") }
func (p *Parser) parseCloneExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseNewExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseArrayExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseArrayLiteral() ast.Node { panic("not implemented") }
func (p *Parser) parseAnonymousFunction() ast.Node { panic("not implemented") }
func (p *Parser) parseArrowFunction() ast.Node { panic("not implemented") }
func (p *Parser) parseMatchExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseThrowExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseYieldExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseYieldFromExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseIncludeExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseIssetExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseEmptyExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseEvalExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseExitExpression() ast.Node { panic("not implemented") }
func (p *Parser) parsePrintExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseListExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseCastExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseErrorSuppressionExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseMagicConstant() ast.Node { panic("not implemented") }
func (p *Parser) parseHeredocExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseBacktickExpression() ast.Node { panic("not implemented") }
func (p *Parser) parseBinaryExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseConcatExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseComparisonExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseEqualityExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseBitwiseExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseLogicalExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseAssignmentExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseCompoundAssignmentExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseCoalesceAssignmentExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseTernaryExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseCoalesceExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseInstanceofExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parsePipeExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseCallExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parsePropertyAccessExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseNullsafePropertyAccessExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseStaticAccessExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parseArrayAccessExpression(left ast.Node) ast.Node { panic("not implemented") }
func (p *Parser) parsePostIncrementDecrement(left ast.Node) ast.Node { panic("not implemented") }