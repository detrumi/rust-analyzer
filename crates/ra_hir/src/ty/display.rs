//! FIXME: write short doc here

use std::fmt;
use std::sync::Arc;

use crate::db::HirDatabase;

pub type Result = std::result::Result<HirDisplayTree, std::fmt::Error>;

pub enum HirDisplayTree {
    Collapsible { prefix: String, expanded: Arc<HirDisplayTree>, suffix: String },
    Sequence(Vec<Arc<HirDisplayTree>>),
    Text(String),
}

impl HirDisplayTree {
    pub fn text(text: &str) -> Self {
        Self::Text(String::from(text))
    }

    pub fn display(d: impl std::fmt::Display) -> Self {
        Self::Text(format!("{}", d))
    }

    pub fn collapsible(prefix: &str, expanded: Self, suffix: &str) -> Self {
        HirDisplayTree::Collapsible {
            prefix: String::from(prefix),
            expanded: Arc::new(expanded),
            suffix: String::from(suffix),
        }
    }

    pub fn sequence() -> Self {
        Self::Sequence(vec![])
    }

    pub fn push(mut self, child: Self) -> Self {
        match self {
            Self::Sequence(ref mut seq) => seq.push(Arc::new(child)),
            _ => return Self::sequence().push(self).push(child),
        }
        self
    }

    pub fn push_str(mut self, str: &str) -> Self {
        match self {
            Self::Sequence(ref mut seq) => seq.push(Arc::new(Self::Text(String::from(str)))),
            _ => return Self::sequence().push(self).push_str(str),
        }
        self
    }

    pub fn push_display(mut self, d: impl std::fmt::Display) -> Self {
        match self {
            Self::Sequence(ref mut seq) => seq.push(Arc::new(Self::Text(format!("{}", d)))),
            _ => return Self::sequence().push(self).push_display(d),
        }
        self
    }
}

impl fmt::Display for HirDisplayTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO move to different function to allow controlling collapsing
        match self {
            HirDisplayTree::Collapsible { prefix, expanded, suffix } => {
                write!(f, "{}{}{}", prefix, expanded, suffix)
            }
            HirDisplayTree::Sequence(seq) => {
                for d in seq {
                    write!(f, "{}", d)?;
                }
                Ok(())
            }
            HirDisplayTree::Text(text) => write!(f, "{}", text),
        }
    }
}

pub struct HirFormatter<'a, 'b, DB> {
    pub db: &'a DB,
    fmt: &'a mut fmt::Formatter<'b>,
}

pub trait HirDisplay {
    fn hir_fmt(&self, f: &mut HirFormatter<impl HirDatabase>) -> Result;
    fn display<'a, DB>(&'a self, db: &'a DB) -> HirDisplayWrapper<'a, DB, Self>
    where
        Self: Sized,
    {
        HirDisplayWrapper(db, self)
    }
}

impl<'a, 'b, DB> HirFormatter<'a, 'b, DB>
where
    DB: HirDatabase,
{
    pub fn write_joined<T: HirDisplay>(
        &mut self,
        iter: impl IntoIterator<Item = T>,
        sep: &str,
    ) -> Result {
        let mut result = HirDisplayTree::sequence();
        let mut first = true;
        for e in iter {
            if !first {
                result = result.push_str(sep);
            }
            first = false;
            result = result.push(e.hir_fmt(self)?);
        }
        Ok(result)
    }

    /// This allows using the `write!` macro directly with a `HirFormatter`.
    pub fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self.fmt, args)
    }
}

pub struct HirDisplayWrapper<'a, DB, T>(&'a DB, &'a T);

impl<'a, DB, T> fmt::Display for HirDisplayWrapper<'a, DB, T>
where
    DB: HirDatabase,
    T: HirDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = self.1.hir_fmt(&mut HirFormatter { db: self.0, fmt: f })?;
        write!(f, "{}", result)
    }
}
