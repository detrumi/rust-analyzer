//! FIXME: write short doc here

use std::fmt;

use crate::db::HirDatabase;

pub struct HirFormatter<'a, 'b, DB> {
    pub db: &'a DB,
    fmt: &'a mut fmt::Formatter<'b>,
    level: u32,
}

pub trait HirDisplay {
    fn hir_fmt(&self, f: &mut HirFormatter<impl HirDatabase>) -> fmt::Result;
    fn display<'a, DB>(&'a self, db: &'a DB) -> HirDisplayWrapper<'a, DB, Self>
    where
        Self: Sized,
    {
        self.display_compact(db, std::u32::MAX)
    }

    fn display_compact<'a, DB>(&'a self, db: &'a DB, levels: u32) -> HirDisplayWrapper<'a, DB, Self>
    where
        Self: Sized,
    {
        HirDisplayWrapper(db, self, levels)
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
    ) -> fmt::Result {
        let mut first = true;
        for e in iter {
            if !first {
                write!(self, "{}", sep)?;
            }
            first = false;
            e.hir_fmt(self)?;
        }
        Ok(())
    }

    pub fn write_nested<F>(&mut self, nested: F) -> fmt::Result
    where
        F: FnOnce(&mut Self) -> fmt::Result,
    {
        if self.level > 0 {
            self.level -= 1;
            nested(self)?;
            self.level += 1;
        } else {
            write!(self, "…")?;
        }
        Ok(())
    }

    pub fn write_nested_joined<T: HirDisplay>(
        &mut self,
        iter: impl IntoIterator<Item = T>,
        sep: &str,
    ) -> fmt::Result {
        self.write_nested(|f| f.write_joined(iter, sep))
    }

    /// This allows using the `write!` macro directly with a `HirFormatter`.
    pub fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self.fmt, args)
    }
}

pub struct HirDisplayWrapper<'a, DB, T>(&'a DB, &'a T, u32);

impl<'a, DB, T> fmt::Display for HirDisplayWrapper<'a, DB, T>
where
    DB: HirDatabase,
    T: HirDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.1.hir_fmt(&mut HirFormatter { db: self.0, level: self.2, fmt: f })
    }
}
