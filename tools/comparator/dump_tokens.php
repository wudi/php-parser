<?php

// Dump tokens as JSON for comparison
// Output format: array of { "kind": string|int, "text": string }

$code = file_get_contents($argv[1]);
$tokens = token_get_all($code);

$output = [];

foreach ($tokens as $token) {
    if (is_string($token)) {
        $output[] = [
            "kind" => $token, // Single char token
            "text" => $token
        ];
    } else {
        list($id, $text, $line) = $token;
        
        // Filter whitespace if Rust parser skips it
        if ($id === T_WHITESPACE) {
            continue;
        }
        
        // Filter open tag if Rust parser handles it differently?
        // Rust parser emits OpenTag. PHP emits T_OPEN_TAG.
        
        $output[] = [
            "kind" => token_name($id),
            "text" => $text
        ];
    }
}

echo json_encode($output, JSON_PRETTY_PRINT);
