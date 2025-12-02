<?php

require __DIR__ . '/vendor/autoload.php';

use PhpParser\ParserFactory;

if ($argc < 2) {
    exit(1);
}

$file = $argv[1];
$code = file_get_contents($file);

// 1. Check Parsing
try {
    $parser = (new ParserFactory)->createForNewestSupportedVersion();
    $parser->parse($code);
} catch (\Throwable $e) {
    exit(1);
}

// 2. Dump Tokens
$tokens = token_get_all($code);
$output = [];

foreach ($tokens as $token) {
    if (is_string($token)) {
        $output[] = [
            "kind" => $token,
            "text" => $token
        ];
    } else {
        list($id, $text, $line) = $token;
        if ($id === T_WHITESPACE) {
            continue;
        }
        $output[] = [
            "kind" => token_name($id),
            "text" => $text
        ];
    }
}

echo json_encode($output);
exit(0);
