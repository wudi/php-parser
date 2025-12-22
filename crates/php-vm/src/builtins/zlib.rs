use crate::core::value::{ArrayData, ArrayKey, Handle, ObjectData, Val};
use crate::vm::engine::VM;
use flate2::read::{
    DeflateDecoder, DeflateEncoder, GzDecoder, GzEncoder as GzReadEncoder, ZlibDecoder, ZlibEncoder,
};
use flate2::write::GzEncoder as GzWriteEncoder;
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use std::any::Any;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Write};
use std::rc::Rc;

pub struct GzFile {
    pub inner: RefCell<Box<dyn GzFileInner>>,
}

pub trait GzFileInner: Any {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>;
    fn eof(&mut self) -> bool;
    fn tell(&mut self) -> u64;
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64>;
    fn gets(&mut self, length: usize) -> std::io::Result<Vec<u8>>;
    fn close(&mut self) -> std::io::Result<()>;
}

struct GzFileReader {
    decoder: GzDecoder<File>,
    path: String,
    eof: bool,
    pos: u64,
}

impl GzFileInner for GzFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.decoder.read(buf)?;
        if n == 0 {
            self.eof = true;
        }
        self.pos += n as u64;
        Ok(n)
    }
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "File opened for reading",
        ))
    }
    fn eof(&mut self) -> bool {
        self.eof
    }
    fn tell(&mut self) -> u64 {
        self.pos
    }
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(0) => {
                let f = File::open(&self.path)?;
                self.decoder = GzDecoder::new(f);
                self.pos = 0;
                self.eof = false;
                Ok(0)
            }
            std::io::SeekFrom::Current(offset) if offset >= 0 => {
                let mut skip = offset as u64;
                let mut buf = [0u8; 8192];
                while skip > 0 {
                    let to_read = std::cmp::min(skip, buf.len() as u64) as usize;
                    let n = self.read(&mut buf[..to_read])?;
                    if n == 0 {
                        break;
                    }
                    skip -= n as u64;
                }
                Ok(self.pos)
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Limited seek support on GzFileReader",
            )),
        }
    }
    fn gets(&mut self, length: usize) -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        while buf.len() < length - 1 {
            if self.read(&mut byte)? == 0 {
                break;
            }
            buf.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        Ok(buf)
    }
    fn close(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct GzFileWriter {
    encoder: Option<GzWriteEncoder<File>>,
    pos: u64,
}

impl GzFileInner for GzFileWriter {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "File opened for writing",
        ))
    }
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(ref mut encoder) = self.encoder {
            let n = encoder.write(buf)?;
            self.pos += n as u64;
            Ok(n)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "File already closed",
            ))
        }
    }
    fn eof(&mut self) -> bool {
        false
    }
    fn tell(&mut self) -> u64 {
        self.pos
    }
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Seek not supported on GzFileWriter",
        ))
    }
    fn gets(&mut self, _length: usize) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "File opened for writing",
        ))
    }
    fn close(&mut self) -> std::io::Result<()> {
        if let Some(encoder) = self.encoder.take() {
            encoder.finish()?;
        }
        Ok(())
    }
}

pub struct DeflateContext {
    pub compress: RefCell<Compress>,
    pub encoding: i64,
}

pub struct InflateContext {
    pub decompress: RefCell<Decompress>,
    pub encoding: i64,
    pub status: RefCell<Status>,
    pub read_len: RefCell<usize>,
}

/// gzcompress(string $data, int $level = -1, int $encoding = ZLIB_ENCODING_DEFLATE): string|false
pub fn php_gzcompress(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 {
        return Err("gzcompress() expects 1 to 3 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzcompress(): Argument #1 ($data) must be of type string".into()),
    };

    let level = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => {
                if *i < -1 || *i > 9 {
                    -1
                } else {
                    *i as i32
                }
            }
            _ => -1,
        }
    } else {
        -1
    };

    let compression = if level == -1 {
        Compression::default()
    } else {
        Compression::new(level as u32)
    };

    let mut encoder = ZlibEncoder::new(&data[..], compression);
    let mut buffer = Vec::new();
    if encoder.read_to_end(&mut buffer).is_err() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// gzuncompress(string $data, int $max_length = 0): string|false
