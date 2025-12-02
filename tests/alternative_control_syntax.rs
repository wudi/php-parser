use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_alternative_if_endif() {
    let source = b"<?php
if ($x > 0):
    echo 'positive';
elseif ($x < 0):
    echo 'negative';
else:
    echo 'zero';
endif;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_while_endwhile() {
    let source = b"<?php
while ($i < 10):
    echo $i;
    $i++;
endwhile;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_for_endfor() {
    let source = b"<?php
for ($i = 0; $i < 10; $i++):
    echo $i;
endfor;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_foreach_endforeach() {
    let source = b"<?php
foreach ($items as $key => $value):
    echo $key . ': ' . $value;
endforeach;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_switch_endswitch() {
    let source = b"<?php
switch ($value):
    case 1:
        echo 'one';
        break;
    case 2:
        echo 'two';
        break;
    default:
        echo 'other';
endswitch;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nested_alternative_syntax() {
    let source = b"<?php
if ($x > 0):
    foreach ($items as $item):
        echo $item;
    endforeach;
endif;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_mixed_regular_and_alternative_syntax() {
    let source = b"<?php
if ($x > 0) {
    foreach ($items as $item):
        echo $item;
    endforeach;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_if_with_html() {
    let source = b"<?php if ($show): ?>
    <div>Content</div>
<?php endif; ?>";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_foreach_with_html() {
    let source = b"<?php foreach ($items as $item): ?>
    <li><?= $item ?></li>
<?php endforeach; ?>";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_if_empty_blocks() {
    let source = b"<?php
if ($x):
elseif ($y):
else:
endif;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_while_empty() {
    let source = b"<?php
while ($condition):
endwhile;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_alternative_for_complex() {
    let source = b"<?php
for ($i = 0, $j = 10; $i < $j; $i++, $j--):
    echo $i + $j;
endfor;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
