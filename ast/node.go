package ast

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Position 表示源码中的位置信息
type Position struct {
	Line   int `json:"line"`   // 行号，从1开始
	Column int `json:"column"` // 列号，从1开始
	Offset int `json:"offset"` // 字符偏移量，从0开始
}

// String 返回位置的字符串表示
func (p Position) String() string {
	return fmt.Sprintf("%d:%d", p.Line, p.Column)
}

// Node 所有AST节点的基础接口
type Node interface {
	// GetKind 返回节点类型
	GetKind() ASTKind
	
	// GetPosition 返回节点在源码中的位置
	GetPosition() Position
	
	// GetChildren 返回子节点列表
	GetChildren() []Node
	
	// Accept 访问者模式
	Accept(visitor Visitor) interface{}
	
	// String 返回节点的字符串表示
	String() string
}

// Visitor 访问者接口
type Visitor interface {
	Visit(node Node) interface{}
}

// BaseNode 基础节点结构，实现Node接口的通用部分
type BaseNode struct {
	Kind     ASTKind  `json:"kind"`
	Position Position `json:"position"`
	Children []Node   `json:"children,omitempty"`
}

// GetKind 返回节点类型
func (b *BaseNode) GetKind() ASTKind {
	return b.Kind
}

// GetPosition 返回节点位置
func (b *BaseNode) GetPosition() Position {
	return b.Position
}

// GetChildren 返回子节点列表
func (b *BaseNode) GetChildren() []Node {
	return b.Children
}

// Accept 访问者模式的默认实现
func (b *BaseNode) Accept(visitor Visitor) interface{} {
	return visitor.Visit(b)
}

// String 返回基础的字符串表示
func (b *BaseNode) String() string {
	return fmt.Sprintf("%s@%s", b.Kind.String(), b.Position.String())
}

// =============================================================================
// 特殊节点类型
// =============================================================================

// ZvalNode 字面量值节点
type ZvalNode struct {
	BaseNode
	Value interface{} `json:"value"`
}

func NewZvalNode(value interface{}, pos Position) *ZvalNode {
	return &ZvalNode{
		BaseNode: BaseNode{Kind: AST_ZVAL, Position: pos},
		Value:    value,
	}
}

func (z *ZvalNode) String() string {
	return fmt.Sprintf("Zval(%v)@%s", z.Value, z.Position.String())
}

// ConstantNode 命名常量节点
type ConstantNode struct {
	BaseNode
	Name string `json:"name"`
}

func NewConstantNode(name string, pos Position) *ConstantNode {
	return &ConstantNode{
		BaseNode: BaseNode{Kind: AST_CONSTANT, Position: pos},
		Name:     name,
	}
}

func (c *ConstantNode) String() string {
	return fmt.Sprintf("Constant(%s)@%s", c.Name, c.Position.String())
}

// =============================================================================
// 列表节点类型
// =============================================================================

// ListNode 通用列表节点
type ListNode struct {
	BaseNode
	Elements []Node `json:"elements"`
}

// NewListNode 创建列表节点
func NewListNode(kind ASTKind, elements []Node, pos Position) *ListNode {
	return &ListNode{
		BaseNode: BaseNode{Kind: kind, Position: pos, Children: elements},
		Elements: elements,
	}
}

// GetChildren 重写以返回Elements
func (l *ListNode) GetChildren() []Node {
	return l.Elements
}

func (l *ListNode) String() string {
	return fmt.Sprintf("%s[%d]@%s", l.Kind.String(), len(l.Elements), l.Position.String())
}

// 具体的列表节点类型
func NewStatementList(statements []Node) *ListNode {
	pos := Position{}
	if len(statements) > 0 {
		pos = statements[0].GetPosition()
	}
	return NewListNode(AST_STMT_LIST, statements, pos)
}

func NewArgumentList(args []Node, pos Position) *ListNode {
	return NewListNode(AST_ARG_LIST, args, pos)
}

func NewParameterList(params []Node, pos Position) *ListNode {
	return NewListNode(AST_PARAM_LIST, params, pos)
}

func NewExpressionList(exprs []Node, pos Position) *ListNode {
	return NewListNode(AST_EXPR_LIST, exprs, pos)
}

func NewArrayLiteral(elements []Node, pos Position) *ListNode {
	return NewListNode(AST_ARRAY, elements, pos)
}

