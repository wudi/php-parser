package ast

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestASTKind(t *testing.T) {
	tests := []struct {
		kind         ASTKind
		name         string
		childCount   int
		isSpecial    bool
		isList       bool
		isExpression bool
		isStatement  bool
	}{
		{AST_ZVAL, "AST_ZVAL", -1, true, false, false, false},
		{AST_CONSTANT, "AST_CONSTANT", -1, true, false, false, false},
		{AST_CLOSURE, "AST_CLOSURE", 5, true, false, false, false},
		{AST_STMT_LIST, "AST_STMT_LIST", -1, false, true, false, true},
		{AST_VAR, "AST_VAR", 1, false, false, true, false},
		{AST_BINARY_OP, "AST_BINARY_OP", 2, false, false, true, false},
		{AST_METHOD_CALL, "AST_METHOD_CALL", 3, false, false, true, false},
		{AST_FOR, "AST_FOR", 4, false, false, true, true},
	}
	
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if test.kind.String() != test.name {
				t.Errorf("Expected name %s, got %s", test.name, test.kind.String())
			}
			
			if test.kind.getChildCount() != test.childCount {
				t.Errorf("Expected child count %d, got %d", test.childCount, test.kind.getChildCount())
			}
			
			if test.kind.IsSpecial() != test.isSpecial {
				t.Errorf("Expected IsSpecial %v, got %v", test.isSpecial, test.kind.IsSpecial())
			}
			
			if test.kind.IsList() != test.isList {
				t.Errorf("Expected IsList %v, got %v", test.isList, test.kind.IsList())
			}
			
			if test.kind.IsExpression() != test.isExpression {
				t.Errorf("Expected IsExpression %v, got %v", test.isExpression, test.kind.IsExpression())
			}
			
			if test.kind.IsStatement() != test.isStatement {
				t.Errorf("Expected IsStatement %v, got %v", test.isStatement, test.kind.IsStatement())
			}
		})
	}
}

func TestPosition(t *testing.T) {
	pos := Position{Line: 10, Column: 5, Offset: 100}
	expected := "10:5"
	
	if pos.String() != expected {
		t.Errorf("Expected position string %s, got %s", expected, pos.String())
	}
}

func TestZvalNode(t *testing.T) {
	tests := []struct {
		name     string
		value    interface{}
		expected string
	}{
		{"integer", int64(42), "Zval(42)@1:1"},
		{"float", 3.14, "Zval(3.14)@1:1"},
		{"string", "hello", "Zval(hello)@1:1"},
		{"boolean", true, "Zval(true)@1:1"},
		{"null", nil, "Zval(<nil>)@1:1"},
	}
	
	pos := Position{Line: 1, Column: 1}
	
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			node := NewZvalNode(test.value, pos)
			
			if node.GetKind() != AST_ZVAL {
				t.Errorf("Expected kind AST_ZVAL, got %s", node.GetKind())
			}
			
			if node.Value != test.value {
				t.Errorf("Expected value %v, got %v", test.value, node.Value)
			}
			
			if node.String() != test.expected {
				t.Errorf("Expected string %s, got %s", test.expected, node.String())
			}
		})
	}
}

func TestVariableNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	
	// 简单变量
	varNode := NewVariable("test", pos)
	
	if varNode.GetKind() != AST_VAR {
		t.Errorf("Expected kind AST_VAR, got %s", varNode.GetKind())
	}
	
	children := varNode.GetChildren()
	if len(children) != 1 {
		t.Errorf("Expected 1 child, got %d", len(children))
	}
	
	nameNode, ok := children[0].(*ZvalNode)
	if !ok {
		t.Errorf("Expected child to be ZvalNode")
	}
	
	if nameNode.Value != "test" {
		t.Errorf("Expected variable name 'test', got %v", nameNode.Value)
	}
}

func TestBinaryNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	left := NewIntegerLiteral(1, pos)
	right := NewIntegerLiteral(2, pos)
	
	node := NewBinaryNode(AST_BINARY_OP, "+", left, right, pos)
	
	if node.GetKind() != AST_BINARY_OP {
		t.Errorf("Expected kind AST_BINARY_OP, got %s", node.GetKind())
	}
	
	if node.Operator != "+" {
		t.Errorf("Expected operator '+', got %s", node.Operator)
	}
	
	if node.Left != left {
		t.Errorf("Expected left operand to match")
	}
	
	if node.Right != right {
		t.Errorf("Expected right operand to match")
	}
	
	children := node.GetChildren()
	if len(children) != 2 {
		t.Errorf("Expected 2 children, got %d", len(children))
	}
	
	expected := "AST_BINARY_OP(+)@1:1"
	if node.String() != expected {
		t.Errorf("Expected string %s, got %s", expected, node.String())
	}
}

func TestAssignNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	left := NewVariable("x", pos)
	right := NewIntegerLiteral(42, pos)
	
	// 简单赋值
	assign := NewAssignNode(left, right, pos)
	
	if assign.GetKind() != AST_ASSIGN {
		t.Errorf("Expected kind AST_ASSIGN, got %s", assign.GetKind())
	}
	
	if assign.Left != left || assign.Right != right {
		t.Errorf("Expected left and right operands to match")
	}
	
	// 复合赋值
	compoundAssign := NewCompoundAssignNode("+=", left, right, pos)
	
	if compoundAssign.GetKind() != AST_ASSIGN_OP {
		t.Errorf("Expected kind AST_ASSIGN_OP, got %s", compoundAssign.GetKind())
	}
	
	if compoundAssign.Op != "+=" {
		t.Errorf("Expected operator '+=', got %s", compoundAssign.Op)
	}
}

func TestCallNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	callee := NewIdentifier("strlen", pos)
	args := []Node{
		NewStringLiteral("hello", pos),
	}
	
	node := NewCallNode(callee, args, pos)
	
	if node.GetKind() != AST_CALL {
		t.Errorf("Expected kind AST_CALL, got %s", node.GetKind())
	}
	
	if len(node.Arguments) != 1 {
		t.Errorf("Expected 1 argument, got %d", len(node.Arguments))
	}
	
	children := node.GetChildren()
	if len(children) != 2 { // callee + argument list
		t.Errorf("Expected 2 children, got %d", len(children))
	}
	
	expected := "Call(1 args)@1:1"
	if node.String() != expected {
		t.Errorf("Expected string %s, got %s", expected, node.String())
	}
}

func TestMethodCallNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	object := NewVariable("obj", pos)
	method := NewIdentifier("test", pos)
	args := []Node{NewIntegerLiteral(1, pos)}
	
	// 普通方法调用
	call := NewMethodCallNode(object, method, args, false, pos)
	
	if call.GetKind() != AST_METHOD_CALL {
		t.Errorf("Expected kind AST_METHOD_CALL, got %s", call.GetKind())
	}
	
	if call.Nullsafe {
		t.Errorf("Expected Nullsafe to be false")
	}
	
	// 空安全方法调用
	nullsafeCall := NewMethodCallNode(object, method, args, true, pos)
	
	if nullsafeCall.GetKind() != AST_NULLSAFE_METHOD_CALL {
		t.Errorf("Expected kind AST_NULLSAFE_METHOD_CALL, got %s", nullsafeCall.GetKind())
	}
	
	if !nullsafeCall.Nullsafe {
		t.Errorf("Expected Nullsafe to be true")
	}
	
	expected := "MethodCall(?->, 1 args)@1:1"
	if nullsafeCall.String() != expected {
		t.Errorf("Expected string %s, got %s", expected, nullsafeCall.String())
	}
}

func TestListNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	elements := []Node{
		NewIntegerLiteral(1, pos),
		NewIntegerLiteral(2, pos),
		NewIntegerLiteral(3, pos),
	}
	
	list := NewListNode(AST_EXPR_LIST, elements, pos)
	
	if list.GetKind() != AST_EXPR_LIST {
		t.Errorf("Expected kind AST_EXPR_LIST, got %s", list.GetKind())
	}
	
	if len(list.Elements) != 3 {
		t.Errorf("Expected 3 elements, got %d", len(list.Elements))
	}
	
	children := list.GetChildren()
	if len(children) != 3 {
		t.Errorf("Expected 3 children, got %d", len(children))
	}
	
	expected := "AST_EXPR_LIST[3]@1:1"
	if list.String() != expected {
		t.Errorf("Expected string %s, got %s", expected, list.String())
	}
}

func TestIfNode(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	
	// 创建if条件和body
	condition := NewVariable("x", pos)
	body := NewStatementList([]Node{})
	ifElem := NewIfElementNode(condition, body, pos)
	
	// 创建else body
	elseBody := NewStatementList([]Node{})
	elseElem := NewIfElementNode(nil, elseBody, pos)
	
	ifNode := NewIfNode([]Node{ifElem, elseElem}, pos)
	
	if ifNode.GetKind() != AST_IF {
		t.Errorf("Expected kind AST_IF, got %s", ifNode.GetKind())
	}
	
	if len(ifNode.Elements) != 2 {
		t.Errorf("Expected 2 elements (if + else), got %d", len(ifNode.Elements))
	}
	
	// 检查if元素
	if ifElem.Condition == nil {
		t.Errorf("Expected if element to have condition")
	}
	
	// 检查else元素
	if elseElem.Condition != nil {
		t.Errorf("Expected else element to have nil condition")
	}
	
	expected := "If(2 branches)@1:1"
	if ifNode.String() != expected {
		t.Errorf("Expected string %s, got %s", expected, ifNode.String())
	}
}

func TestJSONSerialization(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	
	// 创建一个简单的AST结构
	left := NewVariable("x", pos)
	right := NewIntegerLiteral(42, pos)
	assign := NewAssignNode(left, right, pos)
	
	// 序列化为JSON
	jsonData, err := json.Marshal(assign)
	if err != nil {
		t.Fatalf("Failed to marshal to JSON: %v", err)
	}
	
	// 检查JSON包含期望的字段
	jsonStr := string(jsonData)
	expectedFields := []string{
		`"kind":"AST_ASSIGN"`,
		`"position":{"line":1,"column":1,"offset":0}`,
	}
	
	for _, field := range expectedFields {
		if !contains(jsonStr, field) {
			t.Errorf("Expected JSON to contain %s, got %s", field, jsonStr)
		}
	}
}

func TestWalk(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	
	// 创建一个小的AST结构
	left := NewVariable("x", pos)
	right := NewIntegerLiteral(42, pos)
	assign := NewAssignNode(left, right, pos)
	
	// 统计遍历的节点数
	count := 0
	Walk(assign, func(node Node) {
		count++
	})
	
	// 应该有5个节点: assign, left, left.name, right, children
	expected := 4 // assign + variable + variable.name + integer
	if count != expected {
		t.Errorf("Expected to visit %d nodes, visited %d", expected, count)
	}
}

func TestPrint(t *testing.T) {
	pos := Position{Line: 1, Column: 1}
	
	left := NewVariable("x", pos)
	right := NewIntegerLiteral(42, pos)
	assign := NewAssignNode(left, right, pos)
	
	output := Print(assign, "")
	
	// 检查输出包含基本信息
	expectedParts := []string{
		"Assign(=)@1:1",
		"Variable@1:1",
		"Zval(42)@1:1",
	}
	
	for _, part := range expectedParts {
		if !contains(output, part) {
			t.Errorf("Expected output to contain %s, got:\n%s", part, output)
		}
	}
}

// 辅助函数
func contains(s, substr string) bool {
	return strings.Contains(s, substr)
}