use std::{cell::Cell, ops::Range};

#[derive(Clone, Debug)]
pub enum Edit {
    InsertSingle { c: char, idx: u32 },
    DeleteSingle { c: char, idx: u32 },
    Insert { start: Cell<u32>, str_idx: u32 },
    Delete { start: Cell<u32>, str_idx: u32 },
}

impl Edit {
    #[must_use]
    pub fn invert(&self) -> Self {
        match self {
            Edit::InsertSingle { c, idx } => Edit::DeleteSingle { c: *c, idx: *idx },
            Edit::DeleteSingle { c, idx } => Edit::InsertSingle { c: *c, idx: *idx },
            Edit::Insert {
                start,
                str_idx: str,
            } => Edit::Delete {
                start: start.clone(),
                str_idx: *str,
            },
            Edit::Delete {
                start,
                str_idx: str,
            } => Edit::Insert {
                start: start.clone(),
                str_idx: *str,
            },
        }
    }

    pub fn range(&self, edit_strs: &[Vec<char>]) -> Range<u32> {
        match self {
            &Edit::InsertSingle { idx, .. } => idx..(idx + 1),
            &Edit::DeleteSingle { idx, .. } => idx..(idx + 1),
            Edit::Insert { start, str_idx } => {
                start.get()..edit_strs[*str_idx as usize].len() as u32
            }
            Edit::Delete { start, str_idx } => {
                start.get()..edit_strs[*str_idx as usize].len() as u32
            }
        }
    }
}
