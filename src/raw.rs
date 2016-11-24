// Copyright 2016 The RLS Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use AnalysisLoader;
use listings::{DirectoryListing, ListingKind};

use rustc_serialize::json;

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(RustcDecodable, Debug)]
pub struct Analysis {
    pub kind: Format,
    pub prelude: Option<CratePreludeData>,
    pub imports: Vec<Import>,
    pub defs: Vec<Def>,
    pub refs: Vec<Ref>,
    pub macro_refs: Vec<MacroRef>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Release,
    Debug,
}

#[derive(RustcDecodable, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
    JsonApi,
}

pub struct Crate {
    pub analysis: Analysis,
    pub timestamp: SystemTime,
    pub path: PathBuf,
}

impl Crate {
    fn new(analysis: Analysis, timestamp: SystemTime, path: PathBuf) -> Crate {
        Crate {
            analysis: analysis,
            timestamp: timestamp,
            path: path
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Target::Release => write!(f, "release"),
            Target::Debug => write!(f, "debug"),
        }
    }
}

impl Analysis {
    pub fn read_incremental<L: AnalysisLoader>(loader: &L,
                                               timestamps: HashMap<PathBuf, Option<SystemTime>>)
                                               -> Vec<Crate> {
        loader.iter_paths(|p| {
            use std::time::*;

            let t = Instant::now();

            let mut result = vec![];

            let listing = match DirectoryListing::from_path(p) {
                Ok(l) => l,
                Err(_) => { return result; },
            };

            for l in listing.files {
                info!{"Considering {:?}", l}
                if let ListingKind::File(ref time) = l.kind {
                    let mut path = p.to_path_buf();
                    path.push(&l.name);

                    match timestamps.get(&path) {
                        Some(&Some(ref t)) => {
                            if time > t {
                                Self::read_crate_data(&path).map(|a| result.push(Crate::new(a, time.clone(), path)));
                            }
                        }
                        // A crate we should never need to refresh.
                        Some(&None) => {}
                        // A crate we've never seen before.
                        None => {
                            Self::read_crate_data(&path).map(|a| result.push(Crate::new(a, time.clone(), path)));
                        }
                    }
                }
            }

            let _d = t.elapsed();
            // println!("reading {} crates from {} in {}.{:09}s", result.len(), p.display(), _d.as_secs(), _d.subsec_nanos());

            return result;
        })
    }

    pub fn read<L: AnalysisLoader>(loader: &L) -> Vec<Crate> {
        Self::read_incremental(loader, HashMap::new())
    }

    fn read_crate_data(path: &Path) -> Option<Analysis> {
        info!("read_crate_data {:?}", path);
        // TODO unwraps
        let mut file = File::open(&path).unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        json::decode(&buf).ok()
    }
}

#[derive(RustcDecodable, Debug)]
pub struct CompilerId {
    pub krate: u32,
    pub index: u32,
}

#[derive(RustcDecodable, Debug)]
pub struct CratePreludeData {
    pub crate_name: String,
    pub crate_root: String,
    pub external_crates: Vec<ExternalCrateData>,
    pub span: SpanData,
}

#[derive(RustcDecodable, Debug)]
pub struct ExternalCrateData {
    pub name: String,
    pub num: u32,
    pub file_name: String,
}

#[derive(RustcDecodable, Debug)]
pub struct Def {
    pub kind: DefKind,
    pub id: CompilerId,
    pub span: SpanData,
    pub name: String,
    pub qualname: String,
    pub parent: Option<CompilerId>,
    pub children: Option<Vec<CompilerId>>,
    pub value: String,
    pub docs: String,
    pub sig: Option<Signature>,
}

#[derive(RustcDecodable, Debug, Eq, PartialEq, Clone, Copy)]
pub enum DefKind {
    Enum,
    Tuple,
    Struct,
    Trait,
    Function,
    Method,
    Macro,
    Mod,
    Type,
    Local,
    Static,
    Const,
    Field,
    Import,
}

impl DefKind {
    pub fn name_space(&self) -> char {
        match *self {
            DefKind::Enum |
            DefKind::Tuple |
            DefKind::Struct |
            DefKind::Type |
            DefKind::Trait => 't',
            DefKind::Function |
            DefKind::Method |
            DefKind::Mod |
            DefKind::Local |
            DefKind::Static |
            DefKind::Const |
            DefKind::Field => 'v',
            DefKind::Macro => 'm',
            DefKind::Import => { panic!("No namespace for imports"); }
        }
    }
}

#[derive(RustcDecodable, Debug)]
pub struct Signature {
    pub span: SpanData,
    pub text: String,
    pub ident_start: usize,
    pub ident_end: usize,
    pub defs: Vec<SigElement>,
    pub refs: Vec<SigElement>,
}

#[derive(RustcDecodable, Debug)]
pub struct SigElement {
    pub id: CompilerId,
    pub start: usize,
    pub end: usize,
}

#[derive(RustcDecodable, Debug)]
pub struct Ref {
    pub kind: RefKind,
    pub span: SpanData,
    pub ref_id: CompilerId,
}

#[derive(RustcDecodable, Debug)]
pub enum RefKind {
    Function,
    Mod,
    Type,
    Variable,
}

#[derive(RustcDecodable, Debug)]
pub struct MacroRef {
    pub span: SpanData,
    pub qualname: String,
    pub callee_span: SpanData,
}

#[derive(RustcDecodable, Debug)]
pub struct Import {
    pub kind: ImportKind,
    pub ref_id: Option<CompilerId>,
    pub span: SpanData,
    pub name: String,
    pub value: String,
}

#[derive(RustcDecodable, Debug)]
pub enum ImportKind {
    ExternCrate,
    Use,
    GlobUse,
}

#[derive(RustcDecodable, Debug, Clone)]
pub struct SpanData {
    pub file_name: PathBuf,
    pub byte_start: u32,
    pub byte_end: u32,
    /// 1-based.
    pub line_start: usize,
    pub line_end: usize,
    /// 1-based, character offset.
    pub column_start: usize,
    pub column_end: usize,
}