// =============================================================================
// 表达式节点类型
// =============================================================================

// UnaryNode 一元运算节点
type UnaryNode struct {
	BaseNode
	Operator string `json:"operator"`
	Operand  Node   `json:"operand"`
}

func NewUnaryNode(kind ASTKind, operator string, operand Node, pos Position) *UnaryNode {
	return &UnaryNode{
		BaseNode: BaseNode{Kind: kind, Position: pos, Children: []Node{operand}},
		Operator: operator,
		Operand:  operand,
	}
}

func (u *UnaryNode) String() string {
	return fmt.Sprintf("%s(%s)@%s", u.Kind.String(), u.Operator, u.Position.String())
}

// BinaryNode 二元运算节点
type BinaryNode struct {
	BaseNode
	Operator string `json:"operator"`
	Left     Node   `json:"left"`
	Right    Node   `json:"right"`
}

func NewBinaryNode(kind ASTKind, operator string, left, right Node, pos Position) *BinaryNode {
	return &BinaryNode{
		BaseNode: BaseNode{Kind: kind, Position: pos, Children: []Node{left, right}},
		Operator: operator,
		Left:     left,
		Right:    right,
	}
}

func (b *BinaryNode) String() string {
	return fmt.Sprintf("%s(%s)@%s", b.Kind.String(), b.Operator, b.Position.String())
}

// TernaryNode 三元运算节点
type TernaryNode struct {
	BaseNode
	Condition Node `json:"condition"`
	TrueExpr  Node `json:"true_expr"`
	FalseExpr Node `json:"false_expr"`
}

func NewTernaryNode(condition, trueExpr, falseExpr Node, pos Position) *TernaryNode {
	children := []Node{condition}
	if trueExpr != nil {
		children = append(children, trueExpr)
	}
	if falseExpr != nil {
		children = append(children, falseExpr)
	}
	
	return &TernaryNode{
		BaseNode:  BaseNode{Kind: AST_CONDITIONAL, Position: pos, Children: children},
		Condition: condition,
		TrueExpr:  trueExpr,
		FalseExpr: falseExpr,
	}
}

func (t *TernaryNode) String() string {
	return fmt.Sprintf("Conditional@%s", t.Position.String())
}

// AssignNode 赋值节点
type AssignNode struct {
	BaseNode
	Left  Node   `json:"left"`
	Right Node   `json:"right"`
	Op    string `json:"operator,omitempty"` // 用于复合赋值
}

func NewAssignNode(left, right Node, pos Position) *AssignNode {
	return &AssignNode{
		BaseNode: BaseNode{Kind: AST_ASSIGN, Position: pos, Children: []Node{left, right}},
		Left:     left,
		Right:    right,
	}
}

func NewCompoundAssignNode(operator string, left, right Node, pos Position) *AssignNode {
	return &AssignNode{
		BaseNode: BaseNode{Kind: AST_ASSIGN_OP, Position: pos, Children: []Node{left, right}},
		Left:     left,
		Right:    right,
		Op:       operator,
	}
}

func (a *AssignNode) String() string {
	op := "="
	if a.Op != "" {
		op = a.Op
	}
	return fmt.Sprintf("Assign(%s)@%s", op, a.Position.String())
}

// CallNode 函数/方法调用节点
type CallNode struct {
	BaseNode
	Callee    Node   `json:"callee"`
	Arguments []Node `json:"arguments"`
}

func NewCallNode(callee Node, arguments []Node, pos Position) *CallNode {
	children := []Node{callee}
	if len(arguments) > 0 {
		children = append(children, NewArgumentList(arguments, pos))
	}
	
	return &CallNode{
		BaseNode:  BaseNode{Kind: AST_CALL, Position: pos, Children: children},
		Callee:    callee,
		Arguments: arguments,
	}
}

func (c *CallNode) String() string {
	return fmt.Sprintf("Call(%d args)@%s", len(c.Arguments), c.Position.String())
}

// MethodCallNode 方法调用节点
type MethodCallNode struct {
	BaseNode
	Object    Node   `json:"object"`
	Method    Node   `json:"method"`
	Arguments []Node `json:"arguments"`
	Nullsafe  bool   `json:"nullsafe,omitempty"`
}

