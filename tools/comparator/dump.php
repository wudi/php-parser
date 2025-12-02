<?php

require __DIR__ . '/vendor/autoload.php';

use PhpParser\Error;
use PhpParser\NodeDumper;
use PhpParser\ParserFactory;
use PhpParser\Node;

if ($argc < 2) {
    echo "Usage: php dump.php <file>\n";
    exit(1);
}

$code = file_get_contents($argv[1]);
$parser = (new ParserFactory)->createForNewestSupportedVersion();

try {
    $ast = $parser->parse($code);
    
    // Simplify AST for comparison (remove attributes like startLine, endLine, comments for now if we want structural equality)
    // But for now, let's just dump the structure as JSON.
    
    echo json_encode($ast, JSON_PRETTY_PRINT);
} catch (Error $error) {
    echo json_encode(['error' => $error->getMessage()]);
    exit(1);
}
