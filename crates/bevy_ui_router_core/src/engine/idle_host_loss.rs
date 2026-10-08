//! Request-independent settlement of a retired, idle host grant.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdleHostLossSource {
    grant: RetiredHostAuthorityProjection,
    state: RouterNavigationState,
    inventory: HostAdmissionUnavailableOwnerInventoryProjection,
    history: FrozenCommittedHistoryProjection,
    history_revision: HistoryRevision,
    history_keys: CanonicalHistoryView,
    retained_leaves: Vec<retained::RetainedRegistryEntry>,
    keep_alive_capacity: usize,
    admission_source: HostAdmissionSourceProjection,
    admission_source_open: bool,
    branch: Option<FailureCleanupZeroTerminalBranchProjection>,
    slots: Vec<LogicalChildSlotBinding>,
    records: VecDeque<NavigationRequestRecord>,
    manifest_digest: u64,
    obligation_count: usize,
}

pub(crate) struct IdleHostLossOwner {
    retired: RetiredHostAuthority,
    source: IdleHostLossSource,
    terminal_state: RouterNavigationState,
    admission_source: HostAdmissionSourceProjection,
    retained_owners: Vec<retained::RetainedLeafOwner>,
    retained_after: Vec<retained::RetainedRegistryEntry>,
}

/// Frozen, request-independent transfer of the current host's inactive leaves.
/// Its projections cannot recreate a retained owner or retire a host grant.
#[derive(Debug)]
pub struct RetainedIdleHostLossPlan {
    source: IdleHostLossSource,
    logical_roots: Vec<RouteFrameVersion>,
    retiring: Vec<retained::RetainedLeafProjection>,
    retained_after: Vec<retained::RetainedRegistryEntry>,
}

impl RetainedIdleHostLossPlan {
    pub fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.logical_roots
    }

    pub fn retiring(&self) -> &[retained::RetainedLeafProjection] {
        &self.retiring
    }
}

