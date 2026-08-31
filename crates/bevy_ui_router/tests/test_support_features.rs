use bevy_ui_router::prelude::{
    AuditEntityId, AuthorityContentMapping, AuthorityFrameMapping, AuthorityHostMapping,
    AuthoritySlotMapping, InputProbeResult, QuarantineLeaseId, QuarantineOwnerKind,
    QuarantineOwnerSnapshot, QuarantineSnapshot, RouterAuthorityAuditSnapshot,
    RouterAuthorityAudits, RouterInputModality, RouterInputProbes, RouterLifecycleAuditEvent,
    RouterLifecycleAuditEventKind,
};

#[test]
fn root_test_support_exposes_the_complete_authority_observation_closure() {
    fn public<T>() {}
    fn resource<T: bevy_ecs::resource::Resource>() {}

    public::<AuditEntityId>();
    public::<AuthorityContentMapping>();
    public::<AuthorityFrameMapping>();
    public::<AuthorityHostMapping>();
    public::<AuthoritySlotMapping>();
    public::<InputProbeResult>();
    public::<QuarantineLeaseId>();
    public::<QuarantineOwnerKind>();
    public::<QuarantineOwnerSnapshot>();
    public::<QuarantineSnapshot>();
    public::<RouterAuthorityAuditSnapshot>();
    public::<RouterAuthorityAudits>();
    public::<RouterInputModality>();
    public::<RouterInputProbes>();
    public::<RouterLifecycleAuditEvent>();
    public::<RouterLifecycleAuditEventKind>();

    resource::<RouterAuthorityAudits>();
    resource::<RouterInputProbes>();
}
