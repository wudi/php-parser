# Parser TODO (PHP Parity)

Reference sources: `$PHP_SRC_PATH/Zend/zend_language_scanner.l` (tokens/lexing), `$PHP_SRC_PATH/Zend/zend_language_parser.y` (grammar), and `$PHP_SRC_PATH/Zend/zend_ast.h` (AST kinds). Do **not** introduce non-PHP syntax or AST kinds; mirror Zend semantics.