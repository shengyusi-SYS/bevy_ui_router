//! Request-independent settlement of a retired, idle host grant.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdleHostLossSource {
    grant: RetiredHostAuthorityProjection,
    state: RouterNavigationState,
    inventory: HostAdmissionUnavailableOwnerInventoryProjection,
    history: FrozenCommittedHistoryProjection,
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
            && self.state.availability() == &RouterAvailability::Ready
            && self.state.current_request().is_none()
            && self
                .observed_unavailable_owner_inventory()
                .is_observed_unavailable_idle()
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
        let (Some(host), Some(slot)) = (self.state.host().cloned(), self.state.top_slot()) else {
            return Err((IdleHostLossErrorKind::ReceiverMismatch, lease));
        };
        if !self.idle_host_loss_eligible() || !lease.matches(self.router, &host, slot) {
            return Err((IdleHostLossErrorKind::ReceiverMismatch, lease));
        }
        if !roots.iter().eq(self.state.active().frames().iter().rev()) || obligation_count == 0 {
            return Err((IdleHostLossErrorKind::InvalidManifest, lease));
        }
        if self.router_diagnostics.try_reserve(1).is_err() {
            return Err((IdleHostLossErrorKind::RetryableCapacity, lease));
        }
        let diagnostic = DiagnosticEvent::host_or_slot_unavailable("idle host authority lost");
        let mut recovering = self.state.clone();
        if !recovering.begin_authority_recovery(&host, slot) {
            return Err((IdleHostLossErrorKind::ReceiverMismatch, lease));
        }
        recovering.set_active(ActivePresentation::none());
        let mut terminal_state = recovering.clone();
        if !terminal_state.complete_host_loss_settlement(&host, slot) {
            return Err((IdleHostLossErrorKind::ReceiverMismatch, lease));
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
        let source = IdleHostLossSource {
            grant: lease.retirement_projection(),
            state: recovering.clone(),
            inventory: self.observed_unavailable_owner_inventory(),
            history: self.freeze_committed_history(),
            branch: self
                .active_branch
                .as_ref()
                .map(failure_cleanup_zero_terminal_branch_projection),
            slots: self.active_child_slots.clone(),
            records: self.records.clone(),
            manifest_digest,
            obligation_count,
        };
        let receiver = source.clone();
        let retired = issuer
            .retire_current(lease)
            .map_err(|error| (IdleHostLossErrorKind::ReceiverMismatch, error.into_owner()))?;
        install(IdleHostLossOwner {
            retired,
            source,
            terminal_state,
            admission_source,
        });
        self.pending_idle_host_loss = Some(receiver);
        if self.router_diagnostics.len() == ROUTER_DIAGNOSTIC_RETENTION {
            self.router_diagnostics.remove(0);
        }
        self.router_diagnostics.push(diagnostic);
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
            || owner.retired.projection() != source.grant
            || self.state != source.state
            || self.observed_unavailable_owner_inventory() != source.inventory
            || self.freeze_committed_history() != source.history
            || self
                .active_branch
                .as_ref()
                .map(failure_cleanup_zero_terminal_branch_projection)
                != source.branch
            || self.active_child_slots != source.slots
            || self.records != source.records
            || facts.manifest_digest != source.manifest_digest
            || facts.obligation_count != source.obligation_count
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
        self.active_branch = None;
        self.active_child_slots.clear();
        self.host_admission_source = owner.admission_source;
        self.host_admission_source_open = false;
        self.state = owner.terminal_state;
        Ok(installed)
    }
}
