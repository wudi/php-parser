use crate::builtins::openssl;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::{ExtensionRegistry, NativeClassDef};
use crate::core::value::{Val, Visibility};
use std::collections::HashMap;

pub struct OpenSSLExtension;

impl Extension for OpenSSLExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "openssl",
            version: "8.3.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register constants
        registry.register_constant(b"X509_PURPOSE_SSL_CLIENT", Val::Int(openssl::X509_PURPOSE_SSL_CLIENT));
        registry.register_constant(b"X509_PURPOSE_SSL_SERVER", Val::Int(openssl::X509_PURPOSE_SSL_SERVER));
        registry.register_constant(b"X509_PURPOSE_NS_SSL_SERVER", Val::Int(openssl::X509_PURPOSE_NS_SSL_SERVER));
        registry.register_constant(b"X509_PURPOSE_SMIME_SIGN", Val::Int(openssl::X509_PURPOSE_SMIME_SIGN));
        registry.register_constant(b"X509_PURPOSE_SMIME_ENCRYPT", Val::Int(openssl::X509_PURPOSE_SMIME_ENCRYPT));
        registry.register_constant(b"X509_PURPOSE_CRL_SIGN", Val::Int(openssl::X509_PURPOSE_CRL_SIGN));
        registry.register_constant(b"X509_PURPOSE_ANY", Val::Int(openssl::X509_PURPOSE_ANY));

        registry.register_constant(b"OPENSSL_PKCS1_PADDING", Val::Int(openssl::OPENSSL_PKCS1_PADDING));
        registry.register_constant(b"OPENSSL_SSLV23_PADDING", Val::Int(openssl::OPENSSL_SSLV23_PADDING));
        registry.register_constant(b"OPENSSL_NO_PADDING", Val::Int(openssl::OPENSSL_NO_PADDING));
        registry.register_constant(b"OPENSSL_PKCS1_OAEP_PADDING", Val::Int(openssl::OPENSSL_PKCS1_OAEP_PADDING));

        registry.register_constant(b"OPENSSL_KEYTYPE_RSA", Val::Int(openssl::OPENSSL_KEYTYPE_RSA));
        registry.register_constant(b"OPENSSL_KEYTYPE_DSA", Val::Int(openssl::OPENSSL_KEYTYPE_DSA));
        registry.register_constant(b"OPENSSL_KEYTYPE_DH", Val::Int(openssl::OPENSSL_KEYTYPE_DH));
        registry.register_constant(b"OPENSSL_KEYTYPE_EC", Val::Int(openssl::OPENSSL_KEYTYPE_EC));
        registry.register_constant(b"OPENSSL_KEYTYPE_X25519", Val::Int(openssl::OPENSSL_KEYTYPE_X25519));
        registry.register_constant(b"OPENSSL_KEYTYPE_ED25519", Val::Int(openssl::OPENSSL_KEYTYPE_ED25519));
        registry.register_constant(b"OPENSSL_KEYTYPE_X448", Val::Int(openssl::OPENSSL_KEYTYPE_X448));
        registry.register_constant(b"OPENSSL_KEYTYPE_ED448", Val::Int(openssl::OPENSSL_KEYTYPE_ED448));

        registry.register_constant(b"PKCS7_TEXT", Val::Int(openssl::PKCS7_TEXT));
        registry.register_constant(b"PKCS7_BINARY", Val::Int(openssl::PKCS7_BINARY));
        registry.register_constant(b"PKCS7_NOINTERN", Val::Int(openssl::PKCS7_NOINTERN));
        registry.register_constant(b"PKCS7_NOVERIFY", Val::Int(openssl::PKCS7_NOVERIFY));
        registry.register_constant(b"PKCS7_NOCHAIN", Val::Int(openssl::PKCS7_NOCHAIN));
        registry.register_constant(b"PKCS7_NOCERTS", Val::Int(openssl::PKCS7_NOCERTS));
        registry.register_constant(b"PKCS7_NOATTR", Val::Int(openssl::PKCS7_NOATTR));
        registry.register_constant(b"PKCS7_DETACHED", Val::Int(openssl::PKCS7_DETACHED));
        registry.register_constant(b"PKCS7_NOSIGS", Val::Int(openssl::PKCS7_NOSIGS));
        registry.register_constant(b"PKCS7_NOOLDMIMETYPE", Val::Int(openssl::PKCS7_NOOLDMIMETYPE));

        registry.register_constant(b"OPENSSL_CMS_TEXT", Val::Int(openssl::OPENSSL_CMS_TEXT));
        registry.register_constant(b"OPENSSL_CMS_BINARY", Val::Int(openssl::OPENSSL_CMS_BINARY));
        registry.register_constant(b"OPENSSL_CMS_NOINTERN", Val::Int(openssl::OPENSSL_CMS_NOINTERN));
        registry.register_constant(b"OPENSSL_CMS_NOVERIFY", Val::Int(openssl::OPENSSL_CMS_NOVERIFY));
        registry.register_constant(b"OPENSSL_CMS_NOCERTS", Val::Int(openssl::OPENSSL_CMS_NOCERTS));
        registry.register_constant(b"OPENSSL_CMS_NOATTR", Val::Int(openssl::OPENSSL_CMS_NOATTR));
        registry.register_constant(b"OPENSSL_CMS_DETACHED", Val::Int(openssl::OPENSSL_CMS_DETACHED));
        registry.register_constant(b"OPENSSL_CMS_NOSIGS", Val::Int(openssl::OPENSSL_CMS_NOSIGS));
        registry.register_constant(b"OPENSSL_CMS_OLDMIMETYPE", Val::Int(openssl::OPENSSL_CMS_OLDMIMETYPE));

        registry.register_constant(b"OPENSSL_ALGO_DSS1", Val::Int(openssl::OPENSSL_ALGO_DSS1));
        registry.register_constant(b"OPENSSL_ALGO_SHA1", Val::Int(openssl::OPENSSL_ALGO_SHA1));
        registry.register_constant(b"OPENSSL_ALGO_SHA224", Val::Int(openssl::OPENSSL_ALGO_SHA224));
        registry.register_constant(b"OPENSSL_ALGO_SHA256", Val::Int(openssl::OPENSSL_ALGO_SHA256));
        registry.register_constant(b"OPENSSL_ALGO_SHA384", Val::Int(openssl::OPENSSL_ALGO_SHA384));
        registry.register_constant(b"OPENSSL_ALGO_SHA512", Val::Int(openssl::OPENSSL_ALGO_SHA512));
        registry.register_constant(b"OPENSSL_ALGO_RMD160", Val::Int(openssl::OPENSSL_ALGO_RMD160));
        registry.register_constant(b"OPENSSL_ALGO_MD5", Val::Int(openssl::OPENSSL_ALGO_MD5));
        registry.register_constant(b"OPENSSL_ALGO_MD4", Val::Int(openssl::OPENSSL_ALGO_MD4));
        registry.register_constant(b"OPENSSL_ALGO_MD2", Val::Int(openssl::OPENSSL_ALGO_MD2));

        registry.register_constant(b"OPENSSL_CIPHER_RC2_40", Val::Int(openssl::OPENSSL_CIPHER_RC2_40));
        registry.register_constant(b"OPENSSL_CIPHER_RC2_128", Val::Int(openssl::OPENSSL_CIPHER_RC2_128));
        registry.register_constant(b"OPENSSL_CIPHER_RC2_64", Val::Int(openssl::OPENSSL_CIPHER_RC2_64));
        registry.register_constant(b"OPENSSL_CIPHER_DES", Val::Int(openssl::OPENSSL_CIPHER_DES));
        registry.register_constant(b"OPENSSL_CIPHER_3DES", Val::Int(openssl::OPENSSL_CIPHER_3DES));
        registry.register_constant(b"OPENSSL_CIPHER_AES_128_CBC", Val::Int(openssl::OPENSSL_CIPHER_AES_128_CBC));
        registry.register_constant(b"OPENSSL_CIPHER_AES_192_CBC", Val::Int(openssl::OPENSSL_CIPHER_AES_192_CBC));
        registry.register_constant(b"OPENSSL_CIPHER_AES_256_CBC", Val::Int(openssl::OPENSSL_CIPHER_AES_256_CBC));

        registry.register_constant(b"OPENSSL_RAW_DATA", Val::Int(openssl::OPENSSL_RAW_DATA));
        registry.register_constant(b"OPENSSL_DONT_ZERO_PAD_KEY", Val::Int(openssl::OPENSSL_DONT_ZERO_PAD_KEY));
        registry.register_constant(b"OPENSSL_ZERO_PADDING", Val::Int(openssl::OPENSSL_ZERO_PADDING));
        registry.register_constant(b"OPENSSL_ENCODING_SMIME", Val::Int(openssl::OPENSSL_ENCODING_SMIME));
        registry.register_constant(b"OPENSSL_ENCODING_DER", Val::Int(openssl::OPENSSL_ENCODING_DER));
        registry.register_constant(b"OPENSSL_ENCODING_PEM", Val::Int(openssl::OPENSSL_ENCODING_PEM));

        // Register functions
        registry.register_function(b"openssl_error_string", openssl::openssl_error_string);
        registry.register_function_with_by_ref(
            b"openssl_random_pseudo_bytes",
            openssl::openssl_random_pseudo_bytes,
            vec![1],
        );
        registry.register_function(b"openssl_cipher_iv_length", openssl::openssl_cipher_iv_length);
        registry.register_function(b"openssl_cipher_key_length", openssl::openssl_cipher_key_length);
        registry.register_function(b"openssl_digest", openssl::openssl_digest);
        registry.register_function_with_by_ref(
            b"openssl_encrypt",
            openssl::openssl_encrypt,
            vec![5],
        );
        registry.register_function(b"openssl_decrypt", openssl::openssl_decrypt);
        registry.register_function_with_by_ref(
            b"openssl_public_encrypt",
            openssl::openssl_public_encrypt,
            vec![1],
        );
        registry.register_function_with_by_ref(
            b"openssl_private_decrypt",
            openssl::openssl_private_decrypt,
            vec![1],
        );
        registry.register_function(b"openssl_pkey_new", openssl::openssl_pkey_new);
        registry.register_function(b"openssl_pkey_get_details", openssl::openssl_pkey_get_details);
        registry.register_function(b"openssl_pkey_get_private", openssl::openssl_pkey_get_private);
        registry.register_function(b"openssl_pkey_get_public", openssl::openssl_pkey_get_public);
        registry.register_function_with_by_ref(
            b"openssl_pkey_export",
            openssl::openssl_pkey_export,
            vec![1],
        );
        registry.register_function(b"openssl_pkey_export_to_file", openssl::openssl_pkey_export_to_file);
        registry.register_function(b"openssl_pkey_derive", openssl::openssl_pkey_derive);
        registry.register_function(b"openssl_pkey_free", openssl::openssl_pkey_free);
        registry.register_function(b"openssl_get_privatekey", openssl::openssl_pkey_get_private);
        registry.register_function(b"openssl_get_publickey", openssl::openssl_pkey_get_public);
        registry.register_function(b"openssl_free_key", openssl::openssl_pkey_free);
        registry.register_function(b"openssl_x509_read", openssl::openssl_x509_read);
        registry.register_function_with_by_ref(
            b"openssl_x509_export",
            openssl::openssl_x509_export,
            vec![1],
        );
        registry.register_function(b"openssl_x509_export_to_file", openssl::openssl_x509_export_to_file);
        registry.register_function(b"openssl_x509_fingerprint", openssl::openssl_x509_fingerprint);
        registry.register_function(b"openssl_x509_parse", openssl::openssl_x509_parse);
        registry.register_function(b"openssl_x509_check_private_key", openssl::openssl_x509_check_private_key);
        registry.register_function(b"openssl_x509_verify", openssl::openssl_x509_verify);
        registry.register_function(b"openssl_x509_free", openssl::openssl_x509_free);
        registry.register_function_with_by_ref(
            b"openssl_csr_new",
            openssl::openssl_csr_new,
            vec![1],
        );
        registry.register_function_with_by_ref(
            b"openssl_csr_export",
            openssl::openssl_csr_export,
            vec![1],
        );
        registry.register_function(b"openssl_csr_export_to_file", openssl::openssl_csr_export_to_file);
        registry.register_function(b"openssl_csr_sign", openssl::openssl_csr_sign);
        registry.register_function(b"openssl_csr_get_subject", openssl::openssl_csr_get_subject);
        registry.register_function(b"openssl_csr_get_public_key", openssl::openssl_csr_get_public_key);
        registry.register_function_with_by_ref(b"openssl_sign", openssl::openssl_sign, vec![1]);
        registry.register_function(b"openssl_verify", openssl::openssl_verify);
        registry.register_function(b"openssl_pbkdf2", openssl::openssl_pbkdf2);
        registry.register_function(b"openssl_get_curve_names", openssl::openssl_get_curve_names);
        registry.register_function(b"openssl_pkcs7_encrypt", openssl::openssl_pkcs7_encrypt);
        registry.register_function(b"openssl_pkcs7_decrypt", openssl::openssl_pkcs7_decrypt);
        registry.register_function(b"openssl_pkcs7_sign", openssl::openssl_pkcs7_sign);
        registry.register_function(b"openssl_pkcs7_verify", openssl::openssl_pkcs7_verify);
        registry.register_function(b"openssl_cms_encrypt", openssl::openssl_cms_encrypt);
        registry.register_function(b"openssl_cms_decrypt", openssl::openssl_cms_decrypt);
        registry.register_function(b"openssl_cms_sign", openssl::openssl_cms_sign);
        registry.register_function(b"openssl_cms_verify", openssl::openssl_cms_verify);
        registry.register_function(b"openssl_get_md_methods", openssl::openssl_get_md_methods);
        registry.register_function(b"openssl_get_cipher_methods", openssl::openssl_get_cipher_methods);
        registry.register_function(b"openssl_get_cert_locations", openssl::openssl_get_cert_locations);
        registry.register_function(b"openssl_spki_new", openssl::openssl_spki_new);
        registry.register_function(b"openssl_spki_export", openssl::openssl_spki_export);
        registry.register_function(b"openssl_spki_verify", openssl::openssl_spki_verify);
        registry.register_function_with_by_ref(
            b"openssl_pkcs12_export",
            openssl::openssl_pkcs12_export,
            vec![1],
        );
        registry.register_function(b"openssl_pkcs12_export_to_file", openssl::openssl_pkcs12_export_to_file);
        registry.register_function_with_by_ref(
            b"openssl_pkcs12_read",
            openssl::openssl_pkcs12_read,
            vec![1],
        );
        registry.register_function_with_by_ref(
            b"openssl_private_encrypt",
            openssl::openssl_private_encrypt,
            vec![1],
        );
        registry.register_function_with_by_ref(
            b"openssl_public_decrypt",
            openssl::openssl_public_decrypt,
            vec![1],
        );

        // Register classes
        registry.register_class(NativeClassDef {
            name: b"OpenSSLAsymmetricKey".to_vec(),
            parent: None,
            interfaces: Vec::new(),
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        registry.register_class(NativeClassDef {
            name: b"OpenSSLCertificate".to_vec(),
            parent: None,
            interfaces: Vec::new(),
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        registry.register_class(NativeClassDef {
            name: b"OpenSSLCertificateSigningRequest".to_vec(),
            parent: None,
            interfaces: Vec::new(),
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        ExtensionResult::Success
    }
}
