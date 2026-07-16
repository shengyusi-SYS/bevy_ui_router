use crate::admission::{
    AdmissionError as CoreAdmissionError, HostAdmissionIssuer,
    HostAdmissionToken as CoreHostAdmissionToken, HostGrant as CoreHostGrant,
};
use crate::cleanup::{
    CleanupBatchToken, CleanupCompletion as CoreCleanupCompletion, CleanupCompletionDigest,
    CleanupManifestDigest, CleanupManifestSeal, CleanupSummary,
};
use crate::engine::{
    CommitError as CoreCommitError, CommitTicket as CoreCommitTicket,
    EngineAction as CoreEngineAction, EngineInstallError as CoreEngineInstallError,
    FailureCleanupError as CoreFailureCleanupError,
    FailureCleanupRequest as CoreFailureCleanupRequest, FrameMountRequest as CoreFrameMountRequest,
    FrameMountTarget as CoreFrameMountTarget, FrameTask as CoreFrameTask,
    MountMutationArtifact as CoreMountMutationArtifact, PreparedPublication, PrivateRequestLookup,
    PushError as CorePushError, ReportError as CoreReportError, RouterEngine as CoreRouterEngine,
};
use crate::frame::RouteFrameVersion;
use crate::history::{HistorySnapshot, MemoryHistory};
use crate::ids::{NavigationRequestId, RouteSlotId, RouterIdentity};
use crate::location::RouteLocation;
use crate::meta::ErrorBoundaryKey;
#[cfg(test)]
use crate::mount::MountRegistrationCompletion as CoreMountRegistrationCompletion;
use crate::mount::{
    MountGrant as CoreMountGrant, MountReceiptAttestation as CoreMountReceiptAttestation,
    MountReceiptDigest, MountReceiptProof as CoreMountReceiptProof, MountRegistrationDigest,
    MountRegistrationPermit as CoreMountRegistrationPermit,
};
use crate::presentation::{ActivePresentation, RouterAvailability};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestStatus,
};
use crate::route_tree::RouteTree;
use crate::slot::RouteSlotKey;
use crate::trace::{CleanupDisposition, MountRejectionKind, NavigationTraceEvent};
use crate::view_input::RouteViewInput;
use crate::view_key::RouteViewKey;
use crate::{
    HostSnapshot, HostTraceEvent, HostTraceEventKind,
    diagnostics::{DiagnosticEvent, DiagnosticSnapshot},
    fingerprint::RouteBranchFingerprint,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationError {
    Exhausted,
    AwaitingHost,
    OutstandingAdmission,
    AuthorityExists,
    ForeignRouter,
    Duplicate,
    Stale,
    Invalid,
    InvalidManifest,
}

pub struct HostAdmissionToken {
    inner: CoreHostAdmissionToken,
}

pub struct HostGrant {
    inner: CoreHostGrant,
}

pub struct MountGrant {
    inner: CoreMountGrant,
}

impl MountGrant {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn sealed(inner: CoreMountGrant) -> Self {
        Self { inner }
    }

    pub fn slot(&self) -> crate::ids::RouteSlotId {
        self.inner.slot()
    }

    #[cfg(test)]
    fn attest_receipt(self, digest: u64) -> MountReceiptAttestation {
        MountReceiptAttestation {
            inner: self
                .inner
                .attest_receipt(MountReceiptDigest::from_opaque(digest)),
        }
    }

    pub fn attest_receipt_with_proof(
        self,
        digest: u64,
    ) -> (MountReceiptAttestation, MountReceiptProof) {
        let (attestation, proof) = self
            .inner
            .attest_receipt_with_proof(MountReceiptDigest::from_opaque(digest));
        (
            MountReceiptAttestation { inner: attestation },
            MountReceiptProof { inner: proof },
        )
    }
}

pub struct MountReceiptAttestation {
    #[cfg_attr(not(test), allow(dead_code))]
    inner: CoreMountReceiptAttestation,
}

pub struct MountReceiptProof {
    inner: CoreMountReceiptProof,
}

pub struct MountRegistrationPermit {
    inner: CoreMountRegistrationPermit,
}

impl MountRegistrationPermit {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn sealed(inner: CoreMountRegistrationPermit) -> Self {
        Self { inner }
    }

    #[cfg(test)]
    fn slot(&self) -> crate::ids::RouteSlotId {
        self.inner.slot()
    }

    #[cfg(test)]
    fn complete(self, registration_digest: u64) -> MountRegistrationCompletion {
        MountRegistrationCompletion {
            inner: self
                .inner
                .complete(MountRegistrationDigest::from_opaque(registration_digest)),
        }
    }
}

#[cfg(test)]
pub struct MountRegistrationCompletion {
    #[cfg_attr(not(test), allow(dead_code))]
    inner: CoreMountRegistrationCompletion,
}

#[must_use = "a mount registration session must complete or reject"]
pub struct MountRegistrationSession<'engine> {
    engine: &'engine mut CoreRouterEngine,
    permit: CoreMountRegistrationPermit,
    slot: RouteSlotId,
}

impl MountRegistrationSession<'_> {
    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub fn complete(self, registration_digest: u64) {
        self.engine.complete_preflighted_mount_registration(
            self.permit,
            MountRegistrationDigest::from_opaque(registration_digest),
        )
    }

    pub fn reject(self) -> FailureCleanupRequest {
        FailureCleanupRequest {
            inner: self
                .engine
                .reject_preflighted_mount_registration(self.permit),
        }
    }
}

#[must_use = "a failed mount registration begin retains the rejection capability"]
pub struct MountRegistrationBeginError {
    kind: IntegrationError,
    permit: CoreMountRegistrationPermit,
}

impl MountRegistrationBeginError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn reject(
        self,
        engine: &mut RouterEngine,
    ) -> Result<FailureCleanupRequest, MountRegistrationBeginError> {
        if let Err(error) = engine
            .inner
            .validate_mount_registration_rejection(&self.permit)
        {
            return Err(Self {
                kind: map_mount_error(error),
                permit: self.permit,
            });
        }
        Ok(FailureCleanupRequest {
            inner: engine
                .inner
                .reject_preflighted_mount_registration(self.permit),
        })
    }
}

impl std::fmt::Debug for MountRegistrationBeginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MountRegistrationBeginError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl HostGrant {
    pub fn version(&self) -> &crate::HostVersion {
        self.inner.version()
    }

    pub fn top_slot(&self) -> crate::ids::RouteSlotId {
        self.inner.top_slot()
    }
}

pub fn host_version_matches_frame(version: &crate::HostVersion, frame: &RouteFrameVersion) -> bool {
    version.router_identity() == frame.router_identity() && version.host() == frame.host()
}

#[doc(hidden)]
pub fn frame_versions_share_router(left: &RouteFrameVersion, right: &RouteFrameVersion) -> bool {
    left.router_identity() == right.router_identity()
}

