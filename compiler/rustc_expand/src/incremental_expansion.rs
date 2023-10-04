use std::path::{Path, PathBuf};

use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::fx::FxHashMap;
use rustc_serialize::opaque::FileEncoder;
use rustc_serialize::{Decodable, Encodable};
use rustc_session::Session;

use crate::expand::AstFragment;

pub struct IncrementalExpander {
    persist_path: Option<PathBuf>,
    cache: Option<MacroIncrementalData>,
}

impl IncrementalExpander {
    pub fn new(sess: &Session) -> IncrementalExpander {
        let persist_path = if sess.opts.unstable_opts.incremental_macro_expansion {
            sess.incr_comp_session_dir_opt().map(|path| path.join("macro_expansion.bin"))
        } else {
            None
        };

        IncrementalExpander { persist_path, cache: None }
    }

    pub fn initialise(&mut self, sess: &Session) {
        if self.cache.is_none() {
            self.cache =
                self.persist_path.as_ref().map(|path| initialise_macro_cache(path.as_path(), sess));
        }
    }

    pub fn save_cache(&self) -> std::io::Result<()> {
        if let (Some(cache), Some(output_path)) = (&self.cache, &self.persist_path) {
            let mut encoder = FileEncoder::new(output_path)?;

            cache.encode(&mut encoder);

            let _ = encoder.finish()?;

            tracing::debug!(?output_path, "Saved incremental macro cache");
        }

        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        // This path only exists *if* the unstable feature is enabled _and_ there is an incremental cache for this crate.
        self.persist_path.is_some()
    }

    pub fn cache(&mut self) -> &mut MacroIncrementalData {
        self.cache.as_mut().expect("Need to initialise incremental data first")
    }
}

fn initialise_macro_cache(persist_path: &Path, sess: &Session) -> MacroIncrementalData {
    if let Ok(file_data) = std::fs::read(&persist_path) {
        tracing::debug!(cache_path=?persist_path, "Loading incremental cache");
        let mut decoder = incremental_decoder::IncrementalMacroDecoder::new(&sess, &file_data);

        MacroIncrementalData::decode(&mut decoder)
    } else {
        tracing::debug!("No incremental cache present, skipping");

        MacroIncrementalData::default()
    }
}

#[derive(Encodable, Decodable, Default)]
pub struct MacroIncrementalData {
    pub span_map: FxHashMap<String, SpanMapContent>,
}

#[derive(Encodable, Decodable, Default)]
pub struct SpanMapContent {
    /// A map from the hash of the input content to the expanded `AstFragment`
    pub ast_map: FxHashMap<Fingerprint, AstFragment>,
}

mod incremental_decoder {
    use rustc_ast::ast;
    use rustc_ast::tokenstream::{AttrTokenStream, LazyAttrTokenStream};
    use rustc_serialize::opaque::MemDecoder;
    use rustc_serialize::{Decodable, Decoder};
    use rustc_session::Session;

    pub struct IncrementalMacroDecoder<'a> {
        decoder: MemDecoder<'a>,
        sess: &'a Session,
    }

    impl<'a> IncrementalMacroDecoder<'a> {
        pub fn new(sess: &'a Session, data: &'a [u8]) -> IncrementalMacroDecoder<'a> {
            IncrementalMacroDecoder { decoder: MemDecoder::new(data, 0), sess }
        }
    }

    impl<'a> Decodable<IncrementalMacroDecoder<'a>> for ast::AttrId {
        #[inline]
        fn decode(d: &mut IncrementalMacroDecoder<'a>) -> ast::AttrId {
            d.sess.parse_sess.attr_id_generator.mk_attr_id()
        }
    }

    impl<'a> Decodable<IncrementalMacroDecoder<'a>> for LazyAttrTokenStream {
        #[inline]
        fn decode(d: &mut IncrementalMacroDecoder<'a>) -> Self {
            let inner: AttrTokenStream = AttrTokenStream::decode(d);
            LazyAttrTokenStream::new(inner)
        }
    }

    impl<'a> Decoder for IncrementalMacroDecoder<'a> {
        fn read_usize(&mut self) -> usize {
            self.decoder.read_usize()
        }

        fn read_u128(&mut self) -> u128 {
            self.decoder.read_u128()
        }

        fn read_u64(&mut self) -> u64 {
            self.decoder.read_u64()
        }

        fn read_u32(&mut self) -> u32 {
            self.decoder.read_u32()
        }

        fn read_u16(&mut self) -> u16 {
            self.decoder.read_u16()
        }

        fn read_u8(&mut self) -> u8 {
            self.decoder.read_u8()
        }

        fn read_isize(&mut self) -> isize {
            self.decoder.read_isize()
        }

        fn read_i128(&mut self) -> i128 {
            self.decoder.read_i128()
        }

        fn read_i64(&mut self) -> i64 {
            self.decoder.read_i64()
        }

        fn read_i32(&mut self) -> i32 {
            self.decoder.read_i32()
        }

        fn read_i16(&mut self) -> i16 {
            self.decoder.read_i16()
        }

        fn read_raw_bytes(&mut self, len: usize) -> &[u8] {
            self.decoder.read_raw_bytes(len)
        }

        fn peek_byte(&self) -> u8 {
            self.decoder.peek_byte()
        }

        fn position(&self) -> usize {
            self.decoder.position()
        }
    }
}

mod stable_hashing_ctx_expansion {
    use rustc_ast::{Attribute, HashStableContext};
    use rustc_data_structures::stable_hasher::{HashingControls, StableHasher};
    use rustc_data_structures::sync::Lrc;
    use rustc_span::def_id::{DefId, DefPathHash, LocalDefId};
    use rustc_span::{BytePos, SourceFile, Span, SpanData};

    pub struct StableHashCtx;

    impl HashStableContext for StableHashCtx {
        fn hash_attr(&mut self, _: &Attribute, _hasher: &mut StableHasher) {
            todo!()
        }
    }

    impl rustc_span::HashStableContext for StableHashCtx {
        fn def_path_hash(&self, _def_id: DefId) -> DefPathHash {
            todo!()
        }

        fn hash_spans(&self) -> bool {
            false
        }

        fn unstable_opts_incremental_ignore_spans(&self) -> bool {
            todo!()
        }

        fn def_span(&self, _def_id: LocalDefId) -> Span {
            todo!()
        }

        fn span_data_to_lines_and_cols(
            &mut self,
            _span: &SpanData,
        ) -> Option<(Lrc<SourceFile>, usize, BytePos, usize, BytePos)> {
            todo!()
        }

        fn hashing_controls(&self) -> HashingControls {
            todo!()
        }
    }
}
