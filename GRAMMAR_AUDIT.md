# PHP Grammar Implementation Audit

This document tracks the completeness of the PHP parser implementation against `tools/grammar.y`.

## Status Legend
- ✅ **Complete**: Fully implemented with edge cases
- ⚠️ **Partial**: Implemented but missing edge cases
- ❌ **Missing**: Not implemented
- 🔍 **Needs Review**: Requires deeper inspection

---

## 1. Tokens & Lexer (Lines 93-349)

### Magic Constants (Lines 235-249)
- ✅ `T_LINE` - `__LINE__`
- ✅ `T_FILE` - `__FILE__`
- ✅ `T_DIR` - `__DIR__`
- ✅ `T_CLASS_C` - `__CLASS__`
- ✅ `T_TRAIT_C` - `__TRAIT__`
- ✅ `T_METHOD_C` - `__METHOD__`
- ✅ `T_FUNC_C` - `__FUNCTION__`
- ✅ `T_PROPERTY_C` - `__PROPERTY__` (Line 247)
- ✅ `T_NS_C` - `__NAMESPACE__`

### Asymmetric Visibility Modifiers (Lines 204-208)
- 🔍 `T_PRIVATE_SET` - `'private(set)'`
- 🔍 `T_PROTECTED_SET` - `'protected(set)'`
- 🔍 `T_PUBLIC_SET` - `'public(set)'`

### Other Key Tokens
- ✅ `T_PIPE` - `'|>'` (Pipe operator, Line 331)
- ✅ `T_AMPERSAND_FOLLOWED_BY_VAR_OR_VARARG` (Line 339)
- ✅ `T_AMPERSAND_NOT_FOLLOWED_BY_VAR_OR_VARARG` (Line 344)

---

## 2. Basic Rules (Lines 450-524)

### Start & Identifiers
- 🔍 `start` (Line 450-452)
- 🔍 `reserved_non_modifiers` (Lines 453-474) - includes T_PROPERTY_C
- 🔍 `semi_reserved` (Lines 475-480)
- 🔍 `ampersand` (Lines 483-487)
- 🔍 `identifier` (Lines 488-491)
- 🔍 `top_statement_list` (Lines 492-495)

### Namespace Names
- 🔍 `namespace_declaration_name` (Lines 499-502)
- 🔍 `namespace_name` (Lines 506-509)
- 🔍 `legacy_namespace_name` (Lines 515-518)
- 🔍 `name` (Lines 519-524)

---

## 3. Attributes (Lines 525-541)

- 🔍 `attribute_decl` (Lines 525-528)
- 🔍 `attribute_group` (Lines 529-534)
- 🔍 `attribute` (Lines 535-537)
- 🔍 `attributes` (Lines 538-541)

---

## 4. Top-Level Statements (Lines 542-620)

### Attributed Statements
- 🔍 `attributed_statement` (Lines 542-548)
- 🔍 `attributed_top_statement` (Lines 549-553)
- 🔍 `top_statement` (Lines 554-576)

### Use Declarations
- 🔍 `use_type` (Lines 577-580)
- 🔍 `group_use_declaration` (Lines 581-584)
- 🔍 `mixed_group_use_declaration` (Lines 585-588)
- 🔍 `possible_comma` (Lines 589-592)
- 🔍 `inline_use_declarations` (Lines 593-598)
- 🔍 `unprefixed_use_declarations` (Lines 599-603)
- 🔍 `use_declarations` (Lines 604-607)
- 🔍 `inline_use_declaration` (Lines 608-611)
- 🔍 `unprefixed_use_declaration` (Lines 612-615)
- 🔍 `use_declaration` (Lines 616-620)

### Constants
- 🔍 `const_list` (Lines 621-624)

---

## 5. Inner Statements (Lines 625-685)

- 🔍 `inner_statement_list` (Lines 625-628)
- 🔍 `inner_statement` (Lines 629-638)
- ✅ `statement` (Lines 639-685)
  - Includes: blocks, if, while, do-while, for, switch, break, continue, return, global, static, echo, unset, foreach, declare, try-catch, goto, labels, void cast

---

## 6. Exception Handling (Lines 686-702)

- 🔍 `catch_list` (Lines 686-690)
- 🔍 `catch_name_list` (Lines 691-694)
- 🔍 `optional_variable` (Lines 695-698)
- 🔍 `finally_statement` (Lines 699-702)

---

## 7. Variables & Functions (Lines 703-731)