#[doc(hidden)]
pub fn mount_grant_matches_target(
    grant: &MountGrant,
    parent: &RouteFrameVersion,
    outlet: &crate::slot::RouteSlotKey,
) -> bool {
    grant.inner.matches_target(parent, outlet)
}

pub fn root_view(tree: &RouteTree) -> Option<&RouteViewKey> {
    tree.node(tree.root())?.options().view_binding()
}

pub fn for_each_route_view_binding(
    tree: &RouteTree,
    mut visit: impl FnMut(Option<&RouteViewKey>, Option<&ErrorBoundaryKey>, Option<&RouteViewKey>),
) {
    for node in tree.nodes_in_declaration_order() {
        let options = node.options();
        visit(
            options.view_binding(),
            options.error_boundary(),
            options.error_view_binding().map(|(_, view)| view),
        );
    }
}

pub struct FrameTask {
    inner: CoreFrameTask,
}

pub type RootFrameTask = FrameTask;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameMountTarget {
    TopSlot {
        host: crate::HostVersion,
        slot: RouteSlotId,
    },
    ChildOutlet {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    },
}

#[derive(Clone, Eq, PartialEq)]
pub struct ViewReportIdentity {
    inner: crate::engine::RootTaskIdentity,
}

impl FrameTask {
    pub fn report_identity(&self) -> ViewReportIdentity {
        ViewReportIdentity {
            inner: self.inner.identity().clone(),
        }
    }

    pub fn request(&self) -> NavigationRequestId {
        self.inner.identity().request()
    }

    pub fn view_request_sequence(&self) -> u64 {
        self.inner.identity().frame_task().raw()
    }

    pub fn frame(&self) -> &RouteFrameVersion {
        let current = &self.inner;
        // The engine creates the same logical version from this sealed identity
        // and the projection is the only cross-crate representation ECS needs
        current.frame_version()
    }

    pub fn view(&self) -> &RouteViewKey {
        self.inner.view()
    }

    pub fn input(&self) -> &RouteViewInput {
        self.inner.input()
    }

    pub fn take_input(&mut self) -> RouteViewInput {
        self.inner.take_input()
    }

    pub fn mount_target(&self) -> FrameMountTarget {
        match self.inner.mount_target() {
            CoreFrameMountTarget::TopSlot { host, slot } => FrameMountTarget::TopSlot {
                host: host.clone(),
                slot: *slot,
            },
            CoreFrameMountTarget::ChildOutlet { parent, outlet } => FrameMountTarget::ChildOutlet {
                parent: parent.clone(),
                outlet: outlet.clone(),
            },
        }
    }
}

pub struct MountRequest {
    inner: CoreFrameMountRequest,
}

impl MountRequest {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn parent(&self) -> &RouteFrameVersion {
        self.inner.parent()
    }

    pub fn outlet(&self) -> &RouteSlotKey {
        self.inner.outlet()
    }
}

#[doc(hidden)]
pub enum MountMutationArtifact {
    Grant(MountGrant),
    RegistrationPermit(MountRegistrationPermit),
}

#[doc(hidden)]
pub struct FailureCleanupRequest {
    inner: CoreFailureCleanupRequest,
}

impl FailureCleanupRequest {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn logical_roots(&self) -> &[RouteFrameVersion] {
        self.inner.logical_roots()
    }

    pub fn failure(&self) -> &NavigationFailure {
        self.inner.failure()
    }
}

pub enum EngineAction {
    FrameTask(FrameTask),
    MountRequest(MountRequest),
    #[doc(hidden)]
    FailureCleanup(FailureCleanupRequest),
    Commit(CommitTicket),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewFailureKind {
    Build,
    Apply,
}

pub struct CommitTicket {
    inner: CoreCommitTicket,
}

pub struct CleanupManifest {
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
}

impl CleanupManifest {
    pub fn empty() -> Self {
        Self {
            logical_roots: Vec::new(),
            manifest_digest: 0,
            obligation_count: 0,
        }
    }

    pub fn new(
        logical_roots: Vec<RouteFrameVersion>,
        manifest_digest: u64,
        obligation_count: usize,
    ) -> Self {
        Self {
            logical_roots,
            manifest_digest,
            obligation_count,
        }
    }
}

pub struct CleanupBatch {
    inner: CleanupBatchToken,
    manifest_digest: u64,
    obligation_count: usize,
}

impl Clone for CleanupBatch {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
        }
    }
}

impl CleanupBatch {
    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub fn complete_empty(self) -> CleanupCompletion {
        assert_eq!(self.obligation_count, 0, "cleanup batch is not empty");
        CleanupCompletion {
            inner: CoreCleanupCompletion::attest(
                self.inner,
                CleanupCompletionDigest::canonical_empty(),
                0,
            ),
        }
    }

    pub fn complete(self, completion_digest: u64, settled_count: usize) -> CleanupCompletion {
        CleanupCompletion {
            inner: CoreCleanupCompletion::attest(
                self.inner,
                CleanupCompletionDigest::from_opaque(completion_digest),
                settled_count,
            ),
        }
    }
}

pub struct CleanupCompletion {
    inner: CoreCleanupCompletion,
}

pub struct PreparedCommit {
    publication: PreparedPublication,
    cleanup: CleanupBatch,
}

impl PreparedCommit {
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }
}

pub struct NavigationRequestView<'a> {
    record: &'a NavigationRequestRecord,
}

pub enum RequestLookup<'a> {
    Found(NavigationRequestView<'a>),
    NotRetained,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingRequestClassification {
    NotRetained,
    Unknown,
}

#[derive(Clone, Eq, PartialEq)]
pub struct RequestRetentionClassifier {
    router: RouterIdentity,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
}

impl std::fmt::Debug for RequestRetentionClassifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RequestRetentionClassifier { .. }")
    }
}

impl RequestRetentionClassifier {
    pub(crate) const fn project(
        router: RouterIdentity,
        evicted_through: Option<u64>,
        highest_issued: Option<u64>,
    ) -> Self {
        Self {
            router,
            evicted_through,
            highest_issued,
        }
    }

