use ra_db::{SourceRootId, LocationIntener, Cancelable, FileId};
use ra_syntax::{SourceFileNode, SyntaxKind, SyntaxNode, SyntaxNodeRef, SourceFile, AstNode, ast};

use crate::{
    HirDatabase, PerNs, ModuleId, Module, Def, Function, Struct, Enum,
    arena::{Arena, Id},
};

/// hir makes a heavy use of ids: integer (u32) handlers to various things. You
/// can think of id as a pointer (but without a lifetime) or a file descriptor
/// (but for hir objects).
///
/// This module defines a bunch of ids we are using. The most important ones are
/// probably `HirFileId` and `DefId`.

/// Input to the analyzer is a set of file, where each file is indetified by
/// `FileId` and contains source code. However, another source of source code in
/// Rust are macros: each macro can be thought of as producing a "temporary
/// file". To assign id to such file, we use the id of a macro call that
/// produced the file. So, a `HirFileId` is either a `FileId` (source code
/// written by user), or a `MacroCallId` (source code produced by macro).
///
/// What is a `MacroCallId`? Simplifying, it's a `HirFileId` of a file containin
/// the call plus the offset of the macro call in the file. Note that this is a
/// recursive definition! Nethetheless, size_of of `HirFileId` is finite
/// (because everything bottoms out at the real `FileId`) and small
/// (`MacroCallId` uses location interner).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirFileId(HirFileIdRepr);

impl HirFileId {
    /// For macro-expansion files, returns the file original source file the
    /// expansionoriginated from.
    pub(crate) fn original_file(self, db: &impl HirDatabase) -> FileId {
        match self.0 {
            HirFileIdRepr::File(file_id) => file_id,
            HirFileIdRepr::Macro(macro_call_id) => {
                let loc = macro_call_id.loc(db);
                loc.source_item_id.file_id.original_file(db)
            }
        }
    }

    pub(crate) fn as_original_file(self) -> FileId {
        match self.0 {
            HirFileIdRepr::File(file_id) => file_id,
            HirFileIdRepr::Macro(_r) => panic!("macro generated file: {:?}", self),
        }
    }

    pub(crate) fn as_macro_call_id(self) -> Option<MacroCallId> {
        match self.0 {
            HirFileIdRepr::Macro(it) => Some(it),
            _ => None,
        }
    }