pub fn php_gzuncompress(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("gzuncompress() expects 1 or 2 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzuncompress(): Argument #1 ($data) must be of type string".into()),
    };

    let max_length = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i as usize,
            _ => 0,
        }
    } else {
        0
    };

    let mut decoder = ZlibDecoder::new(&data[..]);
    let mut buffer = Vec::new();

    let result = if max_length > 0 {
        decoder.take(max_length as u64).read_to_end(&mut buffer)
    } else {
        decoder.read_to_end(&mut buffer)
    };

    if result.is_err() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// gzdeflate(string $data, int $level = -1, int $encoding = ZLIB_ENCODING_RAW): string|false
pub fn php_gzdeflate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 {
        return Err("gzdeflate() expects 1 to 3 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzdeflate(): Argument #1 ($data) must be of type string".into()),
    };

    let level = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => {
                if *i < -1 || *i > 9 {
                    -1
                } else {
                    *i as i32
                }
            }
            _ => -1,
        }
    } else {
        -1
    };

    let compression = if level == -1 {
        Compression::default()
    } else {
        Compression::new(level as u32)
    };

    let mut encoder = DeflateEncoder::new(&data[..], compression);
    let mut buffer = Vec::new();
    if encoder.read_to_end(&mut buffer).is_err() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// gzinflate(string $data, int $max_length = 0): string|false
pub fn php_gzinflate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("gzinflate() expects 1 or 2 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzinflate(): Argument #1 ($data) must be of type string".into()),
    };

    let max_length = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i as usize,
            _ => 0,
        }
    } else {
        0
    };

    let mut decoder = DeflateDecoder::new(&data[..]);
    let mut buffer = Vec::new();

    let result = if max_length > 0 {
        decoder.take(max_length as u64).read_to_end(&mut buffer)
    } else {
        decoder.read_to_end(&mut buffer)
    };

    if result.is_err() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// gzencode(string $data, int $level = -1, int $encoding = FORCE_GZIP): string|false
pub fn php_gzencode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 {
        return Err("gzencode() expects 1 to 3 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzencode(): Argument #1 ($data) must be of type string".into()),
    };

    let level = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => {
                if *i < -1 || *i > 9 {
                    -1
                } else {
                    *i as i32
                }
            }
            _ => -1,
        }
    } else {
        -1
    };

    let compression = if level == -1 {
        Compression::default()
    } else {
        Compression::new(level as u32)
    };

    let mut encoder = GzReadEncoder::new(&data[..], compression);
    let mut buffer = Vec::new();
    if encoder.read_to_end(&mut buffer).is_err() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// gzdecode(string $data, int $max_length = 0): string|false
pub fn php_gzdecode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("gzdecode() expects 1 or 2 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzdecode(): Argument #1 ($data) must be of type string".into()),
    };

    let max_length = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i as usize,
            _ => 0,
        }
    } else {
        0
    };

    let mut decoder = GzDecoder::new(&data[..]);
    let mut buffer = Vec::new();

    let result = if max_length > 0 {
        decoder.take(max_length as u64).read_to_end(&mut buffer)
    } else {
        decoder.read_to_end(&mut buffer)
    };

    if result.is_err() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// zlib_encode(string $data, int $encoding, int $level = -1): string|false
