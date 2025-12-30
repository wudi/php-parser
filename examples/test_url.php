<?php

echo "Testing urlencode/urldecode:\n";
$url = "https://example.com/path?query=val space&other=@#$";
$encoded = urlencode($url);
echo "Encoded: $encoded\n";
$decoded = urldecode($encoded);
echo "Decoded: $decoded\n";
if ($url === $decoded) echo "PASS\n"; else echo "FAIL\n";

echo "\nTesting rawurlencode/rawurldecode:\n";
$raw_encoded = rawurlencode($url);
echo "Raw Encoded: $raw_encoded\n";
$raw_decoded = rawurldecode($raw_encoded);
echo "Raw Decoded: $raw_decoded\n";
if ($url === $raw_decoded) echo "PASS\n"; else echo "FAIL\n";

echo "\nTesting base64_encode/base64_decode:\n";
$data = "Hello World!";
$b64 = base64_encode($data);
echo "Base64: $b64\n";
$b64_decoded = base64_decode($b64);
echo "Decoded: $b64_decoded\n";
if ($data === $b64_decoded) echo "PASS\n"; else echo "FAIL\n";

echo "\nTesting parse_url with complex URL:\n";
$url2 = "https://user:pass@example.com:8080/path/to/page.php?q=search&id=123#section1";
$parsed2 = parse_url($url2);
print_r($parsed2);

echo "\nTesting parse_url with component:\n";
echo "Scheme: " . parse_url($url2, PHP_URL_SCHEME) . "\n";
echo "Host: " . parse_url($url2, PHP_URL_HOST) . "\n";
echo "Port: " . parse_url($url2, PHP_URL_PORT) . "\n";
echo "User: " . parse_url($url2, PHP_URL_USER) . "\n";
echo "Pass: " . parse_url($url2, PHP_URL_PASS) . "\n";
echo "Path: " . parse_url($url2, PHP_URL_PATH) . "\n";
echo "Query: " . parse_url($url2, PHP_URL_QUERY) . "\n";
echo "Fragment: " . parse_url($url2, PHP_URL_FRAGMENT) . "\n";

echo "\nTesting http_build_query:\n";
$query_data = [
    'a' => 1,
    'b' => 'foo bar',
    'c' => ['d' => 2, 'e' => 'baz'],
    'f' => [10, 20]
];
$query = http_build_query($query_data);
echo "Query: $query\n";

echo "\nTesting http_build_query with object:\n";
class MyQuery {
    public $a = 1;
    public $b = "hello";
}
$obj = new MyQuery();
echo "Query from object: " . http_build_query($obj) . "\n";

echo "\nTesting http_build_query with numeric prefix:\n";
$data = [1, 2, 3];
echo "Query with prefix: " . http_build_query($data, "num_") . "\n";

echo "\nTesting http_build_query with custom separator:\n";
$data = ['a' => 1, 'b' => 2];
echo "Query with separator: " . http_build_query($data, "", ";") . "\n";

echo "\nTesting http_build_query with RFC3986:\n";
$data = ['a' => 'foo bar'];
echo "RFC1738: " . http_build_query($data, "", "&", PHP_QUERY_RFC1738) . "\n";
echo "RFC3986: " . http_build_query($data, "", "&", PHP_QUERY_RFC3986) . "\n";

echo "\nTesting constants:\n";
echo "PHP_URL_SCHEME: " . PHP_URL_SCHEME . "\n";
echo "PHP_QUERY_RFC3986: " . PHP_QUERY_RFC3986 . "\n";
?>
