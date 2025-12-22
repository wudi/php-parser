<?php
$data = "This is the secret message to be encrypted and tested.";

// 1. Generate a new private and public key pair
$new_key_pair = openssl_pkey_new([
    "private_key_bits" => 2048, // Recommended minimum length
    "private_key_type" => OPENSSL_KEYTYPE_RSA,
]);

// 2. Export the private key to a PEM string
openssl_pkey_export($new_key_pair, $private_key_pem);

// 3. Get the public key details and export the public key to a PEM string
$details = openssl_pkey_get_details($new_key_pair);
$public_key_pem = $details['key'];

// 4. Encrypt the data with the *public* key
openssl_public_encrypt($data, $encrypted_data, $public_key_pem);

// 5. Decrypt the data with the *private* key
openssl_private_decrypt($encrypted_data, $decrypted_data, $private_key_pem);

// 6. Test if the original and decrypted data match
if ($data === $decrypted_data) {
    echo "RSA Encryption/Decryption Test: **SUCCESS**\n";
} else {
    echo "RSA Encryption/Decryption Test: **FAILED**\n";
    echo "Original: " . $data . "\n";
    echo "Decrypted: " . $decrypted_data . "\n";
}

?>