- 🔍 `unset_variables` (Lines 703-708)
- 🔍 `unset_variable` (Lines 709-711)
- 🔍 `function_name` (Lines 712-715) - includes T_READONLY
- 🔍 `function_declaration_statement` (Lines 716-721)
- 🔍 `is_reference` (Lines 722-727)
- 🔍 `is_variadic` (Lines 728-731)

---

## 8. Class Declarations (Lines 732-800)

### Class
- 🔍 `class_declaration_statement` (Lines 732-739)
- 🔍 `class_modifiers` (Lines 740-745)
- 🔍 `anonymous_class_modifiers` (Lines 746-749)
- 🔍 `anonymous_class_modifiers_optional` (Lines 750-753)
- 🔍 `class_modifier` (Lines 754-758) - abstract, final, readonly

### Trait, Interface, Enum
- 🔍 `trait_declaration_statement` (Lines 759-764)
- 🔍 `interface_declaration_statement` (Lines 765-769)
- 🔍 `enum_declaration_statement` (Lines 770-774)
- 🔍 `enum_backing_type` (Lines 775-778)
- 🔍 `enum_case` (Lines 779-781)
- 🔍 `enum_case_expr` (Lines 782-785)

### Inheritance
- 🔍 `extends_from` (Lines 786-789)
- 🔍 `interface_extends_list` (Lines 790-794)
- 🔍 `implements_list` (Lines 795-800)

---

## 9. Control Flow (Lines 801-890)

### Loops
- 🔍 `foreach_variable` (Lines 801-807)
- 🔍 `for_statement` (Lines 808-811)
- 🔍 `foreach_statement` (Lines 812-816)
- 🔍 `declare_statement` (Lines 817-820)

### Switch & Match
- 🔍 `switch_case_list` (Lines 821-829)
- 🔍 `case_list` (Lines 830-841)
- 🔍 `match` (Lines 842-845)
- 🔍 `match_arm_list` (Lines 846-849)
- 🔍 `non_empty_match_arm_list` (Lines 850-853)
- 🔍 `match_arm` (Lines 854-860)
- 🔍 `match_arm_cond_list` (Lines 861-864)

### If/While
- 🔍 `while_statement` (Lines 865-868)
- 🔍 `if_stmt_without_else` (Lines 869-872)
- 🔍 `if_stmt` (Lines 873-879)
- 🔍 `alt_if_stmt_without_else` (Lines 880-885)
- 🔍 `alt_if_stmt` (Lines 886-890)

---

## 10. Parameters & Types (Lines 891-990)

### Parameters
- 🔍 `parameter_list` (Lines 891-896)
- 🔍 `non_empty_parameter_list` (Lines 897-901)
- 🔍 `attributed_parameter` (Lines 902-906)
- 🔍 `optional_cpp_modifiers` (Lines 907-910)
- 🔍 `parameter` (Lines 911-921) - **CRITICAL: Includes optional_property_hook_list**

### Types
- 🔍 `optional_type_without_static` (Lines 922-925)
- 🔍 `type_expr` (Lines 926-933)
- 🔍 `type` (Lines 934-937)
- 🔍 `union_type_element` (Lines 938-942)
- 🔍 `union_type` (Lines 943-947)
- 🔍 `intersection_type` (Lines 948-951)
- 🔍 `type_expr_without_static` (Lines 958-963)
- 🔍 `type_without_static` (Lines 964-968)
- 🔍 `union_type_without_static_element` (Lines 969-973)
- 🔍 `union_type_without_static` (Lines 974-980)
- 🔍 `intersection_type_without_static` (Lines 981-986)
- 🔍 `return_type` (Lines 987-990)

---

## 11. Arguments (Lines 991-1044)

### Regular Arguments
- 🔍 `argument_list` (Lines 991-996)
- 🔍 `non_empty_argument_list` (Lines 997-1002)

### Clone Arguments (Special handling)
- 🔍 `clone_argument_list` (Lines 1020-1029)
- 🔍 `non_empty_clone_argument_list` (Lines 1030-1034)
- 🔍 `argument_no_expr` (Lines 1035-1038)
- 🔍 `argument` (Lines 1039-1044)

### Variables
- 🔍 `global_var_list` (Lines 1045-1048)
- 🔍 `global_var` (Lines 1049-1051)
- 🔍 `static_var_list` (Lines 1052-1056)
- 🔍 `static_var` (Lines 1057-1060)

