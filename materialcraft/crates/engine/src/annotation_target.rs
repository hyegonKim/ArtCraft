use crate::{FrameContents, FunctionGraphTarget, Result, SelectionTarget, Session, commands::bad};
use materialcraft_core::{CanvasAnnotations, Frame, NodeId, Value};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Canvas that owns the selected frame, comment or navigation pin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AnnotationTarget {
    #[default]
    Graph,
    Function {
        target: FunctionGraphTarget,
    },
    Pattern {
        owner: NodeId,
    },
}

impl AnnotationTarget {
    pub(crate) fn resolve(session: &Session, parameters: &Json) -> Result<Self> {
        Ok(match SelectionTarget::resolve(session, parameters)? {
            None | Some(SelectionTarget::Function(FunctionGraphTarget::Graph)) => Self::Graph,
            Some(SelectionTarget::Function(target)) => Self::Function { target },
            Some(SelectionTarget::Pattern(owner)) => Self::Pattern { owner },
        })
    }

    /// Command parameters that address this canvas.
    pub fn parameters(&self) -> Json {
        match self {
            Self::Graph => json!({}),
            Self::Function { target } => json!({"function": target}),
            Self::Pattern { owner } => json!({"pattern": owner.0}),
        }
    }

    /// Copy the annotations owned by this canvas.
    pub fn items(&self, session: &Session) -> Result<CanvasAnnotations> {
        Ok(match self {
            Self::Graph => {
                let graph = session.graph()?;
                CanvasAnnotations { frames: graph.frames.clone(), comments: graph.comments.clone(), pins: graph.pins.clone() }
            }
            Self::Function { target } => target.current(session)?.annotations.clone(),
            Self::Pattern { owner } => SelectionTarget::pattern(session, *owner)?.annotations.clone(),
        })
    }

    pub(crate) fn replace(&self, session: &mut Session, items: CanvasAnnotations) -> Result<()> {
        match self {
            Self::Graph => {
                let graph = session.graph_mut()?;
                graph.next_id = graph.next_id.max(items.next_id().unwrap_or(u64::MAX));
                graph.frames = items.frames;
                graph.comments = items.comments;
                graph.pins = items.pins;
            }
            Self::Function { target } => {
                let mut function = target.current(session)?.clone();
                function.annotations = items;
                target.replace(session, function)?;
            }
            Self::Pattern { owner } => {
                let mut graph = SelectionTarget::pattern(session, *owner)?.clone();
                graph.annotations = items;
                session.graph_mut()?.node_mut(*owner)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
            }
        }
        Ok(())
    }

    pub(crate) fn next_id(&self, session: &Session, items: &CanvasAnnotations) -> Result<u64> {
        let next = items.next_id().ok_or_else(|| bad("graph annotation", "no available annotation id"))?;
        Ok(if *self == Self::Graph { next.max(session.graph()?.next_id) } else { next })
    }

    /// Node positions in this canvas, using the shared node-id representation.
    pub fn positions(&self, session: &Session) -> Result<Vec<(NodeId, [f32; 2])>> {
        Ok(match self {
            Self::Graph => {
                let graph = session.graph()?;
                match &graph.function {
                    Some(function) => function.nodes.iter().map(|node| (NodeId(u64::from(node.id)), node.pos)).collect(),
                    None => graph.nodes.iter().map(|node| (node.id, node.pos)).collect(),
                }
            }
            Self::Function { target } => target.current(session)?.nodes.iter().map(|node| (NodeId(u64::from(node.id)), node.pos)).collect(),
            Self::Pattern { owner } => {
                SelectionTarget::pattern(session, *owner)?.nodes.iter().map(|node| (NodeId(u64::from(node.id)), node.position)).collect()
            }
        })
    }

    /// Capture the nodes and annotations moved with a frame.
    pub fn capture_frame(&self, session: &Session, frame: &Frame) -> Result<FrameContents> {
        if *self == Self::Graph {
            return Ok(FrameContents::capture(session.graph()?, frame));
        }

        let items = self.items(session)?;
        let nodes: Vec<_> = self.positions(session)?.into_iter().filter(|(_, position)| frame.contains_position(*position)).map(|(id, _)| id).collect();
        Ok(FrameContents {
            function_nodes: nodes.iter().filter_map(|id| u32::try_from(id.0).ok()).collect(),
            nodes,
            comments: items.comments.iter().filter(|comment| frame.contains_position(comment.pos)).map(|comment| comment.id).collect(),
            frames: items.frames.iter().filter(|candidate| candidate.id == frame.id || frame.contains_rect(candidate.rect)).map(|frame| frame.id).collect(),
            pins: items.pins.iter().filter(|pin| frame.contains_position(pin.pos)).map(|pin| pin.id).collect(),
        })
    }

    pub(crate) fn move_contents(&self, session: &mut Session, contents: &FrameContents, offset: [f32; 2]) -> Result<()> {
        if *self == Self::Graph {
            contents.translate(session.graph_mut()?, offset);
            return Ok(());
        }

        let translate = |position: [f32; 2]| crate::graph_grid::snap_position([position[0] + offset[0], position[1] + offset[1]], false);
        match self {
            Self::Function { target } => {
                let mut graph = target.current(session)?.clone();
                for node in graph.nodes.iter_mut().filter(|node| contents.function_nodes.contains(&node.id)) {
                    node.pos = translate(node.pos);
                }
                target.replace(session, graph)?;
            }
            Self::Pattern { owner } => {
                let mut graph = SelectionTarget::pattern(session, *owner)?.clone();
                for node in graph.nodes.iter_mut().filter(|node| contents.nodes.contains(&NodeId(u64::from(node.id)))) {
                    node.position = translate(node.position);
                }
                session.graph_mut()?.node_mut(*owner)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
            }
            Self::Graph => {}
        }
        let mut items = self.items(session)?;
        for comment in items.comments.iter_mut().filter(|comment| contents.comments.contains(&comment.id)) {
            comment.pos = translate(comment.pos);
        }
        for frame in items.frames.iter_mut().filter(|frame| contents.frames.contains(&frame.id)) {
            let position = translate([frame.rect[0], frame.rect[1]]);
            frame.rect[0] = position[0];
            frame.rect[1] = position[1];
        }
        for pin in items.pins.iter_mut().filter(|pin| contents.pins.contains(&pin.id)) {
            pin.pos = translate(pin.pos);
        }
        self.replace(session, items)
    }
}