pub fn php_zlib_encode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("zlib_encode() expects 2 or 3 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("zlib_encode(): Argument #1 ($data) must be of type string".into()),
    };

    let encoding = match &vm.arena.get(args[1]).value {
        Val::Int(i) => *i,
        _ => return Err("zlib_encode(): Argument #2 ($encoding) must be of type int".into()),
    };

    let level = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => {
                if *i < -1 || *i > 9 {
                    -1
                } else {
                    *i as i32
                }
            }
            _ => -1,
        }
    } else {
        -1
    };

    let compression = if level == -1 {
        Compression::default()
    } else {
        Compression::new(level as u32)
    };

    let mut buffer = Vec::new();
    match encoding {
        15 => {
            // ZLIB_ENCODING_DEFLATE
            let mut encoder = ZlibEncoder::new(&data[..], compression);
            if encoder.read_to_end(&mut buffer).is_err() {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        31 => {
            // ZLIB_ENCODING_GZIP
            let mut encoder = GzReadEncoder::new(&data[..], compression);
            if encoder.read_to_end(&mut buffer).is_err() {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        -1 => {
            // ZLIB_ENCODING_RAW
            let mut encoder = DeflateEncoder::new(&data[..], compression);
            if encoder.read_to_end(&mut buffer).is_err() {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Err(format!("zlib_encode(): Unknown encoding: {}", encoding)),
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// zlib_decode(string $data, int $max_length = 0): string|false
pub fn php_zlib_decode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("zlib_decode() expects 1 or 2 parameters".into());
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("zlib_decode(): Argument #1 ($data) must be of type string".into()),
    };

    let max_length = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i as usize,
            _ => 0,
        }
    } else {
        0
    };

    // zlib_decode is supposed to auto-detect the encoding.
    // flate2 doesn't have an auto-detect decoder easily.
    // PHP's zlib_decode supports raw, zlib, and gzip.

    // Try GZIP first
    let mut decoder = GzDecoder::new(&data[..]);
    let mut buffer = Vec::new();
    let result = if max_length > 0 {
        decoder.take(max_length as u64).read_to_end(&mut buffer)
    } else {
        decoder.read_to_end(&mut buffer)
    };
    if result.is_ok() {
        return Ok(vm.arena.alloc(Val::String(Rc::new(buffer))));
    }

    // Try ZLIB
    buffer.clear();
    let mut decoder = ZlibDecoder::new(&data[..]);
    let result = if max_length > 0 {
        decoder.take(max_length as u64).read_to_end(&mut buffer)
    } else {
        decoder.read_to_end(&mut buffer)
    };
    if result.is_ok() {
        return Ok(vm.arena.alloc(Val::String(Rc::new(buffer))));
    }

    // Try RAW DEFLATE
    buffer.clear();
    let mut decoder = DeflateDecoder::new(&data[..]);
    let result = if max_length > 0 {
        decoder.take(max_length as u64).read_to_end(&mut buffer)
    } else {
        decoder.read_to_end(&mut buffer)
    };
    if result.is_ok() {
        return Ok(vm.arena.alloc(Val::String(Rc::new(buffer))));
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

/// deflate_init(int $encoding, array $options = []): DeflateContext|false
pub fn php_deflate_init(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("deflate_init() expects 1 or 2 parameters".into());
    }

    let encoding = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i,
        _ => return Err("deflate_init(): Argument #1 ($encoding) must be of type int".into()),
    };

    let mut level = -1;
    if args.len() >= 2 {
        if let Val::Array(arr) = &vm.arena.get(args[1]).value {
            let level_key = b"level";
            for (k, v) in arr.map.iter() {
                if let ArrayKey::Str(key_bytes) = k {
                    if key_bytes.as_slice() == level_key {
                        if let Val::Int(l) = vm.arena.get(*v).value {
                            level = l as i32;
                        }
                    }
                }
            }
        }
    }

    let compression = if level == -1 {
        Compression::default()
    } else {
        Compression::new(level as u32)
    };

    let zlib_header = match encoding {
        15 => true,  // ZLIB_ENCODING_DEFLATE
        31 => false, // ZLIB_ENCODING_GZIP (handled differently in flate2)
        -1 => false, // ZLIB_ENCODING_RAW
        _ => return Err(format!("deflate_init(): Unknown encoding: {}", encoding)),
    };

    // flate2::Compress::new(level, zlib_header)
    // If encoding is GZIP, we might need a different approach or manual header.
    // For now, let's support ZLIB and RAW.
    let compress = Compress::new(compression, zlib_header);

    let context = DeflateContext {
        compress: RefCell::new(compress),
        encoding,
    };

    let class_name = vm.context.interner.intern(b"DeflateContext");
    let obj = ObjectData {
        class: class_name,
        properties: indexmap::IndexMap::new(),
        internal: Some(Rc::new(context)),
        dynamic_properties: std::collections::HashSet::new(),
    };

    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

/// deflate_add(DeflateContext $context, string $data, int $flush_mode = ZLIB_NO_FLUSH): string|false
pub fn php_deflate_add(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("deflate_add() expects 2 or 3 parameters".into());
    }

    let obj_handle = args[0];
    let data = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("deflate_add(): Argument #2 ($data) must be of type string".into()),
    };

    let flush_mode = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => match *i {
                0 => FlushCompress::None,
                1 => FlushCompress::Partial,
                2 => FlushCompress::Sync,
                3 => FlushCompress::Full,
                4 => FlushCompress::Finish,
                _ => FlushCompress::None,
            },
            _ => FlushCompress::None,
        }
    } else {
        FlushCompress::None
    };

    let internal = match &vm.arena.get(obj_handle).value {
        Val::ObjPayload(p) => p.internal.clone(),
        _ => {
            return Err(
                "deflate_add(): Argument #1 ($context) must be of type DeflateContext".into(),
            )
        }
    };

    let internal = internal.ok_or("deflate_add(): Invalid DeflateContext")?;
    let context = internal
        .downcast_ref::<DeflateContext>()
        .ok_or("deflate_add(): Invalid DeflateContext")?;

    let mut compress = context.compress.borrow_mut();
    let mut output = Vec::with_capacity(data.len() / 2 + 64);

    // Incremental compression
    let mut input_pos = 0;
    while input_pos < data.len() {
        let before_in = compress.total_in();
        let before_out = compress.total_out();

        let mut temp_out = vec![0u8; 4096];
        match compress.compress(&data[input_pos..], &mut temp_out, flush_mode) {
            Ok(Status::Ok) | Ok(Status::BufError) => {
                let consumed = (compress.total_in() - before_in) as usize;
                let produced = (compress.total_out() - before_out) as usize;
                output.extend_from_slice(&temp_out[..produced]);
                input_pos += consumed;
                if consumed == 0 && produced == 0 {
                    break;
                }
            }
            Ok(Status::StreamEnd) => {
                let produced = (compress.total_out() - before_out) as usize;
                output.extend_from_slice(&temp_out[..produced]);
                break;
            }
            Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    }

    // If flush_mode is Finish or Sync, we might need more calls to get all data
    if flush_mode == FlushCompress::Finish
        || flush_mode == FlushCompress::Sync
        || flush_mode == FlushCompress::Full
    {
        loop {
            let before_out = compress.total_out();
            let mut temp_out = vec![0u8; 4096];
            match compress.compress(&[], &mut temp_out, flush_mode) {
                Ok(status) => {
                    let produced = (compress.total_out() - before_out) as usize;
                    output.extend_from_slice(&temp_out[..produced]);
                    if status == Status::StreamEnd || produced == 0 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(output))))
}

/// inflate_init(int $encoding, array $options = []): InflateContext|false
pub fn php_inflate_init(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("inflate_init() expects 1 or 2 parameters".into());
    }

    let encoding = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i,
        _ => return Err("inflate_init(): Argument #1 ($encoding) must be of type int".into()),
    };

    let zlib_header = match encoding {
        15 => true,  // ZLIB_ENCODING_DEFLATE
        31 => false, // ZLIB_ENCODING_GZIP
        -1 => false, // ZLIB_ENCODING_RAW
        _ => return Err(format!("inflate_init(): Unknown encoding: {}", encoding)),
    };

    let decompress = Decompress::new(zlib_header);

    let context = InflateContext {
        decompress: RefCell::new(decompress),
        encoding,
        status: RefCell::new(Status::Ok),
        read_len: RefCell::new(0),
    };

    let class_name = vm.context.interner.intern(b"InflateContext");
    let obj = ObjectData {
        class: class_name,
        properties: indexmap::IndexMap::new(),
        internal: Some(Rc::new(context)),
        dynamic_properties: std::collections::HashSet::new(),
    };

    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

/// inflate_add(InflateContext $context, string $data, int $flush_mode = ZLIB_NO_FLUSH): string|false
pub fn php_inflate_add(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("inflate_add() expects 2 or 3 parameters".into());
    }

    let obj_handle = args[0];
    let data = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("inflate_add(): Argument #2 ($data) must be of type string".into()),
    };

    let flush_mode = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => match *i {
                0 => FlushDecompress::None,
                1 => FlushDecompress::Sync,
                4 => FlushDecompress::Finish,
                _ => FlushDecompress::None,
            },
            _ => FlushDecompress::None,
        }
    } else {
        FlushDecompress::None
    };

    let internal = match &vm.arena.get(obj_handle).value {
        Val::ObjPayload(p) => p.internal.clone(),
        _ => {
            return Err(
                "inflate_add(): Argument #1 ($context) must be of type InflateContext".into(),
            )
        }
    };

    let internal = internal.ok_or("inflate_add(): Invalid InflateContext")?;
    let context = internal
        .downcast_ref::<InflateContext>()
        .ok_or("inflate_add(): Invalid InflateContext")?;

    let mut decompress = context.decompress.borrow_mut();
    let mut output = Vec::with_capacity(data.len() * 2);

    let mut input_pos = 0;
    loop {
        let before_in = decompress.total_in();
        let before_out = decompress.total_out();

        let mut temp_out = vec![0u8; 4096];
        match decompress.decompress(&data[input_pos..], &mut temp_out, flush_mode) {
            Ok(status) => {
                let consumed = (decompress.total_in() - before_in) as usize;
                let produced = (decompress.total_out() - before_out) as usize;
                output.extend_from_slice(&temp_out[..produced]);
                input_pos += consumed;
                *context.read_len.borrow_mut() += consumed;
                *context.status.borrow_mut() = status;

                if status == Status::StreamEnd {
                    break;
                }
                if consumed == 0 && produced == 0 {
                    if input_pos >= data.len() {
                        break;
                    }
                }
            }
            Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(output))))
}