---

## 12. Class Members (Lines 1061-1219)

### Class Statements
- 🔍 `class_statement_list` (Lines 1061-1067)
- 🔍 `attributed_class_statement` (Lines 1068-1079)
- 🔍 `class_statement` (Lines 1080-1084)

### Traits
- 🔍 `class_name_list` (Lines 1085-1088)
- 🔍 `trait_adaptations` (Lines 1089-1096)
- 🔍 `trait_adaptation_list` (Lines 1097-1100)
- 🔍 `trait_adaptation` (Lines 1101-1104)
- 🔍 `trait_precedence` (Lines 1105-1107)
- 🔍 `trait_alias` (Lines 1108-1119)
- 🔍 `trait_method_reference` (Lines 1120-1123)
- 🔍 `absolute_trait_method_reference` (Lines 1124-1127)

### Methods & Properties
- 🔍 `method_body` (Lines 1128-1132)
- 🔍 `property_modifiers` (Lines 1133-1136)
- 🔍 `method_modifiers` (Lines 1137-1140)
- 🔍 `class_const_modifiers` (Lines 1141-1144)
- 🔍 `non_empty_member_modifiers` (Lines 1145-1148)
- 🔍 `member_modifier` (Lines 1149-1162) - **CRITICAL: Includes T_PUBLIC_SET, T_PROTECTED_SET, T_PRIVATE_SET**

### Properties
- 🔍 `property_list` (Lines 1163-1166)
- 🔍 `property` (Lines 1167-1170)

### **Property Hooks** (Lines 1171-1208) - **CRITICAL SECTION**
- 🔍 `hooked_property` (Lines 1171-1176)
- 🔍 `property_hook_list` (Lines 1177-1181)
- 🔍 `optional_property_hook_list` (Lines 1182-1187)
- 🔍 `property_hook_modifiers` (Lines 1188-1191)
- 🔍 `property_hook` (Lines 1192-1197)
- 🔍 `property_hook_body` (Lines 1198-1204)
- 🔍 `optional_parameter_list` (Lines 1205-1208)

### Constants
- 🔍 `class_const_list` (Lines 1209-1212)
- 🔍 `class_const_decl` (Lines 1213-1216)
- 🔍 `const_decl` (Lines 1217-1219)

---

## 13. Expressions - Part 1 (Lines 1220-1244)

### Echo & For
- 🔍 `echo_expr_list` (Lines 1220-1223)
- 🔍 `echo_expr` (Lines 1224-1226)
- 🔍 `for_cond_exprs` (Lines 1227-1231)
- 🔍 `for_exprs` (Lines 1232-1237)
- ✅ `non_empty_for_exprs` (Lines 1238-1244) - **Includes T_VOID_CAST**

---

## 14. Classes & Objects (Lines 1245-1258)

- 🔍 `anonymous_class` (Lines 1245-1249)
- 🔍 `new_dereferenceable` (Lines 1250-1255)
- 🔍 `new_non_dereferenceable` (Lines 1256-1258)

---

## 15. Expressions - Part 2 (Lines 1259-1369)

### Main Expression Rule
- 🔍 `expr` (Lines 1259-1369) - **MASSIVE RULE**
  - Variable assignments
  - List destructuring
  - Clone (with clone_argument_list)
  - Compound assignments (+=, -=, *=, etc.)
  - Increment/decrement
  - Boolean operators
  - Bitwise operators
  - Arithmetic operators
  - Comparison operators
  - Ternary & null coalesce
  - instanceof
  - Casts
  - Exit/die
  - Error suppression (@)
  - Scalar values
  - Backticks
  - Print
  - Yield & yield from
  - Throw
  - Inline functions (closures & arrow functions)
  - Match expressions
  - Pipe operator (T_PIPE)

---

## 16. Functions & Closures (Lines 1370-1415)

- 🔍 `inline_function` (Lines 1370-1378)
- 🔍 `fn` (Lines 1379-1381)
- 🔍 `function` (Lines 1382-1386)
- 🔍 `backup_doc_comment` (Lines 1387-1389)
- 🔍 `backup_fn_flags` (Lines 1390-1392)
- 🔍 `backup_lex_pos` (Lines 1393-1395)
- 🔍 `returns_ref` (Lines 1396-1399)
- 🔍 `lexical_vars` (Lines 1400-1404)
- 🔍 `lexical_var_list` (Lines 1405-1408)
- 🔍 `lexical_var` (Lines 1409-1415)

