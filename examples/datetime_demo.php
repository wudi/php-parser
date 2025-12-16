<?php

echo "=== Date/Time Functions Demo ===\n\n";

// Basic date functions
echo "Current time: " . time() . "\n";
echo "Current date: " . date("Y-m-d H:i:s") . "\n";
echo "GMT date: " . gmdate("Y-m-d H:i:s") . "\n\n";

// Checkdate
echo "checkdate(2, 29, 2024): " . (checkdate(2, 29, 2024) ? "true" : "false") . "\n";
echo "checkdate(2, 29, 2023): " . (checkdate(2, 29, 2023) ? "true" : "false") . "\n\n";

// Microtime
echo "microtime(true): " . microtime(true) . "\n";
echo "microtime(false): " . microtime(false) . "\n\n";

// mktime
echo "mktime(0, 0, 0, 1, 1, 2024): " . mktime(0, 0, 0, 1, 1, 2024) . "\n\n";

// getdate
print_r(getdate());
echo "\n";

// date_parse
print_r(date_parse("2024-01-15 14:30:45"));
echo "\n";

// Timezone
echo "date_default_timezone_get(): " . date_default_timezone_get() . "\n";
date_default_timezone_set("America/New_York");
echo "After setting to America/New_York: " . date_default_timezone_get() . "\n\n";

// Date constants
echo "DATE_ATOM: " . DATE_ATOM . "\n";
echo "DATE_RFC3339: " . DATE_RFC3339 . "\n";

echo "\n=== Demo Complete ===\n";
