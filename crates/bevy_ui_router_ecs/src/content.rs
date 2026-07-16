#![allow(dead_code)]

use core::{fmt, num::NonZeroU16};
use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ui_router_core::prelude::RouteSlotKey;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContentColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl ContentColor {
    pub const fn srgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn red(&self) -> u8 {
        self.red
    }

    pub const fn green(&self) -> u8 {
        self.green
    }

    pub const fn blue(&self) -> u8 {
        self.blue
    }

    pub const fn alpha(&self) -> u8 {
        self.alpha
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SolidRect {
    width_px: NonZeroU16,
    height_px: NonZeroU16,
    color: ContentColor,
}

impl SolidRect {
    pub fn new(
        width_px: u16,
        height_px: u16,
        color: ContentColor,
    ) -> Result<Self, ContentPrimitiveError> {
        let width_px = NonZeroU16::new(width_px)
            .ok_or_else(|| ContentPrimitiveError::new(ContentPrimitiveErrorKind::ZeroWidth))?;
        let height_px = NonZeroU16::new(height_px)
            .ok_or_else(|| ContentPrimitiveError::new(ContentPrimitiveErrorKind::ZeroHeight))?;
        Ok(Self {
            width_px,
            height_px,
            color,
        })
    }

    pub const fn width_px(&self) -> u16 {
        self.width_px.get()
    }

    pub const fn height_px(&self) -> u16 {
        self.height_px.get()
    }

    pub const fn color(&self) -> ContentColor {
        self.color
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContentPrimitiveKind {
    SolidRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentPrimitiveErrorKind {
    ZeroWidth,
    ZeroHeight,
}

#[derive(Debug)]
pub struct ContentPrimitiveError {
    kind: ContentPrimitiveErrorKind,
}

impl ContentPrimitiveError {
    const fn new(kind: ContentPrimitiveErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> ContentPrimitiveErrorKind {
        self.kind
    }
}

impl fmt::Display for ContentPrimitiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ContentPrimitiveErrorKind::ZeroWidth => formatter.write_str("width must be nonzero"),
            ContentPrimitiveErrorKind::ZeroHeight => formatter.write_str("height must be nonzero"),
        }
    }
}

impl std::error::Error for ContentPrimitiveError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ContentSession(u64);

fn allocate_content_session(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|_| current != 0)
        })
        .ok()
}

