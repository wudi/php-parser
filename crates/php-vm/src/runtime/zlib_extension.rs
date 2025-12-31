use crate::builtins::zlib;
use crate::core::value::Val;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;

pub struct ZlibExtension;

impl Extension for ZlibExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "zlib",
            version: "2.0.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register functions
        registry.register_function(b"gzcompress", zlib::php_gzcompress);
        registry.register_function(b"gzuncompress", zlib::php_gzuncompress);
        registry.register_function(b"gzdeflate", zlib::php_gzdeflate);
        registry.register_function(b"gzinflate", zlib::php_gzinflate);
        registry.register_function(b"gzencode", zlib::php_gzencode);
        registry.register_function(b"gzdecode", zlib::php_gzdecode);
        registry.register_function(b"zlib_encode", zlib::php_zlib_encode);
        registry.register_function(b"zlib_decode", zlib::php_zlib_decode);
        registry.register_function(b"deflate_init", zlib::php_deflate_init);
        registry.register_function(b"deflate_add", zlib::php_deflate_add);
        registry.register_function(b"inflate_init", zlib::php_inflate_init);
        registry.register_function(b"inflate_add", zlib::php_inflate_add);
        registry.register_function(b"inflate_get_status", zlib::php_inflate_get_status);
        registry.register_function(b"inflate_get_read_len", zlib::php_inflate_get_read_len);
        registry.register_function(b"gzopen", zlib::php_gzopen);
        registry.register_function(b"gzread", zlib::php_gzread);
        registry.register_function(b"gzwrite", zlib::php_gzwrite);
        registry.register_function(b"gzclose", zlib::php_gzclose);
        registry.register_function(b"gzeof", zlib::php_gzeof);
        registry.register_function(b"gztell", zlib::php_gztell);
        registry.register_function(b"gzseek", zlib::php_gzseek);
        registry.register_function(b"gzrewind", zlib::php_gzrewind);
        registry.register_function(b"gzgets", zlib::php_gzgets);
        registry.register_function(b"gzgetc", zlib::php_gzgetc);
        registry.register_function(b"gzpassthru", zlib::php_gzpassthru);
        registry.register_function(b"readgzfile", zlib::php_readgzfile);
        registry.register_function(b"gzfile", zlib::php_gzfile);
        registry.register_function(b"ob_gzhandler", zlib::php_ob_gzhandler);
        registry.register_function(b"zlib_get_coding_type", zlib::php_zlib_get_coding_type);

        // Register classes
        use crate::runtime::registry::NativeClassDef;

        registry.register_class(NativeClassDef {
            name: b"DeflateContext".to_vec(),
            parent: None,
            is_interface: false,
            is_trait: false,
            interfaces: Vec::new(),
            methods: std::collections::HashMap::new(),
            constants: std::collections::HashMap::new(),
            constructor: None,
        });

        registry.register_class(NativeClassDef {
            name: b"InflateContext".to_vec(),
            parent: None,
            is_interface: false,
            is_trait: false,
            interfaces: Vec::new(),
            methods: std::collections::HashMap::new(),
            constants: std::collections::HashMap::new(),
            constructor: None,
        });

        // Register constants
        registry.register_constant(b"FORCE_GZIP", Val::Int(31));
        registry.register_constant(b"FORCE_DEFLATE", Val::Int(15));
        registry.register_constant(b"ZLIB_ENCODING_RAW", Val::Int(-1));
        registry.register_constant(b"ZLIB_ENCODING_DEFLATE", Val::Int(15));
        registry.register_constant(b"ZLIB_ENCODING_GZIP", Val::Int(31));

        registry.register_constant(b"ZLIB_FILTERED", Val::Int(1));
        registry.register_constant(b"ZLIB_HUFFMAN_ONLY", Val::Int(2));
        registry.register_constant(b"ZLIB_FIXED", Val::Int(4));
        registry.register_constant(b"ZLIB_RLE", Val::Int(3));
        registry.register_constant(b"ZLIB_DEFAULT_STRATEGY", Val::Int(0));
        registry.register_constant(b"ZLIB_BLOCK", Val::Int(5));

        registry.register_constant(b"ZLIB_NO_FLUSH", Val::Int(0));
        registry.register_constant(b"ZLIB_PARTIAL_FLUSH", Val::Int(1));
        registry.register_constant(b"ZLIB_SYNC_FLUSH", Val::Int(2));
        registry.register_constant(b"ZLIB_FULL_FLUSH", Val::Int(3));
        registry.register_constant(b"ZLIB_FINISH", Val::Int(4));

        registry.register_constant(
            b"ZLIB_VERSION",
            Val::String(std::rc::Rc::new(b"1.2.11".to_vec())),
        );
        registry.register_constant(b"ZLIB_VERNUM", Val::Int(0x12b0));

        registry.register_constant(b"ZLIB_OK", Val::Int(0));
        registry.register_constant(b"ZLIB_STREAM_END", Val::Int(1));
        registry.register_constant(b"ZLIB_NEED_DICT", Val::Int(2));
        registry.register_constant(b"ZLIB_ERRNO", Val::Int(-1));
        registry.register_constant(b"ZLIB_STREAM_ERROR", Val::Int(-2));
        registry.register_constant(b"ZLIB_DATA_ERROR", Val::Int(-3));
        registry.register_constant(b"ZLIB_MEM_ERROR", Val::Int(-4));
        registry.register_constant(b"ZLIB_BUF_ERROR", Val::Int(-5));
        registry.register_constant(b"ZLIB_VERSION_ERROR", Val::Int(-6));

        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_init(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