/// inflate_get_status(InflateContext $context): int
pub fn php_inflate_get_status(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("inflate_get_status() expects 1 parameter".into());
    }

    let internal = match &vm.arena.get(args[0]).value {
        Val::ObjPayload(p) => p.internal.clone(),
        _ => {
            return Err(
                "inflate_get_status(): Argument #1 ($context) must be of type InflateContext"
                    .into(),
            )
        }
    };

    let internal = internal.ok_or("inflate_get_status(): Invalid InflateContext")?;
    let context = internal
        .downcast_ref::<InflateContext>()
        .ok_or("inflate_get_status(): Invalid InflateContext")?;

    let status = match *context.status.borrow() {
        Status::Ok => 0,        // ZLIB_OK
        Status::StreamEnd => 1, // ZLIB_STREAM_END
        Status::BufError => -5, // ZLIB_BUF_ERROR
    };

    Ok(vm.arena.alloc(Val::Int(status as i64)))
}

/// inflate_get_read_len(InflateContext $context): int
pub fn php_inflate_get_read_len(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("inflate_get_read_len() expects 1 parameter".into());
    }

    let internal = match &vm.arena.get(args[0]).value {
        Val::ObjPayload(p) => p.internal.clone(),
        _ => {
            return Err(
                "inflate_get_read_len(): Argument #1 ($context) must be of type InflateContext"
                    .into(),
            )
        }
    };

    let internal = internal.ok_or("inflate_get_read_len(): Invalid InflateContext")?;
    let context = internal
        .downcast_ref::<InflateContext>()
        .ok_or("inflate_get_read_len(): Invalid InflateContext")?;

    let read_len = *context.read_len.borrow();
    Ok(vm.arena.alloc(Val::Int(read_len as i64)))
}