impl ContentSession {
    fn new() -> Self {
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

        let session = allocate_content_session(&NEXT_SESSION)
            .expect("content session identity space exhausted");
        Self(session)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContentEntity {
    session: ContentSession,
    index: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum RenderContent {
    Empty,
    SolidRect(SolidRect),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RealizedContentKind {
    Empty,
    SolidRect(SolidRect),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ContentRealizationEntry {
    local_index: usize,
    kind: RealizedContentKind,
}

impl ContentRealizationEntry {
    pub(crate) const fn local_index(self) -> usize {
        self.local_index
    }

    pub(crate) const fn kind(self) -> RealizedContentKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedOutlet {
    local_index: usize,
    key: RouteSlotKey,
}

impl AcceptedOutlet {
    pub(crate) const fn local_index(&self) -> usize {
        self.local_index
    }

    pub(crate) const fn key(&self) -> &RouteSlotKey {
        &self.key
    }
}

#[derive(Debug)]
pub(crate) struct AcceptedContentPlan {
    entries: Vec<ContentRealizationEntry>,
    relationships: Vec<(usize, usize)>,
    roots: Vec<usize>,
    outlets: Vec<AcceptedOutlet>,
}

impl AcceptedContentPlan {
    pub(crate) fn entries(&self) -> &[ContentRealizationEntry] {
        &self.entries
    }

    pub(crate) fn relationships(&self) -> &[(usize, usize)] {
        &self.relationships
    }

    pub(crate) fn roots(&self) -> &[usize] {
        &self.roots
    }

    pub(crate) fn outlets(&self) -> &[AcceptedOutlet] {
        &self.outlets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContentPlan {
    session: ContentSession,
    content: Vec<Option<RenderContent>>,
    content_order: Vec<ContentEntity>,
    topology_order: Vec<ContentEntity>,
    parents: Vec<Option<ContentEntity>>,
    children: Vec<Vec<ContentEntity>>,
    relationships: Vec<(ContentEntity, ContentEntity)>,
    outlets: Vec<(ContentEntity, RouteSlotKey)>,
}

impl ContentPlan {
    pub(crate) fn new() -> Self {
        Self {
            session: ContentSession::new(),
            content: Vec::new(),
            content_order: Vec::new(),
            topology_order: Vec::new(),
            parents: Vec::new(),
            children: Vec::new(),
            relationships: Vec::new(),
            outlets: Vec::new(),
        }
    }

    fn content_order(&self) -> &[ContentEntity] {
        &self.content_order
    }

    fn topology_order(&self) -> &[ContentEntity] {
        &self.topology_order
    }

    fn relationships(&self) -> &[(ContentEntity, ContentEntity)] {
        &self.relationships
    }

    fn outlets(&self) -> &[(ContentEntity, RouteSlotKey)] {
        &self.outlets
    }

    fn is_live(&self, entity: ContentEntity) -> bool {
        entity.session == self.session
            && self.content.get(entity.index).is_some_and(Option::is_some)
    }

    fn rebuild_topology_order(&mut self) {
        let roots: Vec<_> = self
            .content_order
            .iter()
            .copied()
            .filter(|entity| self.parents[entity.index].is_none())
            .collect();
        let mut order = Vec::with_capacity(self.content_order.len());
        let mut visited = vec![false; self.content.len()];
        for root in roots {
            let mut stack = vec![root];
            while let Some(entity) = stack.pop() {
                assert!(
                    !visited[entity.index],
                    "content topology contains a duplicate entity"
                );
                visited[entity.index] = true;
                order.push(entity);
                stack.extend(self.children[entity.index].iter().rev().copied());
            }
        }
        for (index, content) in self.content.iter().enumerate() {
            if content.is_some() {
                assert!(
                    visited[index],
                    "live content entity is missing from the topology"
                );
            }
        }
        self.topology_order = order;
    }

    pub(crate) fn accept(self) -> AcceptedContentPlan {
        let entries = self
            .topology_order
            .iter()
            .map(|entity| ContentRealizationEntry {
                local_index: entity.index,
                kind: match self.content[entity.index].expect("topology contains only live content")
                {
                    RenderContent::Empty => RealizedContentKind::Empty,
                    RenderContent::SolidRect(rect) => RealizedContentKind::SolidRect(rect),
                },
            })
            .collect();
        let relationships = self
            .relationships
            .iter()
            .map(|(parent, child)| (parent.index, child.index))
            .collect();
        let roots = self
            .topology_order
            .iter()
            .filter(|entity| self.parents[entity.index].is_none())
            .map(|entity| entity.index)
            .collect();
        let mut outlets = self
            .outlets
            .into_iter()
            .map(|(entity, key)| AcceptedOutlet {
                local_index: entity.index,
                key,
            })
            .collect::<Vec<_>>();
        outlets.sort_by(|left, right| {
            left.local_index
                .cmp(&right.local_index)
                .then_with(|| left.key.cmp(&right.key))
        });
        AcceptedContentPlan {
            entries,
            relationships,
            roots,
            outlets,
        }
    }
}

pub struct FrameContentWriter<'a> {
    plan: &'a mut ContentPlan,
}

impl<'a> FrameContentWriter<'a> {
    pub(crate) fn new(plan: &'a mut ContentPlan) -> Self {
        Self { plan }
    }

    pub fn spawn_empty(&mut self) -> Result<ContentEntity, ContentWriteError> {
        Ok(self.spawn(RenderContent::Empty))
    }

    pub fn spawn_solid_rect(
        &mut self,
        rect: SolidRect,
    ) -> Result<ContentEntity, ContentWriteError> {
        Ok(self.spawn(RenderContent::SolidRect(rect)))
    }

    pub fn add_child(
        &mut self,
        parent: ContentEntity,
        child: ContentEntity,
    ) -> Result<(), ContentWriteError> {
        self.validate_session(parent)?;
        self.validate_session(child)?;
        self.validate_live(parent)?;
        self.validate_live(child)?;

        if parent == child {
            return Err(ContentWriteError::new(ContentWriteErrorKind::SelfParent));
        }
        if self.plan.parents[child.index] == Some(parent) {
            return Err(ContentWriteError::new(ContentWriteErrorKind::DuplicateEdge));
        }
        if self.plan.parents[child.index].is_some() {
            return Err(ContentWriteError::new(
                ContentWriteErrorKind::DuplicateParent,
            ));
        }

        let mut ancestor = Some(parent);
        while let Some(entity) = ancestor {
            if entity == child {
                return Err(ContentWriteError::new(
                    ContentWriteErrorKind::RelationshipCycle,
                ));
            }
            ancestor = self.plan.parents[entity.index];
        }

        self.plan.parents[child.index] = Some(parent);
        self.plan.children[parent.index].push(child);
        self.plan.relationships.push((parent, child));
        self.plan.rebuild_topology_order();
        Ok(())
    }

    pub fn despawn(&mut self, entity: ContentEntity) -> Result<(), ContentWriteError> {
        self.validate_session(entity)?;
        self.validate_live(entity)?;

        let is_outlet = self
            .plan
            .outlets
            .iter()
            .any(|(outlet, _)| *outlet == entity);
        if self.plan.parents[entity.index].is_some()
            || !self.plan.children[entity.index].is_empty()
            || is_outlet
        {
            return Err(ContentWriteError::new(
                ContentWriteErrorKind::InvalidDespawn,
            ));
        }

        self.plan.content[entity.index] = None;
        self.plan
            .content_order
            .retain(|candidate| *candidate != entity);
        self.plan
            .topology_order
            .retain(|candidate| *candidate != entity);
        Ok(())
    }

    pub(crate) fn declare_outlet(
        &mut self,
        entity: ContentEntity,
        slot: RouteSlotKey,
    ) -> Result<(), ContentWriteError> {
        self.validate_session(entity)?;
        self.validate_live(entity)?;

        if self
            .plan
            .outlets
            .iter()
            .any(|(_, declared_slot)| declared_slot == &slot)
        {
            return Err(ContentWriteError::new(
                ContentWriteErrorKind::DuplicateOutlet,
            ));
        }

        self.plan.outlets.push((entity, slot));
        Ok(())
    }

    fn spawn(&mut self, content: RenderContent) -> ContentEntity {
        let entity = ContentEntity {
            session: self.plan.session,
            index: self.plan.content.len(),
        };
        self.plan.content.push(Some(content));
        self.plan.content_order.push(entity);
        self.plan.topology_order.push(entity);
        self.plan.parents.push(None);
        self.plan.children.push(Vec::new());
        entity
    }

    fn validate_session(&self, entity: ContentEntity) -> Result<(), ContentWriteError> {
        if entity.session != self.plan.session {
            return Err(ContentWriteError::new(
                ContentWriteErrorKind::ForeignSession,
            ));
        }
        Ok(())
    }

    fn validate_live(&self, entity: ContentEntity) -> Result<(), ContentWriteError> {
        if !self.plan.is_live(entity) {
            return Err(ContentWriteError::new(ContentWriteErrorKind::UnknownHandle));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentWriteErrorKind {
    UnknownHandle,
    ForeignSession,
    SelfParent,
    DuplicateEdge,
    DuplicateParent,
    RelationshipCycle,
    DuplicateOutlet,
    InvalidDespawn,
}

#[derive(Debug)]
pub struct ContentWriteError {
    kind: ContentWriteErrorKind,
}

impl ContentWriteError {
    const fn new(kind: ContentWriteErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> ContentWriteErrorKind {
        self.kind
    }
}

impl fmt::Display for ContentWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ContentWriteErrorKind::ForeignSession => "content entity belongs to another session",
            ContentWriteErrorKind::UnknownHandle => "content entity is not live",
            ContentWriteErrorKind::SelfParent => "content entity cannot parent itself",
            ContentWriteErrorKind::DuplicateEdge => "content relationship already exists",
            ContentWriteErrorKind::DuplicateParent => "content entity already has a parent",
            ContentWriteErrorKind::RelationshipCycle => "content relationship would form a cycle",
            ContentWriteErrorKind::DuplicateOutlet => "content outlet is already declared",
            ContentWriteErrorKind::InvalidDespawn => "content entity is still in use",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ContentWriteError {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy_ui_router_core::prelude::RouteSlotKey;

    #[test]
    fn exhausted_content_session_domain_never_wraps_or_reuses_authority() {
        let counter = AtomicU64::new(u64::MAX - 1);

        assert_eq!(allocate_content_session(&counter), Some(u64::MAX - 1));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(allocate_content_session(&counter), None);
        assert_eq!(allocate_content_session(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    fn color() -> ContentColor {
        ContentColor::srgba8(1, 2, 3, 4)
    }

    fn writer(plan: &mut ContentPlan) -> FrameContentWriter<'_> {
        FrameContentWriter::new(plan)
    }

    #[test]
    fn public_content_types_have_the_final_shape() {
        fn assert_value<T: Clone + Copy + std::fmt::Debug + Eq + std::hash::Hash + PartialEq>() {}
        fn assert_error_kind<T: Clone + Copy + std::fmt::Debug + Eq + PartialEq>() {}
        fn assert_error<
            T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
        >() {
        }

        let _: fn(&ContentColor) -> u8 = ContentColor::red;
        let _: fn(&ContentColor) -> u8 = ContentColor::green;
        let _: fn(&ContentColor) -> u8 = ContentColor::blue;
        let _: fn(&ContentColor) -> u8 = ContentColor::alpha;
        let _: fn(&SolidRect) -> u16 = SolidRect::width_px;
        let _: fn(&SolidRect) -> u16 = SolidRect::height_px;
        let _: fn(&SolidRect) -> ContentColor = SolidRect::color;
        let _: fn(&ContentPrimitiveError) -> ContentPrimitiveErrorKind =
            ContentPrimitiveError::kind;
        let _: fn(&ContentWriteError) -> ContentWriteErrorKind = ContentWriteError::kind;

        let primitive = ContentPrimitiveKind::SolidRect;
        assert_eq!(primitive, ContentPrimitiveKind::SolidRect);
        assert_value::<ContentColor>();
        assert_value::<SolidRect>();
        assert_value::<ContentPrimitiveKind>();
        assert_error_kind::<ContentPrimitiveErrorKind>();
        assert_error_kind::<ContentWriteErrorKind>();
        assert_error::<ContentPrimitiveError>();
        assert_error::<ContentWriteError>();
    }

    #[test]
    fn kind_getters_are_not_const() {
        let sources = [include_str!("content.rs"), include_str!("driver.rs")];
        let forbidden = ["pub const fn ", "kind"].concat();
        assert!(sources.iter().all(|source| !source.contains(&forbidden)));
    }

    #[test]
    fn content_write_error_discriminants_are_stable() {
        assert_eq!(ContentWriteErrorKind::UnknownHandle as usize, 0);
        assert_eq!(ContentWriteErrorKind::ForeignSession as usize, 1);
    }

    #[test]
    fn solid_rect_validates_nonzero_dimensions_and_preserves_srgba8() {
        assert_eq!(
            SolidRect::new(0, 0, color()).unwrap_err().kind(),
            ContentPrimitiveErrorKind::ZeroWidth
        );
        assert_eq!(
            SolidRect::new(0, 1, color()).unwrap_err().kind(),
            ContentPrimitiveErrorKind::ZeroWidth
        );
        assert_eq!(
            SolidRect::new(1, 0, color()).unwrap_err().kind(),
            ContentPrimitiveErrorKind::ZeroHeight
        );

        let rect = SolidRect::new(10, 20, color()).unwrap();
        assert_eq!((rect.width_px(), rect.height_px()), (10, 20));
        assert_eq!(
            (
                rect.color().red(),
                rect.color().green(),
                rect.color().blue(),
                rect.color().alpha()
            ),
            (1, 2, 3, 4)
        );
    }

    #[test]
    pub(crate) fn relationship_rejections_are_zero_write_with_fixed_priority() {
        let mut first = ContentPlan::new();
        let mut second = ContentPlan::new();
        let (a, b, c) = {
            let mut writer = writer(&mut first);
            (
                writer.spawn_empty().unwrap(),
                writer.spawn_empty().unwrap(),
                writer.spawn_empty().unwrap(),
            )
        };
        let foreign = writer(&mut second).spawn_empty().unwrap();

        let before = first.clone();
        assert_eq!(
            writer(&mut first).add_child(a, foreign).unwrap_err().kind(),
            ContentWriteErrorKind::ForeignSession
        );
        assert_eq!(first, before);
        assert_eq!(
            writer(&mut first).add_child(a, a).unwrap_err().kind(),
            ContentWriteErrorKind::SelfParent
        );
        assert_eq!(first, before);

        writer(&mut first).add_child(a, b).unwrap();
        let before = first.clone();
        assert_eq!(
            writer(&mut first).add_child(a, b).unwrap_err().kind(),
            ContentWriteErrorKind::DuplicateEdge
        );
        assert_eq!(first, before);
        assert_eq!(
            writer(&mut first).add_child(c, b).unwrap_err().kind(),
            ContentWriteErrorKind::DuplicateParent
        );
        assert_eq!(first, before);
        writer(&mut first).add_child(b, c).unwrap();
        let before = first.clone();
        assert_eq!(
            writer(&mut first).add_child(c, a).unwrap_err().kind(),
            ContentWriteErrorKind::RelationshipCycle
        );
        assert_eq!(first, before);
    }

    #[test]
    fn outlets_despawn_and_unknown_handles_are_zero_write() {
        let mut plan = ContentPlan::new();
        let mut foreign_plan = ContentPlan::new();
        let anchor = writer(&mut plan).spawn_empty().unwrap();
        let foreign = writer(&mut foreign_plan).spawn_empty().unwrap();
        let key = RouteSlotKey::named("detail").unwrap();

        let before = plan.clone();
        assert_eq!(
            writer(&mut plan)
                .declare_outlet(foreign, key.clone())
                .unwrap_err()
                .kind(),
            ContentWriteErrorKind::ForeignSession
        );
        assert_eq!(plan, before);
        writer(&mut plan)
            .declare_outlet(anchor, key.clone())
            .unwrap();
        let before = plan.clone();
        assert_eq!(
            writer(&mut plan)
                .declare_outlet(anchor, key)
                .unwrap_err()
                .kind(),
            ContentWriteErrorKind::DuplicateOutlet
        );
        assert_eq!(plan, before);
        assert_eq!(
            writer(&mut plan).despawn(anchor).unwrap_err().kind(),
            ContentWriteErrorKind::InvalidDespawn
        );
        assert_eq!(plan, before);

        let isolated = writer(&mut plan)
            .spawn_solid_rect(SolidRect::new(2, 3, color()).unwrap())
            .unwrap();
        writer(&mut plan).despawn(isolated).unwrap();
        let before = plan.clone();
        assert_eq!(
            writer(&mut plan).despawn(isolated).unwrap_err().kind(),
            ContentWriteErrorKind::UnknownHandle
        );
        assert_eq!(plan, before);
    }

    #[test]
    fn plan_preserves_stable_content_topology_relationship_and_outlet_order() {
        let mut plan = ContentPlan::new();
        let (child, root, leaf) = {
            let mut writer = writer(&mut plan);
            (
                writer.spawn_empty().unwrap(),
                writer.spawn_empty().unwrap(),
                writer
                    .spawn_solid_rect(SolidRect::new(4, 5, color()).unwrap())
                    .unwrap(),
            )
        };
        writer(&mut plan).add_child(root, child).unwrap();
        writer(&mut plan).add_child(child, leaf).unwrap();
        writer(&mut plan)
            .declare_outlet(root, RouteSlotKey::named("first").unwrap())
            .unwrap();
        writer(&mut plan)
            .declare_outlet(child, RouteSlotKey::named("second").unwrap())
            .unwrap();

        assert_eq!(plan.content_order(), &[child, root, leaf]);
        assert_eq!(plan.topology_order(), &[root, child, leaf]);
        assert_eq!(plan.relationships(), &[(root, child), (child, leaf)]);
        assert_eq!(plan.outlets()[0].0, root);
        assert_eq!(plan.outlets()[1].0, child);
    }

    #[test]
    fn accepted_plan_retains_outlets_in_stable_local_index_and_key_order() {
        let mut plan = ContentPlan::new();
        let (root, child) = {
            let mut writer = writer(&mut plan);
            (writer.spawn_empty().unwrap(), writer.spawn_empty().unwrap())
        };
        writer(&mut plan).add_child(root, child).unwrap();
        let first = RouteSlotKey::named("first").unwrap();
        let second = RouteSlotKey::named("second").unwrap();
        writer(&mut plan)
            .declare_outlet(child, second.clone())
            .unwrap();
        writer(&mut plan)
            .declare_outlet(root, first.clone())
            .unwrap();

        let accepted = plan.accept();

        assert_eq!(
            accepted
                .outlets()
                .iter()
                .map(|outlet| (outlet.local_index(), outlet.key().clone()))
                .collect::<Vec<_>>(),
            vec![(root.index, first), (child.index, second)],
        );
    }

    #[test]
    fn topology_rebuild_fails_closed_on_duplicate_child_corruption() {
        let mut plan = ContentPlan::new();
        let (root, child) = {
            let mut writer = writer(&mut plan);
            (writer.spawn_empty().unwrap(), writer.spawn_empty().unwrap())
        };
        plan.parents[child.index] = Some(root);
        plan.children[root.index] = vec![child, child];

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            plan.rebuild_topology_order();
        }));

        assert!(result.is_err());
    }

    #[test]
    fn topology_rebuild_preserves_a_stable_deep_chain() {
        const DEPTH: usize = 4_096;
        let mut plan = ContentPlan::new();
        let entities: Vec<_> = {
            let mut writer = writer(&mut plan);
            (0..DEPTH).map(|_| writer.spawn_empty().unwrap()).collect()
        };
        for pair in entities.windows(2) {
            let parent = pair[0];
            let child = pair[1];
            plan.parents[child.index] = Some(parent);
            plan.children[parent.index].push(child);
        }

        plan.rebuild_topology_order();

        assert_eq!(plan.topology_order(), entities);
    }
}