    pub fn classify(&self, id: NavigationRequestId) -> MissingRequestClassification {
        if id.router_identity() != self.router
            || !self
                .highest_issued
                .is_some_and(|highest| id.sequence() <= highest)
        {
            return MissingRequestClassification::Unknown;
        }
        if self
            .evicted_through
            .is_some_and(|sequence| id.sequence() <= sequence)
        {
            MissingRequestClassification::NotRetained
        } else {
            MissingRequestClassification::Unknown
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterSnapshotProjection {
    requested: Option<RouteLocation>,
    matched: Option<RouteBranchFingerprint>,
    active: ActivePresentation,
    availability: RouterAvailability,
    pending_phase: Option<NavigationPhase>,
    history: HistorySnapshot,
    running_request: Option<NavigationRequestId>,
    earliest_retained_sequence: Option<u64>,
    request_retention: RequestRetentionClassifier,
    diagnostics: DiagnosticSnapshot,
    host: HostSnapshot,
}

impl RouterSnapshotProjection {
    pub fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.matched.as_ref()
    }

    pub fn active(&self) -> &ActivePresentation {
        &self.active
    }

    pub fn availability(&self) -> &RouterAvailability {
        &self.availability
    }

    pub fn pending_phase(&self) -> Option<&NavigationPhase> {
        self.pending_phase.as_ref()
    }

    pub fn history(&self) -> &HistorySnapshot {
        &self.history
    }

    pub fn running_request(&self) -> Option<NavigationRequestId> {
        self.running_request
    }

    pub fn earliest_retained_sequence(&self) -> Option<u64> {
        self.earliest_retained_sequence
    }

    pub fn request_retention(&self) -> &RequestRetentionClassifier {
        &self.request_retention
    }

    pub fn diagnostics(&self) -> &DiagnosticSnapshot {
        &self.diagnostics
    }

    pub fn host(&self) -> &HostSnapshot {
        &self.host
    }
}

pub fn project_cleanup_summary(
    foreground_pending: bool,
    quarantine_count: usize,
    last_disposition: Option<CleanupDisposition>,
) -> CleanupSummary {
    CleanupSummary::project(foreground_pending, quarantine_count, last_disposition)
}

impl NavigationRequestView<'_> {
    pub fn id(&self) -> NavigationRequestId {
        self.record.id()
    }

    pub fn requested(&self) -> &RouteLocation {
        self.record.requested()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.record.matched()
    }

    pub fn status(&self) -> &RequestStatus {
        self.record.status()
    }

    pub fn trace(&self) -> &[NavigationTraceEvent] {
        self.record.trace()
    }

    pub fn diagnostics(&self) -> &[DiagnosticEvent] {
        self.record.diagnostics()
    }
}

pub struct RouterEngine {
    inner: CoreRouterEngine,
    admission: HostAdmissionIssuer,
    host_trace: Vec<HostTraceEvent>,
}

impl RouterEngine {
    pub fn install(tree: RouteTree, fallback: RouteViewKey) -> Result<Self, IntegrationError> {
        let inner = CoreRouterEngine::install(tree, fallback).map_err(map_install_error)?;
        let admission = HostAdmissionIssuer::new().map_err(|_| IntegrationError::Exhausted)?;
        Ok(Self {
            inner,
            admission,
            host_trace: Vec::new(),
        })
    }

