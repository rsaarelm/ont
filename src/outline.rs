use std::fmt;

use anyhow::Result;
use derive_more::{Deref, DerefMut};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::parse;

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
        let head = head.into();
        assert!(head.chars().all(|c| c != '\n'));
        Section { head, body }
    }

    pub fn is_important(&self) -> bool {
        self.head.ends_with(" *")
    }

    pub fn wiki_title(&self) -> Option<&str> {
        let head = parse::important(&self.head).unwrap_or(&self.head);
        parse::wiki_word(head)
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

impl FromIterator<Section> for Outline {
    fn from_iter<I: IntoIterator<Item = Section>>(iter: I) -> Self {
        Outline {
            attrs: IndexMap::new(),
            children: iter.into_iter().collect(),
        }
    }
}

impl Outline {
    pub fn new(
        attrs: IndexMap<String, String>,
        children: Vec<Section>,
    ) -> Self {
        Outline { attrs, children }
    }

    /// Iterate mutable recursive sections of the outline.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Section> {
        self.context_iter_mut(()).map(|x| x.1)
    }

    /// Iterate mutable recursive sections of the outline with a context
    /// object that is passed to child sections.
    pub fn context_iter_mut<C: Clone>(
        &mut self,
        init: C,
    ) -> ContextIterMut<'_, C> {
        ContextIterMut::new(self, init)
    }

    /// Iterate recursive sections of the outline.
    pub fn iter(&self) -> impl Iterator<Item = &Section> {
        self.context_iter(()).map(|x| x.1)
    }

    /// Iterate recursive sections of the outline with a context object that
    /// is passed to child sections.
    pub fn context_iter<C: Clone>(&self, init: C) -> ContextIter<'_, C> {
        ContextIter::new(self, init)
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

    /// Get an attribute as a handle that gets written back to the outline as
    /// it drops out of scope.
    pub fn get_mut<'a, T: DeserializeOwned + Serialize>(
        &'a mut self,
        name: &str,
    ) -> Result<Option<FieldHandle<'a, T>>> {
        let inner = self.get(name)?;
        let Some(inner) = inner else { return Ok(None) };

        Ok(Some(FieldHandle {
            inner,
            name: name.to_owned(),
            parent: self,
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty() && self.children.is_empty()
    }

    pub fn set<T: Serialize>(&mut self, name: &str, value: &T) -> Result<()> {
        self.attrs.insert(name.to_owned(), idm::to_string(value)?);
        Ok(())
    }

    pub fn uris(&self) -> Result<Vec<String>> {
        let mut ret = Vec::new();

        if let Some(uri) = self.attrs.get("uri") {
            ret.push(uri.clone());
        } else {
            // Must have the initial uri or it doesn't count.
            return Ok(ret);
        }

        if let Some(seq) = self.get::<Vec<String>>("sequence")? {
            ret.extend(seq);
        }

        Ok(ret)
    }

    pub fn push(&mut self, s: Section) {
        self.children.push(s);
    }

    pub fn push_line(&mut self, s: impl Into<String>) {
        self.children.push(Section::new(s, Default::default()));
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

#[derive(Deref, DerefMut)]
pub struct FieldHandle<'a, T: Serialize> {
    #[deref]
    #[deref_mut]
    inner: T,
    name: String,
    parent: &'a mut Outline,
}

impl<T: Serialize> Drop for FieldHandle<'_, T> {
    fn drop(&mut self) {
        self.parent
            .set(&self.name, &self.inner)
            .expect("FieldHandle: Failed to set");
    }
}

pub struct ContextIterMut<'a, C> {
    // (context-value, pointer-to-outline, current-item)
    stack: Vec<(C, *mut Outline, usize)>,
    phantom: std::marker::PhantomData<&'a Section>,
}

impl<'a, C: Clone> ContextIterMut<'a, C> {
    fn new(outline: &'a mut Outline, init: C) -> Self {
        let stack = vec![(init, outline as *mut Outline, 0)];
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
            let (_, outline, i) = self.stack[self.stack.len() - 1];
            if i >= unsafe { (*outline).children.len() } {
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

        // Clone current context object. The clone is pushed to next stack
        // layer and passed as mutable pointer to the iterating context.
        // Context changes will show up in children.
        let ctx = self.stack[len - 1].0.clone();

        // Get index of next item to yield and increment index value on stack.
        let idx = self.stack[len - 1].2;
        self.stack[len - 1].2 += 1;

        unsafe {
            let current_item = &mut (*self.stack[len - 1].1).children[idx];

            // Add children of current item to stack.
            self.stack
                .push((ctx, &mut current_item.body as *mut Outline, 0));

            // Take a mutable pointer to the new context object passed to the
            // child range and yield it along with current item. "len" is now
            // a valid index since we have pushed a new item to the stack.
            let ctx = &mut self.stack[len].0 as *mut C;

            Some((&mut *ctx, current_item))
        }
    }
}

pub struct ContextIter<'a, C> {
    // (context-value, pointer-to-outline, current-item)
    stack: Vec<(C, *const Outline, usize)>,
    phantom: std::marker::PhantomData<&'a Section>,
}

impl<'a, C: Clone> ContextIter<'a, C> {
    fn new(outline: &'a Outline, init: C) -> Self {
        let stack = vec![(init, outline as *const Outline, 0)];
        ContextIter {
            stack,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, C: Clone + 'a> Iterator for ContextIter<'a, C> {
    type Item = (&'a mut C, &'a Section);

    fn next(&mut self) -> Option<Self::Item> {
        // Remove completed ranges.
        while !self.stack.is_empty() {
            let (_, outline, i) = self.stack[self.stack.len() - 1];
            if i >= unsafe { (*outline).children.len() } {
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

        // Clone current context object. The clone is pushed to next stack
        // layer and passed as mutable pointer to the iterating context.
        // Context changes will show up in children.
        let ctx = self.stack[len - 1].0.clone();

        // Get index of next item to yield and increment index value on stack.
        let idx = self.stack[len - 1].2;
        self.stack[len - 1].2 += 1;

        unsafe {
            let current_item = &(*self.stack[len - 1].1).children[idx];

            // Add children of current item to stack.
            self.stack
                .push((ctx, &current_item.body as *const Outline, 0));

            // Take a mutable pointer to the new context object passed to the
            // child range and yield it along with current item. "len" is now
            // a valid index since we have pushed a new item to the stack.
            let ctx = &mut self.stack[len].0 as *mut C;

            Some((&mut *ctx, current_item))
        }
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
