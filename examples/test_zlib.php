<?php
// Original string
$str = 'hello world, this is a simple string that will be compressed.';

// Define the compression encoding (DEFLATE is a raw zlib format)
$encoding = ZLIB_ENCODING_DEFLATE;

// Compress the string
$compressed_data = zlib_encode($str, $encoding);

// Output the original and compressed size (for demonstration)
echo "Original size: " . strlen($str) . " bytes\n";
echo "Compressed size: " . strlen($compressed_data) . " bytes\n\n";

// Decompress the data
$uncompressed_data = zlib_decode($compressed_data);

// Verify that the uncompressed data matches the original
if ($uncompressed_data === $str) {
    echo "Decompression successful. Data matches original.\n";
    echo "Uncompressed string: " . $uncompressed_data;
} else {
    echo "Decompression failed or data does not match.";
}