---

## 17. Function Calls & Names (Lines 1416-1443)

- 🔍 `function_call` (Lines 1416-1425)
- 🔍 `class_name` (Lines 1426-1429)
- 🔍 `class_name_reference` (Lines 1430-1434)
- 🔍 `backticks_expr` (Lines 1435-1439)
- 🔍 `ctor_arguments` (Lines 1440-1443)

---

## 18. Scalars & Constants (Lines 1444-1482)

- 🔍 `dereferenceable_scalar` (Lines 1444-1450)
- 🔍 `scalar` (Lines 1451-1462)
- 🔍 `constant` (Lines 1463-1474) - **CRITICAL: Includes T_PROPERTY_C (Line 1471)**
- 🔍 `class_constant` (Lines 1475-1482)

---

## 19. Variables & Dereferencing (Lines 1483-1556)

- 🔍 `optional_expr` (Lines 1483-1486)
- 🔍 `variable_class_name` (Lines 1489-1491)
- 🔍 `fully_dereferenceable` (Lines 1492-1498)
- 🔍 `array_object_dereferenceable` (Lines 1499-1502)
- 🔍 `callable_expr` (Lines 1503-1508)
- 🔍 `callable_variable` (Lines 1509-1517)
- 🔍 `variable` (Lines 1518-1526)
- 🔍 `simple_variable` (Lines 1527-1531)
- 🔍 `static_member` (Lines 1532-1536)
- 🔍 `new_variable` (Lines 1537-1546)
- 🔍 `member_name` (Lines 1547-1551)
- 🔍 `property_name` (Lines 1552-1556)

---

## 20. Arrays (Lines 1559-1581)

- 🔍 `array_pair_list` (Lines 1559-1563)
- 🔍 `possible_array_pair` (Lines 1564-1567)
- 🔍 `non_empty_array_pair_list` (Lines 1568-1571)
- 🔍 `array_pair` (Lines 1572-1581)

---

## 21. String Interpolation (Lines 1582-1606)

- 🔍 `encaps_list` (Lines 1582-1589)
- 🔍 `encaps_var` (Lines 1590-1600)
- 🔍 `encaps_var_offset` (Lines 1601-1606)

---

## 22. Internal Functions (Lines 1609-1626)

- 🔍 `internal_functions_in_yacc` (Lines 1609-1617)
- 🔍 `isset_variables` (Lines 1618-1623)
- 🔍 `isset_variable` (Lines 1624-1626)

---

## Critical Areas Requiring Deep Review

### 1. **Property Hooks** (PHP 8.4 Feature)
- Lines 1171-1208 in grammar
- Must verify: hooked_property, property_hook_list, property_hook, property_hook_body
- Edge cases: attributes on hooks, modifiers, arrow syntax vs block syntax

### 2. **Asymmetric Visibility** (PHP 8.4 Feature)
- Lines 204-208, 1149-1162 in grammar
- Must verify: T_PUBLIC_SET, T_PROTECTED_SET, T_PRIVATE_SET in member_modifier

### 3. **Magic Constant __PROPERTY__** (PHP 8.4 Feature)
- Line 247 (token), Line 473 (reserved_non_modifiers), Line 1471 (constant rule)
- Must verify: lexer recognition, parser handling, AST representation

### 4. **Clone with Arguments** (PHP 8.4 Feature?)
- Lines 1020-1034, 1265-1267 in grammar
- Special clone_argument_list to handle ambiguity

### 5. **Pipe Operator**
- Line 331 (T_PIPE token), Line 1326 (expr rule)
- Must verify: precedence, associativity

### 6. **Void Cast in For Loops**
- Lines 682, 1238-1244 in grammar
- T_VOID_CAST in statements and non_empty_for_exprs

### 7. **Parameters with Property Hooks**
- Line 915, 920 in grammar (parameter rule includes optional_property_hook_list)
- Constructor property promotion with hooks

---

## Next Steps

1. ✅ Review lexer tokens implementation
2. 🔍 Review property hooks implementation in parser
3. 🔍 Review asymmetric visibility implementation
4. 🔍 Review __PROPERTY__ magic constant
5. 🔍 Review clone argument list handling
6. 🔍 Review pipe operator
7. 🔍 Review void cast
8. 🔍 Systematic review of all expression rules
9. 🔍 Systematic review of all statement rules
10. 🔍 Edge case testing for each feature