    pub fn request_host_admission(&mut self) -> Result<HostAdmissionToken, IntegrationError> {
        let inner = self
            .inner
            .request_host_admission(&mut self.admission)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRequested,
            None,
            None,
        ));
        Ok(HostAdmissionToken { inner })
    }

    pub fn reject_host_admission(
        &mut self,
        token: HostAdmissionToken,
    ) -> Result<(), IntegrationError> {
        self.admission
            .reject(&token.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRejected,
            None,
            None,
        ));
        Ok(())
    }

    pub fn grant_host_admission(
        &mut self,
        token: &HostAdmissionToken,
    ) -> Result<HostGrant, IntegrationError> {
        let inner = self
            .inner
            .grant_host_admission(&mut self.admission, &token.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::HostGranted,
            Some(inner.version().clone()),
            Some(inner.top_slot()),
        ));
        Ok(HostGrant { inner })
    }

    pub fn reject_host_after_mutation(&mut self, grant: HostGrant) -> Result<(), IntegrationError> {
        let version = grant.inner.version().clone();
        let slot = grant.inner.top_slot();
        self.admission
            .reject_after_mutation(&grant.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRejectedAfterMutation,
            Some(version),
            Some(slot),
        ));
        Ok(())
    }

    pub fn record_top_slot_registered(
        &mut self,
        grant: &HostGrant,
    ) -> Result<(), IntegrationError> {
        if !self.admission.is_outstanding_grant(&grant.inner) {
            return Err(IntegrationError::Stale);
        }
        let version = grant.inner.version();
        let slot = grant.inner.top_slot();
        if self.host_trace.iter().any(|event| {
            event.kind() == HostTraceEventKind::TopSlotRegistered
                && event.version() == Some(version)
                && event.slot() == Some(slot)
        }) {
            return Err(IntegrationError::Duplicate);
        }
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::TopSlotRegistered,
            Some(version.clone()),
            Some(slot),
        ));
        Ok(())
    }

    pub fn confirm_host(&mut self, grant: HostGrant) {
        let confirmation = self
            .admission
            .confirm(&grant.inner)
            .expect("a live sealed host grant has one valid confirmation");
        self.inner
            .confirm_host(&self.admission, &confirmation)
            .expect("the engine cannot change while its sealed host grant is outstanding");
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::HostReady,
            self.inner.host().cloned(),
            self.inner.top_slot(),
        ));
    }

    pub fn push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, IntegrationError> {
        self.inner.enqueue_push(location).map_err(map_push_error)
    }

    pub fn collect(&mut self) {
        self.inner.collect();
    }

    pub fn action(&mut self) -> Option<EngineAction> {
        self.inner.action().map(|action| match action {
            CoreEngineAction::FrameTask(inner) => EngineAction::FrameTask(FrameTask { inner }),
            CoreEngineAction::MountRequest(inner) => {
                EngineAction::MountRequest(MountRequest { inner })
            }
            CoreEngineAction::FailureCleanup(inner) => {
                EngineAction::FailureCleanup(FailureCleanupRequest { inner })
            }
            CoreEngineAction::Commit(inner) => EngineAction::Commit(CommitTicket { inner }),
        })
    }

    pub fn report_applied_with_outlets(
        &mut self,
        task: FrameTask,
        outlets: Vec<RouteSlotKey>,
    ) -> Result<(), IntegrationError> {
        self.inner
            .accept_applied_report(task.inner.into_applied_report(outlets))
            .map_err(map_report_error)
    }

    pub fn report_applied(
        &mut self,
        task: RootFrameTask,
    ) -> Result<CommitTicket, IntegrationError> {
        self.inner
            .accept_applied_report(task.inner.into_legacy_applied_report())
            .map_err(map_report_error)?;
        match self.inner.action() {
            Some(CoreEngineAction::Commit(inner)) => Ok(CommitTicket { inner }),
            Some(other) => {
                self.inner.restore_action(other);
                Err(IntegrationError::Invalid)
            }
            None => Err(IntegrationError::Invalid),
        }
    }

    pub fn resolve_mount(&mut self, request: MountRequest) -> Result<MountGrant, IntegrationError> {
        self.inner
            .resolve_mount(request.inner)
            .map(MountGrant::sealed)
            .map_err(map_mount_error)
    }

    #[doc(hidden)]
    pub fn reject_mount(
        &mut self,
        request: MountRequest,
        rejection: MountRejectionKind,
    ) -> Result<FailureCleanupRequest, IntegrationError> {
        self.inner
            .reject_mount(request.inner, rejection)
            .map(|inner| FailureCleanupRequest { inner })
            .map_err(map_mount_error)
    }

    #[doc(hidden)]
    pub fn reject_mount_after_mutation(
        &mut self,
        artifact: MountMutationArtifact,
    ) -> Result<FailureCleanupRequest, IntegrationError> {
        let artifact = match artifact {
            MountMutationArtifact::Grant(grant) => CoreMountMutationArtifact::Grant(grant.inner),
            MountMutationArtifact::RegistrationPermit(permit) => {
                CoreMountMutationArtifact::RegistrationPermit(permit.inner)
            }
        };
        self.inner
            .reject_mount_after_mutation(artifact)
            .map(|inner| FailureCleanupRequest { inner })
            .map_err(map_mount_error)
    }

    pub fn accept_mount_receipt(
        &mut self,
        receipt: MountReceiptAttestation,
    ) -> Result<MountRegistrationPermit, IntegrationError> {
        self.inner
            .accept_mount_receipt(receipt.inner)
            .map(MountRegistrationPermit::sealed)
            .map_err(map_mount_error)
    }

    pub fn begin_mount_registration<'engine>(
        &'engine mut self,
        permit: MountRegistrationPermit,
        proof: &MountReceiptProof,
    ) -> Result<MountRegistrationSession<'engine>, MountRegistrationBeginError> {
        let slot = match self
            .inner
            .validate_mount_registration(&permit.inner, &proof.inner)
        {
            Ok(slot) => slot,
            Err(error) => {
                return Err(MountRegistrationBeginError {
                    kind: map_mount_error(error),
                    permit: permit.inner,
                });
            }
        };
        Ok(MountRegistrationSession {
            engine: &mut self.inner,
            permit: permit.inner,
            slot,
        })
    }

    #[cfg(test)]
    fn complete_mount_registration(
        &mut self,
        completion: MountRegistrationCompletion,
    ) -> Result<(), IntegrationError> {
        self.inner
            .complete_mount_registration(completion.inner)
            .map_err(map_mount_error)
    }

    pub fn validate_report_identity(
        &mut self,
        task: &RootFrameTask,
    ) -> Result<(), IntegrationError> {
        self.inner
            .validate_view_report_identity(task.inner.identity())
            .map_err(map_report_error)
    }

    pub fn report_failed(
        &mut self,
        task: RootFrameTask,
        kind: ViewFailureKind,
        message: &'static str,
    ) -> Result<Option<FailureCleanupRequest>, IntegrationError> {
        let kind = match kind {
            ViewFailureKind::Build => NavigationFailureKind::Build,
            ViewFailureKind::Apply => NavigationFailureKind::Apply,
        };
        self.inner
            .begin_view_report(task.inner.identity())
            .map(|session| {
                session
                    .failed(NavigationFailure::new(kind, message))
                    .map(|inner| FailureCleanupRequest { inner })
            })
            .map_err(map_report_error)
    }

    #[doc(hidden)]
    pub fn prepare_failure_cleanup(
        &mut self,
        request: FailureCleanupRequest,
        manifest: CleanupManifest,
    ) -> Result<CleanupBatch, IntegrationError> {
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let inner = self
            .inner
            .prepare_failure_cleanup(
                request.inner,
                CleanupManifestSeal::new(
                    manifest.logical_roots,
                    CleanupManifestDigest::from_opaque(manifest_digest),
                    obligation_count,
                ),
            )
            .map_err(map_failure_cleanup_error)?;
        Ok(CleanupBatch {
            inner,
            manifest_digest,
            obligation_count,
        })
    }

    pub fn prepare_commit(
        &mut self,
        ticket: &CommitTicket,
        manifest: CleanupManifest,
    ) -> Result<PreparedCommit, IntegrationError> {
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let sealed = self
            .inner
            .seal_commit(
                &ticket.inner,
                CleanupManifestSeal::new(
                    manifest.logical_roots,
                    CleanupManifestDigest::from_opaque(manifest_digest),
                    obligation_count,
                ),
            )
            .map_err(map_commit_error)?;
        let cleanup = CleanupBatch {
            inner: sealed.cleanup().clone(),
            manifest_digest,
            obligation_count,
        };
        let publication = self
            .inner
            .prepare_publication(sealed)
            .map_err(map_commit_error)?;
        Ok(PreparedCommit {
            publication,
            cleanup,
        })
    }

    pub fn publish<F>(&mut self, prepared: PreparedCommit, mutation: F)
    where
        F: FnOnce(),
    {
        self.inner.publish_prepared(prepared.publication, mutation)
    }

    pub fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), IntegrationError> {
        self.inner
            .complete_cleanup(completion.inner)
            .map_err(|error| match error {
                crate::engine::CleanupError::Duplicate => IntegrationError::Duplicate,
                crate::engine::CleanupError::Stale => IntegrationError::Stale,
                crate::engine::CleanupError::Invalid => IntegrationError::Invalid,
            })
    }

    pub fn observe_cleanup_receipt(
        &mut self,
        batch: &CleanupBatch,
        disposition: CleanupDisposition,
    ) -> Result<(), IntegrationError> {
        self.inner
            .observe_cleanup_receipt(&batch.inner, disposition)
            .map_err(|_| IntegrationError::Stale)
    }

    pub fn snapshot(&self) -> RouterSnapshotProjection {
        let (router, evicted_through, highest_issued) = self.inner.request_retention_facts();
        RouterSnapshotProjection {
            requested: self.inner.requested().cloned(),
            matched: self.inner.matched().cloned(),
            active: self.inner.active().clone(),
            availability: self.inner.availability().clone(),
            pending_phase: self.inner.pending_phase().cloned(),
            history: self.inner.history_snapshot(),
            running_request: self.inner.running_request(),
            earliest_retained_sequence: self.inner.earliest_retained_sequence(),
            request_retention: RequestRetentionClassifier::project(
                router,
                evicted_through,
                highest_issued,
            ),
            diagnostics: self.inner.diagnostic_snapshot(),
            host: HostSnapshot::project(
                self.inner.host().cloned(),
                self.inner.top_slot(),
                self.host_trace.clone(),
            ),
        }
    }

    pub fn active(&self) -> &ActivePresentation {
        self.inner.active()
    }

    pub fn history(&self) -> &MemoryHistory {
        self.inner.history()
    }

    pub fn router_diagnostics(&self) -> &[DiagnosticEvent] {
        self.inner.router_diagnostics()
    }

    pub fn request(&self, id: NavigationRequestId) -> Option<NavigationRequestView<'_>> {
        self.inner
            .request(id)
            .map(|record| NavigationRequestView { record })
    }

    pub fn request_lookup(&self, id: NavigationRequestId) -> RequestLookup<'_> {
        match self.inner.private_lookup(id) {
            PrivateRequestLookup::Retained(record) => {
                RequestLookup::Found(NavigationRequestView { record })
            }
            PrivateRequestLookup::Evicted => RequestLookup::NotRetained,
            PrivateRequestLookup::Unknown => RequestLookup::Unknown,
        }
    }
}