    pub(crate) fn hir_source_file(db: &impl HirDatabase, file_id: HirFileId) -> SourceFileNode {
        match file_id.0 {
            HirFileIdRepr::File(file_id) => db.source_file(file_id),
            HirFileIdRepr::Macro(m) => {
                if let Some(exp) = db.expand_macro_invocation(m) {
                    return exp.file();
                }
                // returning an empty string looks fishy...
                SourceFileNode::parse("")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HirFileIdRepr {
    File(FileId),
    Macro(MacroCallId),
}

impl From<FileId> for HirFileId {
    fn from(file_id: FileId) -> HirFileId {
        HirFileId(HirFileIdRepr::File(file_id))
    }
}

impl From<MacroCallId> for HirFileId {
    fn from(macro_call_id: MacroCallId) -> HirFileId {
        HirFileId(HirFileIdRepr::Macro(macro_call_id))
    }
}

/// `MacroCallId` identifies a particular macro invocation, like
/// `println!("Hello, {}", world)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroCallId(u32);
ra_db::impl_numeric_id!(MacroCallId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroCallLoc {
    pub(crate) source_root_id: SourceRootId,
    pub(crate) module_id: ModuleId,
    pub(crate) source_item_id: SourceItemId,
}

impl MacroCallId {
    pub(crate) fn loc(
        self,
        db: &impl AsRef<LocationIntener<MacroCallLoc, MacroCallId>>,
    ) -> MacroCallLoc {
        db.as_ref().id2loc(self)
    }
}

impl MacroCallLoc {
    #[allow(unused)]
    pub(crate) fn id(
        &self,
        db: &impl AsRef<LocationIntener<MacroCallLoc, MacroCallId>>,
    ) -> MacroCallId {
        db.as_ref().loc2id(&self)
    }
}

/// Def's are a core concept of hir. A `Def` is an Item (function, module, etc)
/// in a specific module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(pub(crate) u32);
ra_db::impl_numeric_id!(DefId);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DefLoc {
    pub(crate) kind: DefKind,
    pub(crate) source_root_id: SourceRootId,
    pub(crate) module_id: ModuleId,
    pub(crate) source_item_id: SourceItemId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DefKind {
    Module,
    Function,
    Struct,
    Enum,
    Item,

    StructCtor,
}

impl DefId {
    pub(crate) fn loc(self, db: &impl AsRef<LocationIntener<DefLoc, DefId>>) -> DefLoc {
        db.as_ref().id2loc(self)
    }

    pub fn resolve(self, db: &impl HirDatabase) -> Cancelable<Def> {
        let loc = self.loc(db);
        let res = match loc.kind {
            DefKind::Module => {
                let module = Module::new(db, loc.source_root_id, loc.module_id)?;
                Def::Module(module)
            }
            DefKind::Function => {
                let function = Function::new(self);
                Def::Function(function)
            }
            DefKind::Struct => {
                let struct_def = Struct::new(self);
                Def::Struct(struct_def)
            }
            DefKind::Enum => {
                let enum_def = Enum::new(self);
                Def::Enum(enum_def)
            }
            DefKind::StructCtor => Def::Item,
            DefKind::Item => Def::Item,
        };
        Ok(res)
    }

    /// For a module, returns that module; for any other def, returns the containing module.
    pub fn module(self, db: &impl HirDatabase) -> Cancelable<Module> {
        let loc = self.loc(db);
        Module::new(db, loc.source_root_id, loc.module_id)
    }
}

impl DefLoc {
    pub(crate) fn id(&self, db: &impl AsRef<LocationIntener<DefLoc, DefId>>) -> DefId {
        db.as_ref().loc2id(&self)
    }
}

impl DefKind {
    pub(crate) fn for_syntax_kind(kind: SyntaxKind) -> PerNs<DefKind> {
        match kind {
            SyntaxKind::FN_DEF => PerNs::values(DefKind::Function),
            SyntaxKind::MODULE => PerNs::types(DefKind::Module),
            SyntaxKind::STRUCT_DEF => PerNs::both(DefKind::Struct, DefKind::StructCtor),
            SyntaxKind::ENUM_DEF => PerNs::types(DefKind::Enum),
            // These define items, but don't have their own DefKinds yet:
            SyntaxKind::TRAIT_DEF => PerNs::types(DefKind::Item),
            SyntaxKind::TYPE_DEF => PerNs::types(DefKind::Item),
            SyntaxKind::CONST_DEF => PerNs::values(DefKind::Item),
            SyntaxKind::STATIC_DEF => PerNs::values(DefKind::Item),
            _ => PerNs::none(),
        }
    }
}

/// Identifier of item within a specific file. This is stable over reparses, so
/// it's OK to use it as a salsa key/value.
pub(crate) type SourceFileItemId = Id<SyntaxNode>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceItemId {
    pub(crate) file_id: HirFileId,
    /// None for the whole file.
    pub(crate) item_id: Option<SourceFileItemId>,
}

/// Maps item's `SyntaxNode`s to `SourceFileItemId` and back.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceFileItems {
    file_id: HirFileId,
    arena: Arena<SyntaxNode>,
}

impl SourceFileItems {
    pub(crate) fn new(file_id: HirFileId, source_file: SourceFile) -> SourceFileItems {
        let mut res = SourceFileItems {
            file_id,
            arena: Arena::default(),
        };
        res.init(source_file);
        res
    }

    fn init(&mut self, source_file: SourceFile) {
        source_file.syntax().descendants().for_each(|it| {
            if let Some(module_item) = ast::ModuleItem::cast(it) {
                self.alloc(module_item.syntax().owned());
            } else if let Some(macro_call) = ast::MacroCall::cast(it) {
                self.alloc(macro_call.syntax().owned());
            }
        });
    }

    fn alloc(&mut self, item: SyntaxNode) -> SourceFileItemId {
        self.arena.alloc(item)
    }
    pub(crate) fn id_of(&self, file_id: HirFileId, item: SyntaxNodeRef) -> SourceFileItemId {
        assert_eq!(
            self.file_id, file_id,
            "SourceFileItems: wrong file, expected {:?}, got {:?}",
            self.file_id, file_id
        );
        self.id_of_unchecked(item)
    }
    pub(crate) fn id_of_unchecked(&self, item: SyntaxNodeRef) -> SourceFileItemId {
        if let Some((id, _)) = self.arena.iter().find(|(_id, i)| i.borrowed() == item) {
            return id;
        }
        // This should not happen. Let's try to give a sensible diagnostics.
        if let Some((id, i)) = self.arena.iter().find(|(_id, i)| i.range() == item.range()) {
            // FIXME(#288): whyyy are we getting here?
            log::error!(
                "unequal syntax nodes with the same range:\n{:?}\n{:?}",
                item,
                i
            );
            return id;
        }
        panic!(
            "Can't find {:?} in SourceFileItems:\n{:?}",
            item,
            self.arena.iter().map(|(_id, i)| i).collect::<Vec<_>>(),
        );
    }
    pub fn id_of_source_file(&self) -> SourceFileItemId {
        let (id, _syntax) = self.arena.iter().next().unwrap();
        id
    }
}

impl std::ops::Index<SourceFileItemId> for SourceFileItems {
    type Output = SyntaxNode;
    fn index(&self, idx: SourceFileItemId) -> &SyntaxNode {
        &self.arena[idx]
    }
}
