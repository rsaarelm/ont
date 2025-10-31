use std::cmp::Ordering;

use anyhow::Result;
use ont::{Outline, Section};

use crate::IoPipe;

pub fn run(
    io: IoPipe,
    sort_field: String,
    separate_favorites: bool,
) -> Result<()> {
    let mut outline: Outline = io.read_outline()?;
    outline
        .children
        .sort_by(|a, b| ord(separate_favorites, &sort_field, a, b));
    io.write(&outline)
}

fn ord(
    separate_favorites: bool,
    sort_field: &str,
    a: &Section,
    b: &Section,
) -> Ordering {
    let a_head = if separate_favorites {
        !a.head.ends_with(" *")
    } else {
        false
    };
    let b_head = if separate_favorites {
        !b.head.ends_with(" *")
    } else {
        false
    };

    let blank = String::new();
    let a_val = a.body.attrs.get(sort_field).unwrap_or(&blank);
    let b_val = b.body.attrs.get(sort_field).unwrap_or(&blank);

    (a_head, a_val).cmp(&(b_head, b_val))
}