fn map_install_error(error: CoreEngineInstallError) -> IntegrationError {
    match error {
        CoreEngineInstallError::IdExhausted => IntegrationError::Exhausted,
    }
}

fn map_admission_error(error: CoreAdmissionError) -> IntegrationError {
    match error {
        CoreAdmissionError::OutstandingExists => IntegrationError::OutstandingAdmission,
        CoreAdmissionError::AuthorityExists => IntegrationError::AuthorityExists,
        CoreAdmissionError::ForeignRouter => IntegrationError::ForeignRouter,
        CoreAdmissionError::Stale => IntegrationError::Stale,
        CoreAdmissionError::Exhausted => IntegrationError::Exhausted,
    }
}

fn map_push_error(error: CorePushError) -> IntegrationError {
    match error {
        CorePushError::AwaitingHost => IntegrationError::AwaitingHost,
        CorePushError::IdExhausted => IntegrationError::Exhausted,
    }
}

fn map_report_error(error: CoreReportError) -> IntegrationError {
    match error {
        CoreReportError::Duplicate => IntegrationError::Duplicate,
        CoreReportError::Stale => IntegrationError::Stale,
        CoreReportError::Invalid => IntegrationError::Invalid,
    }
}

fn map_mount_error(error: crate::mount::MountError) -> IntegrationError {
    match error {
        crate::mount::MountError::Exhausted => IntegrationError::Exhausted,
        crate::mount::MountError::Foreign => IntegrationError::ForeignRouter,
        crate::mount::MountError::Stale => IntegrationError::Stale,
        crate::mount::MountError::WrongPhase => IntegrationError::Invalid,
    }
}

fn map_commit_error(error: CoreCommitError) -> IntegrationError {
    match error {
        CoreCommitError::Stale => IntegrationError::Stale,
        CoreCommitError::InvalidManifest => IntegrationError::InvalidManifest,
    }
}

