use anyhow::Result;
use idm_tools::{Outline, Section};
use indexmap::IndexSet;

use crate::IoArgs;

pub fn run(io: IoArgs) -> Result<()> {
    let outline: Outline = idm::from_str(&io.read()?)?;

    // Construct the complete set of fields from the attribute fields in all
    // the toplevel items of the outline.
    let mut fields = IndexSet::new();

    // XXX: This mangles the order somewhat if the first items seen are
    // missing fields. A fancier version would try to fit the fields seen
    // later in correct positions so that any "well-formed" input that may
    // have fields missing in any item but always has the fields in the same
    // order will produce that same order here.
    for sec in &outline.children {
        for field in sec.body.attrs.keys() {
            fields.insert(field.clone());
        }
    }

    let mut columns = Outline::default();

    for field in &fields {
        let mut elt = Section::new(format!(":{}", field), Outline::default());
        for sec in &outline.children {
            let attrs = &sec.body.attrs;

            if attrs.is_empty() {
                continue;
            }

            // Value is "-" if `attrs` is missing the field.
            let value = attrs
                .get(field)
                .map_or_else(|| "-".to_string(), |v| v.clone());

            elt.body
                .children
                .push(Section::new(sec.head.clone(), idm::from_str(&value)?));
        }
        columns.children.push(elt);
    }

    io.write(&format!("{columns}"))?;

    Ok(())
}