/// gzopen(string $filename, string $mode, int $use_include_path = 0): resource|false
pub fn php_gzopen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("gzopen() expects 2 or 3 parameters".into());
    }

    let filename = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("gzopen(): Argument #1 ($filename) must be of type string".into()),
    };

    let mode = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("gzopen(): Argument #2 ($mode) must be of type string".into()),
    };

    let file = if mode.contains('r') {
        let f = File::open(&filename).map_err(|e| e.to_string())?;
        let decoder = GzDecoder::new(f);
        GzFile {
            inner: RefCell::new(Box::new(GzFileReader {
                decoder,
                path: filename.clone(),
                eof: false,
                pos: 0,
            })),
        }
    } else if mode.contains('w') || mode.contains('a') {
        let f = File::create(&filename).map_err(|e| e.to_string())?;
        let encoder = GzWriteEncoder::new(f, Compression::default());
        GzFile {
            inner: RefCell::new(Box::new(GzFileWriter {
                encoder: Some(encoder),
                pos: 0,
            })),
        }
    } else {
        return Err(format!("gzopen(): Invalid mode: {}", mode));
    };

    Ok(vm.arena.alloc(Val::Resource(Rc::new(file))))
}

/// gzread(resource $stream, int $length): string|false
pub fn php_gzread(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("gzread() expects 2 parameters".into());
    }

    let length = match &vm.arena.get(args[1]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("gzread(): Argument #2 ($length) must be of type int".into()),
    };

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzread(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzread(): Invalid resource")?;

    let mut buffer = vec![0u8; length];
    let n = gz_file
        .inner
        .borrow_mut()
        .read(&mut buffer)
        .map_err(|e| e.to_string())?;
    buffer.truncate(n);

    Ok(vm.arena.alloc(Val::String(Rc::new(buffer))))
}