func NewMethodCallNode(object, method Node, arguments []Node, nullsafe bool, pos Position) *MethodCallNode {
	kind := AST_METHOD_CALL
	if nullsafe {
		kind = AST_NULLSAFE_METHOD_CALL
	}
	
	children := []Node{object, method}
	if len(arguments) > 0 {
		children = append(children, NewArgumentList(arguments, pos))
	}
	
	return &MethodCallNode{
		BaseNode:  BaseNode{Kind: kind, Position: pos, Children: children},
		Object:    object,
		Method:    method,
		Arguments: arguments,
		Nullsafe:  nullsafe,
	}
}

func (m *MethodCallNode) String() string {
	op := "->"
	if m.Nullsafe {
		op = "?->"
	}
	return fmt.Sprintf("MethodCall(%s, %d args)@%s", op, len(m.Arguments), m.Position.String())
}

// PropertyNode 属性访问节点
type PropertyNode struct {
	BaseNode
	Object   Node `json:"object"`
	Property Node `json:"property"`
	Nullsafe bool `json:"nullsafe,omitempty"`
}

func NewPropertyNode(object, property Node, nullsafe bool, pos Position) *PropertyNode {
	kind := AST_PROP
	if nullsafe {
		kind = AST_NULLSAFE_PROP
	}
	
	return &PropertyNode{
		BaseNode: BaseNode{Kind: kind, Position: pos, Children: []Node{object, property}},
		Object:   object,
		Property: property,
		Nullsafe: nullsafe,
	}
}

func (p *PropertyNode) String() string {
	op := "->"
	if p.Nullsafe {
		op = "?->"
	}
	return fmt.Sprintf("Property(%s)@%s", op, p.Position.String())
}

// ArrayAccessNode 数组访问节点
type ArrayAccessNode struct {
	BaseNode
	Array Node `json:"array"`
	Index Node `json:"index"`
}

func NewArrayAccessNode(array, index Node, pos Position) *ArrayAccessNode {
	return &ArrayAccessNode{
		BaseNode: BaseNode{Kind: AST_DIM, Position: pos, Children: []Node{array, index}},
		Array:    array,
		Index:    index,
	}
}

func (a *ArrayAccessNode) String() string {
	return fmt.Sprintf("ArrayAccess@%s", a.Position.String())
}

// VariableNode 变量节点
type VariableNode struct {
	BaseNode
	Name Node `json:"name"` // 可以是字符串或表达式(如可变变量)
}

func NewVariableNode(name Node, pos Position) *VariableNode {
	return &VariableNode{
		BaseNode: BaseNode{Kind: AST_VAR, Position: pos, Children: []Node{name}},
		Name:     name,
	}
}

// NewVariable 创建简单变量节点(名称为字符串)
func NewVariable(name string, pos Position) *VariableNode {
	nameNode := NewZvalNode(name, pos)
	return NewVariableNode(nameNode, pos)
}

func (v *VariableNode) String() string {
	return fmt.Sprintf("Variable@%s", v.Position.String())
}

// IdentifierNode 标识符节点
type IdentifierNode struct {
	BaseNode
	Name string `json:"name"`
}

func NewIdentifier(name string, pos Position) *IdentifierNode {
	return &IdentifierNode{
		BaseNode: BaseNode{Kind: AST_CONSTANT, Position: pos}, // 标识符作为常量处理
		Name:     name,
	}
}

func (i *IdentifierNode) String() string {
	return fmt.Sprintf("Identifier(%s)@%s", i.Name, i.Position.String())
}

// =============================================================================
// 字面量节点类型
// =============================================================================

func NewIntegerLiteral(value int64, pos Position) *ZvalNode {
	return NewZvalNode(value, pos)
}

func NewFloatLiteral(value float64, pos Position) *ZvalNode {
	return NewZvalNode(value, pos)
}

func NewStringLiteral(value string, pos Position) *ZvalNode {
	return NewZvalNode(value, pos)
}

func NewBooleanLiteral(value bool, pos Position) *ZvalNode {
	return NewZvalNode(value, pos)
}

func NewNullLiteral(pos Position) *ZvalNode {
	return NewZvalNode(nil, pos)
}

// =============================================================================
// 语句节点类型
// =============================================================================

// IfNode if语句节点
type IfNode struct {
	BaseNode
	Elements []Node `json:"elements"` // IfElementNode列表
}

