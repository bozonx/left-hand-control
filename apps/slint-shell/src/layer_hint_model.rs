//! Layer hint: the keys of the held layer, sent by the settings process to
//! the Spell worker, which shows them above all windows without taking the
//! keyboard focus or pointer input.

use crate::ui::{Assignment, LayerKeyCell};
use serde::{Deserialize, Serialize};
use slint::{ModelRc, VecModel};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerHint {
    pub name: String,
    /// Grid slots of each hand in row order; `None` is an empty slot.
    pub left: Vec<Option<HintKey>>,
    pub right: Vec<Option<HintKey>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintKey {
    pub label: String,
    pub kind: HintKind,
    pub action: String,
    pub emoji: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HintKind {
    Inherit,
    Suppress,
    Action,
}

impl LayerHint {
    pub fn from_cells(name: String, left: &[LayerKeyCell], right: &[LayerKeyCell]) -> Self {
        let keys = |cells: &[LayerKeyCell]| cells.iter().map(HintKey::from_cell).collect();
        Self {
            name,
            left: keys(left),
            right: keys(right),
        }
    }

    pub fn left_cells(&self) -> ModelRc<LayerKeyCell> {
        cells(&self.left)
    }

    pub fn right_cells(&self) -> ModelRc<LayerKeyCell> {
        cells(&self.right)
    }
}

impl HintKey {
    fn from_cell(cell: &LayerKeyCell) -> Option<Self> {
        (cell.index >= 0).then(|| Self {
            label: cell.label.to_string(),
            kind: match cell.kind {
                Assignment::Swallow => HintKind::Suppress,
                Assignment::Action => HintKind::Action,
                _ => HintKind::Inherit,
            },
            action: cell.action.to_string(),
            emoji: cell.emoji,
        })
    }
}

fn cells(keys: &[Option<HintKey>]) -> ModelRc<LayerKeyCell> {
    let cells: Vec<LayerKeyCell> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| match key {
            None => LayerKeyCell {
                index: -1,
                ..Default::default()
            },
            Some(key) => LayerKeyCell {
                index: index as i32,
                label: key.label.as_str().into(),
                kind: match key.kind {
                    HintKind::Inherit => Assignment::Transparent,
                    HintKind::Suppress => Assignment::Swallow,
                    HintKind::Action => Assignment::Action,
                },
                action: key.action.as_str().into(),
                emoji: key.emoji,
                ..Default::default()
            },
        })
        .collect();
    ModelRc::new(VecModel::from(cells))
}