/// gzwrite(resource $stream, string $data, ?int $length = null): int|false
pub fn php_gzwrite(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("gzwrite() expects 2 or 3 parameters".into());
    }

    let data = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("gzwrite(): Argument #2 ($data) must be of type string".into()),
    };

    let length = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => Some(*i as usize),
            Val::Null => None,
            _ => return Err("gzwrite(): Argument #3 ($length) must be of type int or null".into()),
        }
    } else {
        None
    };

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzwrite(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzwrite(): Invalid resource")?;

    let to_write = if let Some(l) = length {
        if l < data.len() {
            &data[..l]
        } else {
            &data[..]
        }
    } else {
        &data[..]
    };

    let n = gz_file
        .inner
        .borrow_mut()
        .write(to_write)
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Int(n as i64)))
}

/// gzclose(resource $stream): bool
pub fn php_gzclose(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gzclose() expects 1 parameter".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzclose(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzclose(): Invalid resource")?;

    gz_file
        .inner
        .borrow_mut()
        .close()
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// gzeof(resource $stream): bool
pub fn php_gzeof(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gzeof() expects 1 parameter".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzeof(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzeof(): Invalid resource")?;

    let eof = gz_file.inner.borrow_mut().eof();
    Ok(vm.arena.alloc(Val::Bool(eof)))
}

/// gztell(resource $stream): int|false
pub fn php_gztell(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gztell() expects 1 parameter".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gztell(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gztell(): Invalid resource")?;

    let pos = gz_file.inner.borrow_mut().tell();
    Ok(vm.arena.alloc(Val::Int(pos as i64)))
}

/// gzseek(resource $stream, int $offset, int $whence = SEEK_SET): int
/// gzseek(resource $stream, int $offset, int $whence = SEEK_SET): int
pub fn php_gzseek(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("gzseek() expects 2 or 3 parameters".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzseek(): Argument #1 ($stream) must be of type resource".into()),
    };

    let offset = match &vm.arena.get(args[1]).value {
        Val::Int(i) => *i,
        _ => return Err("gzseek(): Argument #2 ($offset) must be of type int".into()),
    };

    let whence = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => *i as i32,
            _ => 0, // SEEK_SET
        }
    } else {
        0 // SEEK_SET
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzseek(): Invalid resource")?;

    let seek_from = match whence {
        0 => std::io::SeekFrom::Start(offset as u64),
        1 => std::io::SeekFrom::Current(offset),
        2 => std::io::SeekFrom::End(offset),
        _ => return Ok(vm.arena.alloc(Val::Int(-1))),
    };

    let result = match gz_file.inner.borrow_mut().seek(seek_from) {
        Ok(_) => Ok(vm.arena.alloc(Val::Int(0))),
        Err(_) => Ok(vm.arena.alloc(Val::Int(-1))),
    };
    result
}