fn map_failure_cleanup_error(error: CoreFailureCleanupError) -> IntegrationError {
    match error {
        CoreFailureCleanupError::Foreign => IntegrationError::ForeignRouter,
        CoreFailureCleanupError::Stale => IntegrationError::Stale,
        CoreFailureCleanupError::InvalidManifest => IntegrationError::InvalidManifest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::HostAdmissionIssuer;
    use crate::frame::RouteFrameVersion;
    use crate::ids::{CoreIdAllocator, RouteGeneration};
    use crate::mount::MountIssuer;
    use crate::prelude::{
        ActivePresentationKind, CleanupDisposition, FrameOrigin, HostTraceEventKind,
        NavigationPhase, NavigationTraceEventKind, RequestStatus, RouteLocation, RouteNodeOptions,
        RoutePattern, RouteTree, RouteViewKey, RouterAvailability,
    };

    fn tree(view: &RouteViewKey) -> RouteTree {
        RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap()
    }

    fn nested_tree() -> RouteTree {
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn confirmed_nested_engine() -> RouterEngine {
        let mut engine =
            RouterEngine::install(nested_tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn pending_nested_receipt(engine: &mut RouterEngine, digest: u64) -> MountReceiptAttestation {
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        engine.resolve_mount(mount).unwrap().attest_receipt(digest)
    }

    fn pending_mount_registration(
        engine: &mut RouterEngine,
        location: &str,
        digest: u64,
    ) -> (
        NavigationRequestId,
        RouteFrameVersion,
        RouteSlotId,
        MountRegistrationPermit,
        MountReceiptProof,
    ) {
        let request = engine.push(RouteLocation::new(location)).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_frame = parent.frame().clone();
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        let grant = engine.resolve_mount(mount).unwrap();
        let slot = grant.slot();
        let (receipt, proof) = grant.attest_receipt_with_proof(digest);
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        (request, parent_frame, slot, permit, proof)
    }

    #[test]
    fn mount_registration_cross_router_proof_is_foreign_and_zero_write() {
        let mut receiver = confirmed_nested_engine();
        let (request, parent, receiver_slot, receiver_permit, _) =
            pending_mount_registration(&mut receiver, "/items/1", 311);
        let mut source = confirmed_nested_engine();
        let (_, _, source_slot, _, source_proof) =
            pending_mount_registration(&mut source, "/items/1", 311);
        assert_eq!(receiver_slot, source_slot);
        let before = receiver.inner.testing_snapshot();

        let error = match receiver.begin_mount_registration(receiver_permit, &source_proof) {
            Ok(_) => panic!("cross-router proof must not open a registration session"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        assert_eq!(receiver.inner.testing_snapshot(), before);
        let failure = error.reject(&mut receiver).unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    #[test]
    fn mount_registration_old_same_router_proof_is_stale_and_zero_write() {
        let mut engine = confirmed_nested_engine();
        let (_, _, _, permit, old_proof) = pending_mount_registration(&mut engine, "/items/1", 313);
        engine
            .begin_mount_registration(permit, &old_proof)
            .unwrap()
            .complete(317);
        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("completed registration must issue the child task")
        };
        engine
            .report_applied_with_outlets(child, Vec::new())
            .unwrap();
        let EngineAction::Commit(ticket) = engine.action().unwrap() else {
            panic!("completed child must yield one commit")
        };
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        engine.publish(prepared, || {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();

        let (request, parent, _, permit, _) =
            pending_mount_registration(&mut engine, "/items/2", 313);
        let before = engine.inner.testing_snapshot();
        let error = match engine.begin_mount_registration(permit, &old_proof) {
            Ok(_) => panic!("an old same-router proof must not open a registration session"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::Stale);
        assert_eq!(engine.inner.testing_snapshot(), before);
        let failure = error.reject(&mut engine).unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    #[test]
    fn mount_registration_session_withholds_child_until_completion() {
        let mut engine = confirmed_nested_engine();
        let (_, _, slot, permit, proof) = pending_mount_registration(&mut engine, "/items/1", 319);
        assert!(engine.action().is_none());
        let session = engine.begin_mount_registration(permit, &proof).unwrap();

        assert_eq!(session.slot(), slot);
        session.complete(321);

        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("only completed registration may issue the child task")
        };
        assert!(matches!(
            child.mount_target(),
            FrameMountTarget::ChildOutlet { .. }
        ));
    }

    #[test]
    fn mount_registration_context_drift_returns_linear_rejection_capability() {
        let mut engine = confirmed_nested_engine();
        let (request, parent, _, permit, proof) =
            pending_mount_registration(&mut engine, "/items/1", 327);
        engine
            .inner
            .testing_set_mount_registration_context_live(false);
        let before = engine.inner.testing_snapshot();

        let error = match engine.begin_mount_registration(permit, &proof) {
            Ok(_) => panic!("drifted engine context must not open a registration session"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::Invalid);
        assert_eq!(engine.inner.testing_snapshot(), before);
        engine
            .inner
            .testing_set_mount_registration_context_live(true);
        let failure = error.reject(&mut engine).unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    #[test]
    fn mount_registration_session_rejects_with_exact_failure_cleanup() {
        let mut engine = confirmed_nested_engine();
        let (request, parent, slot, permit, proof) =
            pending_mount_registration(&mut engine, "/items/1", 323);
        let session = engine.begin_mount_registration(permit, &proof).unwrap();
        assert_eq!(session.slot(), slot);

        let failure = session.reject();

        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    fn pending_nested_completion(
        engine: &mut RouterEngine,
        receipt_digest: u64,
        registration_digest: u64,
    ) -> MountRegistrationCompletion {
        let receipt = pending_nested_receipt(engine, receipt_digest);
        engine
            .accept_mount_receipt(receipt)
            .unwrap()
            .complete(registration_digest)
    }

    #[test]
    fn nested_integration_requires_outlet_report_and_registration_completion() {
        let mut engine = confirmed_nested_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_sequence = parent.view_request_sequence();
        let parent_frame = parent.frame().clone();
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();

        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("parent report must yield a mount request")
        };
        assert_eq!(mount.request(), request);
        assert_eq!(mount.parent(), &parent_frame);
        assert!(mount.outlet().is_default());
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(101);
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        assert!(engine.action().is_none());
        let completion = permit.complete(103);
        assert!(engine.action().is_none());
        engine.complete_mount_registration(completion).unwrap();

        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("registration completion must yield the child frame task")
        };
        assert_ne!(child.view_request_sequence(), parent_sequence);
        assert!(matches!(
            child.mount_target(),
            FrameMountTarget::ChildOutlet { .. }
        ));
        engine
            .report_applied_with_outlets(child, Vec::new())
            .unwrap();
        assert!(matches!(engine.action(), Some(EngineAction::Commit(_))));
    }

    #[test]
    fn foreign_mount_receipt_and_completion_are_stable_in_idle_and_terminal_receivers() {
        let mut idle_receipt_receiver = confirmed_nested_engine();
        let mut receipt_source = confirmed_nested_engine();
        let foreign_receipt = pending_nested_receipt(&mut receipt_source, 251);
        assert!(matches!(
            idle_receipt_receiver.accept_mount_receipt(foreign_receipt),
            Err(IntegrationError::ForeignRouter)
        ));

        let mut terminal_receipt_receiver = confirmed_nested_engine();
        settle_push(&mut terminal_receipt_receiver, RouteLocation::new("/"));
        let mut terminal_receipt_source = confirmed_nested_engine();
        let foreign_receipt = pending_nested_receipt(&mut terminal_receipt_source, 257);
        assert!(matches!(
            terminal_receipt_receiver.accept_mount_receipt(foreign_receipt),
            Err(IntegrationError::ForeignRouter)
        ));

        let mut idle_completion_receiver = confirmed_nested_engine();
        let mut completion_source = confirmed_nested_engine();
        let foreign_completion = pending_nested_completion(&mut completion_source, 263, 269);
        assert_eq!(
            idle_completion_receiver.complete_mount_registration(foreign_completion),
            Err(IntegrationError::ForeignRouter)
        );

        let mut terminal_completion_receiver = confirmed_nested_engine();
        settle_push(&mut terminal_completion_receiver, RouteLocation::new("/"));
        let mut terminal_completion_source = confirmed_nested_engine();
        let foreign_completion =
            pending_nested_completion(&mut terminal_completion_source, 271, 277);
        assert_eq!(
            terminal_completion_receiver.complete_mount_registration(foreign_completion),
            Err(IntegrationError::ForeignRouter)
        );
    }

    #[test]
    fn consumed_mount_receipt_and_completion_replays_are_stale_without_fallback() {
        let mut engine = confirmed_nested_engine();
        let receipt = pending_nested_receipt(&mut engine, 281);
        let receipt_replay = MountReceiptAttestation {
            inner: receipt.inner.clone(),
        };
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let after_receipt = engine.snapshot();
        assert!(matches!(
            engine.accept_mount_receipt(receipt_replay),
            Err(IntegrationError::Stale)
        ));
        assert_eq!(engine.snapshot(), after_receipt);

        let completion = permit.complete(283);
        let completion_replay = MountRegistrationCompletion {
            inner: completion.inner.clone(),
        };
        engine.complete_mount_registration(completion).unwrap();
        let after_completion = engine.snapshot();
        assert_eq!(
            engine.complete_mount_registration(completion_replay),
            Err(IntegrationError::Stale)
        );
        assert_eq!(engine.snapshot(), after_completion);
        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("normal completion must keep the child frame path")
        };
        assert_ne!(child.frame().origin(), &FrameOrigin::RouterFallback);
    }

    #[test]
    fn legacy_report_applied_reissues_nested_parent_without_lifecycle_change() {
        let mut engine = confirmed_nested_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let exact = parent.report_identity();
        let before = engine.snapshot();

        assert!(matches!(
            engine.report_applied(parent),
            Err(IntegrationError::Invalid)
        ));
        assert_eq!(engine.snapshot(), before);
        let EngineAction::FrameTask(reissued) = engine.action().unwrap() else {
            panic!("legacy misuse must restore exactly one parent task")
        };
        assert!(reissued.report_identity() == exact);
    }

    #[test]
    fn outlet_aware_missing_required_key_yields_mount_request_and_failure_cleanup() {
        let mut engine = confirmed_nested_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_frame = parent.frame().clone();

        engine
            .report_applied_with_outlets(parent, Vec::new())
            .unwrap();

        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("outlet-aware report still yields the matched mount request")
        };
        let failure = engine
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent_frame.clone()]);
        assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );

        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::new(vec![parent_frame], 181, 1))
            .unwrap();
        assert_eq!(cleanup.manifest_digest(), 181);
        assert_eq!(cleanup.obligation_count(), 1);
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| { event.kind() != NavigationTraceEventKind::RequestTerminal })
        );
        engine.complete_cleanup(cleanup.complete(191, 1)).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Mount
        ));
    }

    #[test]
    fn registration_permit_is_the_only_post_receipt_rejection_capability() {
        let mut engine = confirmed_nested_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_frame = parent.frame().clone();
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(193);
        let permit = engine.accept_mount_receipt(receipt).unwrap();

        let failure = engine
            .reject_mount_after_mutation(MountMutationArtifact::RegistrationPermit(permit))
            .unwrap();

        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent_frame.clone()]);
        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::new(vec![parent_frame], 197, 2))
            .unwrap();
        engine.complete_cleanup(cleanup.complete(199, 2)).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Mount
        ));
    }

    #[test]
    fn foreign_failure_cleanup_request_is_rejected_before_receiver_mutation() {
        let mut source = confirmed_nested_engine();
        source.push(RouteLocation::new("/items/1")).unwrap();
        source.collect();
        let EngineAction::FrameTask(parent) = source.action().unwrap() else {
            panic!("parent task expected")
        };
        source
            .report_applied_with_outlets(parent, Vec::new())
            .unwrap();
        let EngineAction::MountRequest(mount) = source.action().unwrap() else {
            panic!("mount request expected")
        };
        let failure = source
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let roots = failure.logical_roots().to_vec();

        let mut receiver = confirmed_nested_engine();
        let before = receiver.snapshot();
        assert!(matches!(
            receiver.prepare_failure_cleanup(failure, CleanupManifest::new(roots, 229, 1)),
            Err(IntegrationError::ForeignRouter)
        ));
        assert_eq!(receiver.snapshot(), before);
    }

    #[test]
    fn mount_wrappers_consume_only_sealed_grant_and_registration_permit_artifacts() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let mut admission = HostAdmissionIssuer::new().unwrap();
        let host_token = admission.issue(router).unwrap();
        let host_grant = admission.grant(&host_token, &mut ids).unwrap();
        let host = admission.confirm(&host_grant).unwrap();
        let request = ids.allocate_request(router).unwrap();
        let transaction = ids.allocate_transaction().unwrap();
        let parent = RouteFrameVersion::new(
            router,
            host.version().host(),
            host.top_slot(),
            FrameOrigin::RouterFallback,
            ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        let outlet = crate::slot::RouteSlotKey::named("details").unwrap();
        let mut issuer = MountIssuer::new(router);
        let request_token = issuer
            .issue(
                &mut ids,
                host.version().clone(),
                request,
                transaction,
                parent.clone(),
                outlet.clone(),
            )
            .unwrap();
        let core_grant = issuer.grant(&request_token, &mut ids).unwrap();

        let grant = MountGrant::sealed(core_grant);
        let mut foreign_ids = CoreIdAllocator::new().unwrap();
        let foreign_parent = RouteFrameVersion::new(
            foreign_ids.allocate_router_identity().unwrap(),
            foreign_ids.allocate_host().unwrap(),
            foreign_ids.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            foreign_ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        assert_eq!(parent.router(), foreign_parent.router());
        assert_eq!(parent.host(), foreign_parent.host());
        assert!(mount_grant_matches_target(&grant, &parent, &outlet));
        assert!(!mount_grant_matches_target(
            &grant,
            &parent,
            &crate::slot::RouteSlotKey::named("other").unwrap()
        ));
        assert!(!mount_grant_matches_target(
            &grant,
            &foreign_parent,
            &outlet
        ));
        let slot = grant.slot();
        let receipt = grant.attest_receipt(67);
        let core_permit = issuer.accept_receipt(&receipt.inner).unwrap();
        let permit = MountRegistrationPermit::sealed(core_permit);
        assert_eq!(permit.slot(), slot);

        let completion = permit.complete(71);
        let confirmation = issuer.complete_registration(&completion.inner).unwrap();
        assert_eq!(confirmation.slot(), slot);
        assert!(confirmation.has_valid_binding());
    }

    #[test]
    fn frame_router_match_uses_opaque_identity_not_raw_projection() {
        let mut first = CoreIdAllocator::new().unwrap();
        let first_router = first.allocate_router_identity().unwrap();
        let first_frame = RouteFrameVersion::new(
            first_router,
            first.allocate_host().unwrap(),
            first.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            first.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        let same_router_frame = RouteFrameVersion::new(
            first_router,
            first.allocate_host().unwrap(),
            first.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            first.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        let mut second = CoreIdAllocator::new().unwrap();
        let foreign_frame = RouteFrameVersion::new(
            second.allocate_router_identity().unwrap(),
            second.allocate_host().unwrap(),
            second.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            second.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );

        assert_eq!(first_frame.router(), foreign_frame.router());
        assert!(frame_versions_share_router(
            &first_frame,
            &same_router_frame
        ));
        assert!(!frame_versions_share_router(&first_frame, &foreign_frame));
    }

    fn ready_engine() -> (RouterEngine, HostGrant) {
        let view = RouteViewKey::new("root").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        (engine, grant)
    }

    fn confirmed_engine() -> RouterEngine {
        let (mut engine, grant) = ready_engine();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn settle_push(engine: &mut RouterEngine, location: RouteLocation) -> NavigationRequestId {
        let request = engine.push(location).unwrap();
        engine.collect();
        let ticket = match engine.action().unwrap() {
            EngineAction::FrameTask(task) => engine.report_applied(task).unwrap(),
            EngineAction::MountRequest(_) => panic!("helper expects a depth-one route"),
            EngineAction::FailureCleanup(_) => {
                panic!("depth-one helper cannot hold nested failure cleanup")
            }
            EngineAction::Commit(ticket) => ticket,
        };
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        engine.publish(prepared, || {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        request
    }

    #[test]
    fn host_success_trace_is_ordered_and_uses_canonical_ready_state() {
        let (mut engine, grant) = ready_engine();
        let expected_version = grant.version().clone();
        let expected_slot = grant.top_slot();

        engine.record_top_slot_registered(&grant).unwrap();
        let before_replay = engine.snapshot();
        assert_eq!(
            engine.record_top_slot_registered(&grant),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(engine.snapshot(), before_replay);
        engine.confirm_host(grant);

        let host = engine.snapshot().host().clone();
        assert_eq!(host.version(), Some(&expected_version));
        assert_eq!(host.top_slot(), Some(expected_slot));
        assert_eq!(
            host.trace()
                .iter()
                .map(crate::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::TopSlotRegistered,
                HostTraceEventKind::HostReady,
            ]
        );
        assert!(host.trace().iter().all(|event| !matches!(
            event.kind(),
            HostTraceEventKind::SettlementStarted | HostTraceEventKind::SettlementCompleted
        )));
    }

    #[test]
    fn host_rejection_trace_records_only_successful_workflow_transitions() {
        let view = RouteViewKey::new("root").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();

        let token = engine.request_host_admission().unwrap();
        engine.reject_host_admission(token).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.reject_host_after_mutation(grant).unwrap();

        let host = engine.snapshot().host().clone();
        assert!(host.version().is_none());
        assert!(host.top_slot().is_none());
        assert_eq!(
            host.trace()
                .iter()
                .map(crate::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::AdmissionRejected,
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::AdmissionRejectedAfterMutation,
            ]
        );
    }

    #[test]
    fn request_lookup_bridge_preserves_found_not_retained_and_unknown() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine, RouteLocation::new("/")))
            .collect::<Vec<_>>();

        assert!(matches!(
            engine.request_lookup(requests[0]),
            RequestLookup::NotRetained
        ));
        assert!(matches!(
            engine.request_lookup(requests[2]),
            RequestLookup::Found(_)
        ));

        let mut foreign = confirmed_engine();
        let unknown = foreign.push(RouteLocation::new("/")).unwrap();
        assert!(matches!(
            engine.request_lookup(unknown),
            RequestLookup::Unknown
        ));
    }

    #[test]
    fn frozen_request_retention_classifier_rejects_foreign_and_future_ids() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine, RouteLocation::new("/")))
            .collect::<Vec<_>>();
        let snapshot = engine.snapshot();

        assert_eq!(
            snapshot.request_retention().classify(requests[0]),
            MissingRequestClassification::NotRetained
        );

        let mut foreign = confirmed_engine();
        let foreign_request = foreign.push(RouteLocation::new("/")).unwrap();
        assert_eq!(
            snapshot.request_retention().classify(foreign_request),
            MissingRequestClassification::Unknown
        );

        let future_request = engine.push(RouteLocation::new("/")).unwrap();
        assert_eq!(
            snapshot.request_retention().classify(future_request),
            MissingRequestClassification::Unknown
        );
    }

    #[test]
    fn router_projection_uses_only_current_request_then_clears_at_terminal() {
        let mut engine = confirmed_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();

        let running = engine.snapshot();
        assert_eq!(running.requested(), Some(&RouteLocation::new("/")));
        assert!(running.matched().is_some());
        assert_eq!(running.active().kind(), ActivePresentationKind::None);
        assert_eq!(running.availability(), &RouterAvailability::Ready);
        assert_eq!(
            running.pending_phase(),
            Some(&NavigationPhase::ApplyingFrame(0))
        );
        assert_eq!(running.running_request(), Some(request));
        assert_eq!(running.history().len(), 0);
        assert_eq!(running.earliest_retained_sequence(), None);
        assert!(running.diagnostics().events().is_empty());
        assert!(running.host().version().is_some());

        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first push must issue a frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        engine.publish(prepared, || {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();

        let terminal = engine.snapshot();
        assert!(terminal.requested().is_none());
        assert!(terminal.matched().is_none());
        assert!(terminal.pending_phase().is_none());
        assert!(terminal.running_request().is_none());
        assert_eq!(terminal.active().kind(), ActivePresentationKind::Route);
        assert_eq!(terminal.history().current(), Some(&RouteLocation::new("/")));
        assert_eq!(
            terminal.earliest_retained_sequence(),
            Some(request.sequence())
        );
    }

    #[test]
    fn cleanup_receipt_observation_precedes_aggregate_and_stale_is_zero_write() {
        let mut engine = confirmed_engine();
        settle_push(&mut engine, RouteLocation::new("/"));
        let leaving = engine.active().frames()[0].clone();

        let request = engine
            .push(RouteLocation::new("/").with_fragment("details"))
            .unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("fragment push must issue a frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::new(vec![leaving], 41, 1))
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let stale = cleanup.clone();
        engine.publish(prepared, || {});

        engine
            .observe_cleanup_receipt(&cleanup, CleanupDisposition::Removed)
            .unwrap();
        let receipt_trace = engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            receipt_trace.last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::CleanupSettled)
        );
        assert_eq!(
            receipt_trace.last().unwrap().cleanup_disposition(),
            Some(CleanupDisposition::Removed)
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(cleanup.complete(43, 1)).unwrap();
        let trace = engine.request(request).unwrap().trace().to_vec();
        let cleanup_events = trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .map(NavigationTraceEvent::cleanup_disposition)
            .collect::<Vec<_>>();
        assert_eq!(
            cleanup_events,
            vec![Some(CleanupDisposition::Removed), None]
        );
        assert_eq!(
            trace.last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::RequestTerminal)
        );

        let before_snapshot = engine.snapshot();
        let before_trace = trace;
        assert_eq!(
            engine.observe_cleanup_receipt(&stale, CleanupDisposition::AlreadyAbsent),
            Err(IntegrationError::Stale)
        );
        assert_eq!(engine.snapshot(), before_snapshot);
        assert_eq!(engine.request(request).unwrap().trace(), before_trace);
    }

    #[test]
    fn cleanup_summary_projection_accepts_only_read_only_ledger_facts() {
        let summary = project_cleanup_summary(
            true,
            3,
            Some(CleanupDisposition::QuarantinedForeignDescendant),
        );

        assert!(summary.foreground_pending());
        assert_eq!(summary.quarantine_count(), 3);
        assert_eq!(
            summary.last_disposition(),
            Some(CleanupDisposition::QuarantinedForeignDescendant)
        );
    }

    #[test]
    fn sealed_integration_projects_host_and_root_task_without_raw_authority_mutation() {
        let (mut engine, grant) = ready_engine();
        let expected_host = grant.version().clone();
        let expected_slot = grant.top_slot();

        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };

        assert_eq!(task.request(), request);
        assert_eq!(task.frame().host(), expected_host.host());
        assert_eq!(task.frame().slot(), expected_slot);
        assert_eq!(task.view(), &RouteViewKey::new("root").unwrap());
        assert_eq!(task.take_input().location(), &RouteLocation::new("/"));
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
    }

    #[test]
    fn prepared_publication_runs_the_ecs_mutation_before_infallible_core_publish() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let replay = cleanup.clone().complete_empty();
        let malformed = cleanup.clone().complete(7, 0);
        let mut mutation_ran = false;

        engine.publish(prepared, || mutation_ran = true);

        assert!(mutation_ran);
        assert_eq!(engine.active().location(), Some(&RouteLocation::new("/")));
        assert_eq!(engine.history().len(), 1);
        assert_eq!(engine.history().current(), Some(&RouteLocation::new("/")));
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        assert_eq!(
            engine.complete_cleanup(replay),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(
            engine.complete_cleanup(malformed),
            Err(IntegrationError::Invalid)
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Succeeded
        );
    }

    #[test]
    fn sealed_install_binding_is_used_for_engine_issued_router_fallback() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal root task expected")
        };

        engine
            .report_failed(normal, ViewFailureKind::Build, "normal build failed")
            .unwrap();

        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("router fallback task expected")
        };
        assert_eq!(fallback.request(), request);
        assert_eq!(fallback.view(), &RouteViewKey::new("fallback").unwrap());
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
        assert_eq!(
            fallback.input().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Build)
        );
    }

    #[test]
    fn rejected_manifest_does_not_consume_the_single_commit_ticket() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };
        let ticket = engine.report_applied(task).unwrap();

        let rejected = engine.prepare_commit(&ticket, CleanupManifest::new(Vec::new(), 17, 1));
        assert!(matches!(rejected, Err(IntegrationError::InvalidManifest)));

        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        engine.publish(prepared, || {});
        assert_eq!(engine.history().len(), 1);
    }

    #[test]
    fn queued_push_has_no_lifecycle_action_until_collect() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);

        engine.push(RouteLocation::new("/")).unwrap();
        assert!(engine.action().is_none());

        engine.collect();
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    }
}
