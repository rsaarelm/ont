use std::{fmt, fs, path::Path};

use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

mod collection;
pub use collection::{read_directory, write_directory};

/// An element of an outline with a single headline and nested contents.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
// Serialize using a special form that triggers IDM's raw mode.
#[serde(from = "((String,), Outline)", into = "((String,), Outline)")]
pub struct Section {
    /// First line of the section.
    pub head: String,
    /// Indented outline block under the section head.
    pub body: Outline,
}

impl Section {
    pub fn new(head: impl Into<String>, body: Outline) -> Self {
        Section {
            head: head.into(),
            body,
        }
    }
}

impl From<((String,), Outline)> for Section {
    fn from(((head,), body): ((String,), Outline)) -> Self {
        Section { head, body }
    }
}

impl From<Section> for ((String,), Outline) {
    fn from(val: Section) -> Self {
        ((val.head,), val.body)
    }
}

/// An outline block with named attributes and child elements.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
// Serialize using a special form that triggers IDM's raw mode.
#[serde(
    from = "((IndexMap<String, String>,), Vec<Section>)",
    into = "((IndexMap<String, String>,), Vec<Section>)"
)]
pub struct Outline {
    /// Named attributes of the outline.
    pub attrs: IndexMap<String, String>,
    /// Contents of the outline.
    pub children: Vec<Section>,
}

impl Outline {
    pub fn new(
        attrs: IndexMap<String, String>,
        children: Vec<Section>,
    ) -> Self {
        Outline { attrs, children }
    }
}

impl From<((IndexMap<String, String>,), Vec<Section>)> for Outline {
    fn from(
        ((attrs,), children): ((IndexMap<String, String>,), Vec<Section>),
    ) -> Self {
        Outline { attrs, children }
    }
}

impl From<Outline> for ((IndexMap<String, String>,), Vec<Section>) {
    fn from(val: Outline) -> Self {
        ((val.attrs,), val.children)
    }
}

pub struct ContextIterMut<'a, C> {
    // (context-value, pointer-to-current-item, pointer-past-last-item)
    stack: Vec<(C, *mut Section, *mut Section)>,
    phantom: std::marker::PhantomData<&'a Section>,
}

impl<'a, C: Clone> ContextIterMut<'a, C> {
    fn new(outline: &'a mut Outline, init: C) -> Self {
        let stack = vec![unsafe {
            let a = outline.children.as_mut_ptr();
            let b = a.add(outline.children.len());
            (init, a, b)
        }];
        ContextIterMut {
            stack,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, C: Clone + 'a> Iterator for ContextIterMut<'a, C> {
    type Item = (&'a mut C, &'a mut Section);

    fn next(&mut self) -> Option<Self::Item> {
        // Remove completed ranges.
        while !self.stack.is_empty() {
            let (_, begin, end) = self.stack[self.stack.len() - 1];
            if begin == end {
                self.stack.pop();
            } else {
                break;
            }
        }

        // End iteration if no more content left.
        if self.stack.is_empty() {
            return None;
        }

        let len = self.stack.len();
        // Clone current context object. The clone is pushed for next stack
        // layer and passed as mutable pointer to the iterating context.
        // Context changes will show up in children.
        let ctx = self.stack[len - 1].0.clone();
        let begin = self.stack[len - 1].1;

        // Safety analysis statement: I dunno lol
        unsafe {
            let children = &mut (*begin).body.children;
            self.stack[len - 1].1 = begin.add(1);
            let a = children.as_mut_ptr();
            let b = a.add(children.len());
            self.stack.push((ctx, a, b));
            let ctx = &mut self.stack[len].0 as *mut C;

            Some((&mut *ctx, &mut *begin))
        }
    }
}

impl Outline {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Section> {
        self.context_iter_mut(()).map(|x| x.1)
    }

    pub fn context_iter_mut<C: Clone>(
        &mut self,
        init: C,
    ) -> ContextIterMut<'_, C> {
        ContextIterMut::new(self, init)
    }

    /// Get an attribute value deserialized to type.
    pub fn get<'a, T: Deserialize<'a>>(
        &'a self,
        name: &str,
    ) -> Result<Option<T>> {
        let Some(a) = self.attrs.get(name) else {
            return Ok(None);
        };
        Ok(Some(idm::from_str(a)?))
    }

    pub fn set<T: Serialize>(&mut self, name: &str, value: &T) -> Result<()> {
        self.attrs.insert(name.to_owned(), idm::to_string(value)?);
        Ok(())
    }
}

impl fmt::Display for Outline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(
            f: &mut fmt::Formatter<'_>,
            depth: usize,
            outline: &Outline,
        ) -> fmt::Result {
            for (k, v) in &outline.attrs {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                write!(f, ":{k}")?;

                if v.chars().any(|c| c == '\n') {
                    // If value is multi-line, write it indented under the key.
                    writeln!(f)?;
                    for line in v.lines() {
                        for _ in 0..(depth + 1) {
                            write!(f, "  ")?;
                        }
                        writeln!(f, "{line}")?;
                    }
                } else {
                    // Otherwise write the value inline.
                    writeln!(f, " {v}")?;
                }
            }

            for section in &outline.children {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                writeln!(f, "{}", section.head)?;
                print(f, depth + 1, &section.body)?;
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

pub type SimpleSection = ((String,), SimpleOutline);

/// An outline without a separately parsed header section.
///
/// Most general form of parsing an IDM document.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimpleOutline(pub Vec<SimpleSection>);

impl fmt::Display for SimpleOutline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(
            f: &mut fmt::Formatter<'_>,
            depth: usize,
            outline: &SimpleOutline,
        ) -> fmt::Result {
            for ((head,), body) in &outline.0 {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                writeln!(f, "{head}")?;
                print(f, depth + 1, body)?;
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

/// Delete file at `path` and any empty subdirectories between `root` and
/// `path`.
///
/// This is a git-style file delete that deletes the containing subdirectory
/// if it's emptied of files.
pub fn tidy_delete(root: &Path, mut path: &Path) -> Result<()> {
    fs::remove_file(path)?;
    log::debug!("tidy_delete: Deleted {path:?}");

    while let Some(parent) = path.parent() {
        // Keep going up the subdirectories...
        path = parent;

        // ...that are underneath `root`...
        if !path.starts_with(root)
            || path.components().count() <= root.components().count()
        {
            break;
        }

        // ...and deleting them if you can.
        if fs::remove_dir(path).is_ok() {
            log::debug!("tidy_delete: Deleted empty subdirectory {path:?}");
        } else {
            // We couldn't delete the directory, assume it wasn't empty and
            // that there's no point going further up the tree.
            break;
        }
    }

    Ok(())
}