/// gzrewind(resource $stream): bool
pub fn php_gzrewind(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gzrewind() expects 1 parameter".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzrewind(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzrewind(): Invalid resource")?;

    let result = match gz_file.inner.borrow_mut().seek(std::io::SeekFrom::Start(0)) {
        Ok(_) => Ok(vm.arena.alloc(Val::Bool(true))),
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    };
    result
}

/// gzgets(resource $stream, ?int $length = null): string|false
pub fn php_gzgets(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 {
        return Err("gzgets() expects 1 or 2 parameters".into());
    }

    let length = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i as usize,
            _ => 1024,
        }
    } else {
        1024
    };

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzgets(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzgets(): Invalid resource")?;

    let line = gz_file
        .inner
        .borrow_mut()
        .gets(length)
        .map_err(|e| e.to_string())?;
    if line.is_empty() && gz_file.inner.borrow_mut().eof() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(line))))
}

/// gzgetc(resource $stream): string|false
pub fn php_gzgetc(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gzgetc() expects 1 parameter".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzgetc(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzgetc(): Invalid resource")?;

    let mut byte = [0u8; 1];
    let n = gz_file
        .inner
        .borrow_mut()
        .read(&mut byte)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(byte.to_vec()))))
}

/// gzpassthru(resource $stream): int
pub fn php_gzpassthru(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gzpassthru() expects 1 parameter".into());
    }

    let resource = match &vm.arena.get(args[0]).value {
        Val::Resource(r) => r.clone(),
        _ => return Err("gzpassthru(): Argument #1 ($stream) must be of type resource".into()),
    };

    let gz_file = resource
        .downcast_ref::<GzFile>()
        .ok_or("gzpassthru(): Invalid resource")?;

    let mut total = 0;
    let mut buf = [0u8; 8192];
    loop {
        let n = gz_file
            .inner
            .borrow_mut()
            .read(&mut buf)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        print!("{}", String::from_utf8_lossy(&buf[..n]));
        total += n;
    }

    Ok(vm.arena.alloc(Val::Int(total as i64)))
}

/// readgzfile(string $filename, int $use_include_path = 0): int|false
pub fn php_readgzfile(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("readgzfile() expects 1 or 2 parameters".into());
    }

    let mode_handle = vm.arena.alloc(Val::String(Rc::new(b"rb".to_vec())));
    let gz_handle = php_gzopen(vm, &[args[0], mode_handle])?;

    if let Val::Bool(false) = vm.arena.get(gz_handle).value {
        return Ok(gz_handle);
    }

    let result = php_gzpassthru(vm, &[gz_handle]);
    let _ = php_gzclose(vm, &[gz_handle]);

    result
}

/// gzfile(string $filename, int $use_include_path = 0): array|false
pub fn php_gzfile(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("gzfile() expects 1 or 2 parameters".into());
    }

    let mode_handle = vm.arena.alloc(Val::String(Rc::new(b"rb".to_vec())));
    let gz_handle = php_gzopen(vm, &[args[0], mode_handle])?;

    if let Val::Bool(false) = vm.arena.get(gz_handle).value {
        return Ok(gz_handle);
    }

    let mut lines = ArrayData::new();
    loop {
        let line_handle = php_gzgets(vm, &[gz_handle])?;
        match &vm.arena.get(line_handle).value {
            Val::String(_) => {
                lines.push(line_handle);
            }
            _ => break,
        }
    }

    let _ = php_gzclose(vm, &[gz_handle]);

    Ok(vm.arena.alloc(Val::Array(Rc::new(lines))))
}

/// ob_gzhandler(string $data, int $mode): string|false
pub fn php_ob_gzhandler(_vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("ob_gzhandler() expects 2 parameters".into());
    }
    // Simplified: just return data for now
    Ok(args[0])
}

/// zlib_get_coding_type(): string|false
pub fn php_zlib_get_coding_type(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}