func NewIfNode(elements []Node, pos Position) *IfNode {
	return &IfNode{
		BaseNode: BaseNode{Kind: AST_IF, Position: pos, Children: elements},
		Elements: elements,
	}
}

func (i *IfNode) String() string {
	return fmt.Sprintf("If(%d branches)@%s", len(i.Elements), i.Position.String())
}

// IfElementNode if/elseif/else元素节点
type IfElementNode struct {
	BaseNode
	Condition Node `json:"condition"` // nil表示else分支
	Body      Node `json:"body"`
}

func NewIfElementNode(condition, body Node, pos Position) *IfElementNode {
	children := []Node{body}
	if condition != nil {
		children = []Node{condition, body}
	}
	
	return &IfElementNode{
		BaseNode:  BaseNode{Kind: AST_IF_ELEM, Position: pos, Children: children},
		Condition: condition,
		Body:      body,
	}
}

func (i *IfElementNode) String() string {
	if i.Condition == nil {
		return fmt.Sprintf("Else@%s", i.Position.String())
	}
	return fmt.Sprintf("IfElem@%s", i.Position.String())
}

// WhileNode while循环节点
type WhileNode struct {
	BaseNode
	Condition Node `json:"condition"`
	Body      Node `json:"body"`
}

func NewWhileNode(condition, body Node, pos Position) *WhileNode {
	return &WhileNode{
		BaseNode:  BaseNode{Kind: AST_WHILE, Position: pos, Children: []Node{condition, body}},
		Condition: condition,
		Body:      body,
	}
}

func (w *WhileNode) String() string {
	return fmt.Sprintf("While@%s", w.Position.String())
}

// ForNode for循环节点
type ForNode struct {
	BaseNode
	Init      Node `json:"init"`
	Condition Node `json:"condition"`
	Update    Node `json:"update"`
	Body      Node `json:"body"`
}

func NewForNode(init, condition, update, body Node, pos Position) *ForNode {
	children := []Node{}
	if init != nil {
		children = append(children, init)
	}
	if condition != nil {
		children = append(children, condition)
	}
	if update != nil {
		children = append(children, update)
	}
	if body != nil {
		children = append(children, body)
	}
	
	return &ForNode{
		BaseNode:  BaseNode{Kind: AST_FOR, Position: pos, Children: children},
		Init:      init,
		Condition: condition,
		Update:    update,
		Body:      body,
	}
}

func (f *ForNode) String() string {
	return fmt.Sprintf("For@%s", f.Position.String())
}

// =============================================================================
// JSON序列化支持
// =============================================================================

// MarshalJSON 自定义JSON序列化
func (b *BaseNode) MarshalJSON() ([]byte, error) {
	type Alias BaseNode
	return json.Marshal(&struct {
		Kind string `json:"kind"`
		*Alias
	}{
		Kind:  b.Kind.String(),
		Alias: (*Alias)(b),
	})
}

// Walk 遍历AST节点
func Walk(node Node, fn func(Node)) {
	if node == nil {
		return
	}
	
	fn(node)
	
	for _, child := range node.GetChildren() {
		Walk(child, fn)
	}
}

// WalkDepthFirst 深度优先遍历AST节点
func WalkDepthFirst(node Node, fn func(Node) bool) {
	if node == nil {
		return
	}
	
	for _, child := range node.GetChildren() {
		WalkDepthFirst(child, fn)
	}
	
	fn(node)
}

// Print 打印AST结构
func Print(node Node, indent string) string {
	if node == nil {
		return indent + "<nil>"
	}
	
	result := indent + node.String()
	
	children := node.GetChildren()
	if len(children) > 0 {
		result += "\n"
		for i, child := range children {
			if i > 0 {
				result += "\n"
			}
			result += Print(child, indent+"  ")
		}
	}
	
	return result
}

// PrintCompact 打印紧凑的AST结构
func PrintCompact(node Node) string {
	if node == nil {
		return "<nil>"
	}
	
	children := node.GetChildren()
	if len(children) == 0 {
		return node.String()
	}
	
	var parts []string
	for _, child := range children {
		parts = append(parts, PrintCompact(child))
	}
	
	return fmt.Sprintf("%s{%s}", node.String(), strings.Join(parts, ", "))
}