pub(crate) struct IdleHostLossTerminal {
    pub(crate) grant: RetiredHostAuthorityProjection,
    pub(crate) physical: IdleHostLossPhysicalFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleHostLossPhysicalDisposition {
    Settled {
        completion_digest: u64,
        settled_count: usize,
    },
    SafeResidual {
        observation_digest: u64,
    },
    IsolationUnproven {
        observation_digest: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdleHostLossPhysicalFacts {
    pub manifest_digest: u64,
    pub obligation_count: usize,
    pub disposition: IdleHostLossPhysicalDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleHostLossErrorKind {
    ReceiverMismatch,
    InvalidManifest,
    RetryableCapacity,
}

impl RouterEngine {
    pub(crate) fn idle_host_loss_eligible(&self) -> bool {
        self.pending_idle_host_loss.is_none()
            && self
                .retained_leaves
                .iter()
                .all(retained::RetainedRegistryEntry::is_live)
            && self.pending_retained_transfer.is_none()
            && self.state.availability() == &RouterAvailability::Ready
            && self.state.current_request().is_none()
            && self
                .observed_unavailable_owner_inventory()
                .is_observed_unavailable_idle()
    }

    fn observe_idle_host_loss_source(
        &self,
        grant: RetiredHostAuthorityProjection,
        manifest_digest: u64,
        obligation_count: usize,
    ) -> IdleHostLossSource {
        IdleHostLossSource {
            grant,
            state: self.state.clone(),
            inventory: self.observed_unavailable_owner_inventory(),
            history: self.freeze_committed_history(),
            history_revision: self.history.revision(),
            history_keys: self.history.entry_key_projection(),
            retained_leaves: self.retained_leaves.clone(),
            keep_alive_capacity: self.keep_alive_capacity,
            admission_source: self.host_admission_source.clone(),
            admission_source_open: self.host_admission_source_open,
            branch: self
                .active_branch
                .as_ref()
                .map(failure_cleanup_zero_terminal_branch_projection),
            slots: self.active_child_slots.clone(),
            records: self.records.clone(),
            manifest_digest,
            obligation_count,
        }
    }

    pub(crate) fn prepare_retained_idle_host_loss_plan(
        &self,
        lease: &HostLease,
        inventory: &[&retained::RetainedLeafOwner],
    ) -> Result<RetainedIdleHostLossPlan, IdleHostLossErrorKind> {
        let (Some(host), Some(slot)) = (self.state.host(), self.state.top_slot()) else {
            return Err(IdleHostLossErrorKind::ReceiverMismatch);
        };
        if !self.idle_host_loss_eligible() || !lease.matches(self.router, host, slot) {
            return Err(IdleHostLossErrorKind::ReceiverMismatch);
        }
        if !retained::owner_projections_match(
            inventory.iter().copied(),
            self.retained_leaves
                .iter()
                .map(retained::RetainedRegistryEntry::projection),
        ) {
            return Err(IdleHostLossErrorKind::InvalidManifest);
        }
        let mut retiring = Vec::new();
        let mut retained_after = Vec::new();
        for entry in &self.retained_leaves {
            let projection = entry.projection();
            if projection.host() == host {
                if !retained::scope_survives(
                    projection,
                    host,
                    self.state.active().frames(),
                    &self.active_child_slots,
                ) {
                    return Err(IdleHostLossErrorKind::InvalidManifest);
                }
                retiring.push(projection.clone());
            } else {
                retained_after.push(entry.clone());
            }
        }
        let mut logical_roots = self
            .state
            .active()
            .frames()
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        logical_roots.extend(retiring.iter().map(|projection| projection.frame().clone()));
        if logical_roots
            .iter()
            .enumerate()
            .any(|(index, frame)| logical_roots[..index].contains(frame))
        {
            return Err(IdleHostLossErrorKind::InvalidManifest);
        }
        Ok(RetainedIdleHostLossPlan {
            source: self.observe_idle_host_loss_source(lease.retirement_projection(), 0, 0),
            logical_roots,
            retiring,
            retained_after,
        })
    }

    pub(crate) fn begin_idle_host_loss<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        roots: &[RouteFrameVersion],
        manifest_digest: u64,
        obligation_count: usize,
        install: F,
    ) -> Result<(), (IdleHostLossErrorKind, HostLease)>
    where
        F: FnOnce(IdleHostLossOwner),
    {
        if !self.retained_leaves.is_empty() || self.pending_retained_transfer.is_some() {
            return Err((IdleHostLossErrorKind::ReceiverMismatch, lease));
        }
        let plan = match self.prepare_retained_idle_host_loss_plan(&lease, &[]) {
            Ok(plan) => plan,
            Err(kind) => return Err((kind, lease)),
        };
        self.begin_idle_host_loss_with_retention(
            issuer,
            lease,
            plan,
            roots,
            manifest_digest,
            obligation_count,
            Vec::new(),
            install,
        )
        .map_err(|(lease, _, _, kind)| (kind, lease))
    }

    pub(crate) fn begin_idle_host_loss_with_retention<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        plan: RetainedIdleHostLossPlan,
        roots: &[RouteFrameVersion],
        manifest_digest: u64,
        obligation_count: usize,
        owners: Vec<retained::RetainedLeafOwner>,
        install: F,
    ) -> Result<
        (),
        (
            HostLease,
            RetainedIdleHostLossPlan,
            Vec<retained::RetainedLeafOwner>,
            IdleHostLossErrorKind,
        ),
    >
    where
        F: FnOnce(IdleHostLossOwner),
    {
        let (Some(host), Some(slot)) = (self.state.host().cloned(), self.state.top_slot()) else {
            return Err((lease, plan, owners, IdleHostLossErrorKind::ReceiverMismatch));
        };
        if !self.idle_host_loss_eligible()
            || !lease.matches(self.router, &host, slot)
            || self.observe_idle_host_loss_source(lease.retirement_projection(), 0, 0)
                != plan.source
        {
            return Err((lease, plan, owners, IdleHostLossErrorKind::ReceiverMismatch));
        }
        if roots != plan.logical_roots.as_slice()
            || !retained::owner_projections_match(owners.iter(), plan.retiring.iter())
            || obligation_count == 0
        {
            return Err((lease, plan, owners, IdleHostLossErrorKind::InvalidManifest));
        }
        if self.router_diagnostics.try_reserve(1).is_err() {
            return Err((
                lease,
                plan,
                owners,
                IdleHostLossErrorKind::RetryableCapacity,
            ));
        }
        let diagnostic = DiagnosticEvent::host_or_slot_unavailable("idle host authority lost");
        let mut recovering = self.state.clone();
        if !recovering.begin_authority_recovery(&host, slot) {
            return Err((lease, plan, owners, IdleHostLossErrorKind::ReceiverMismatch));
        }
        recovering.set_active(ActivePresentation::none());
        let mut terminal_state = recovering.clone();
        if !terminal_state.complete_host_loss_settlement(&host, slot) {
            return Err((lease, plan, owners, IdleHostLossErrorKind::ReceiverMismatch));
        }
        let incarnation = self
            .host_admission_source
            .availability_incarnation
            .0
            .checked_add(1);
        let admission_source = HostAdmissionSourceProjection {
            router: self.router,
            availability_incarnation: AvailabilityIncarnation(
                incarnation.unwrap_or(self.host_admission_source.availability_incarnation.0),
            ),
            kind: if incarnation.is_some() {
                HostAdmissionSourceKind::RetryableHostLost
            } else {
                terminal_state
                    .fail_closed_unavailable(RouterUnavailableReason::AuthorityEstablishmentFailed);
                HostAdmissionSourceKind::PermanentIdentityExhausted
            },
            public_reason: Some(if incarnation.is_some() {
                RouterUnavailableReason::HostLost
            } else {
                RouterUnavailableReason::AuthorityEstablishmentFailed
            }),
            host: Some(host),
            top_slot: Some(slot),
        };
        let mut source = plan.source.clone();
        source.state = recovering.clone();
        source.manifest_digest = manifest_digest;
        source.obligation_count = obligation_count;
        let receiver = source.clone();
        let retired = match issuer.retire_current(lease) {
            Ok(retired) => retired,
            Err(error) => {
                return Err((
                    error.into_owner(),
                    plan,
                    owners,
                    IdleHostLossErrorKind::ReceiverMismatch,
                ));
            }
        };
        install(IdleHostLossOwner {
            retired,
            source,
            terminal_state,
            admission_source,
            retained_owners: owners,
            retained_after: plan.retained_after,
        });
        self.pending_idle_host_loss = Some(receiver);
        append_router_diagnostic(
            &mut self.router_diagnostics,
            &mut self.report_dispositions,
            diagnostic,
        );
        self.state = recovering;
        Ok(())
    }

    pub(crate) fn complete_idle_host_loss<F, T>(
        &mut self,
        owner: IdleHostLossOwner,
        facts: IdleHostLossPhysicalFacts,
        install: F,
    ) -> Result<T, IdleHostLossOwner>
    where
        F: FnOnce(IdleHostLossTerminal) -> T,
    {
        let source = &owner.source;
        if self.pending_idle_host_loss.as_ref() != Some(source)
            || self.pending_retained_transfer.is_some()
            || self.observe_idle_host_loss_source(
                owner.retired.projection(),
                facts.manifest_digest,
                facts.obligation_count,
            ) != *source
            || !owner
                .retained_owners
                .iter()
                .map(retained::RetainedLeafOwner::projection)
                .eq(source
                    .retained_leaves
                    .iter()
                    .map(retained::RetainedRegistryEntry::projection)
                    .filter(|projection| projection.host() == source.grant.version()))
            || matches!(facts.disposition, IdleHostLossPhysicalDisposition::Settled { settled_count, .. } if settled_count != source.obligation_count)
        {
            return Err(owner);
        }
        let terminal = IdleHostLossTerminal {
            grant: owner.retired.projection(),
            physical: facts,
        };
        let installed = install(terminal);
        self.pending_idle_host_loss = None;
        self.retained_leaves = owner.retained_after;
        self.active_branch = None;
        self.active_child_slots.clear();
        self.host_admission_source = owner.admission_source;
        self.host_admission_source_open = false;
        self.state = owner.terminal_state;
        Ok(installed)
    }
}
