use crate::{Comment, Frame, Pin};
use serde::{Deserialize, Serialize};

/// Editable helpers belonging to one embedded graph canvas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasAnnotations {
    #[serde(default)]
    pub frames: Vec<Frame>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub pins: Vec<Pin>,
}

impl CanvasAnnotations {
    /// Whether the canvas has no frames, comments or pins.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.comments.is_empty() && self.pins.is_empty()
    }

    /// The next annotation id, or none when all ids have been exhausted.
    pub fn next_id(&self) -> Option<u64> {
        self.frames
            .iter()
            .map(|frame| frame.id)
            .chain(self.comments.iter().map(|comment| comment.id))
            .chain(self.pins.iter().map(|pin| pin.id))
            .max()
            .unwrap_or(0)
            .checked_add(1)
    }
}
