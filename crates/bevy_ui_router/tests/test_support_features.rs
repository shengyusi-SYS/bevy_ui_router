use bevy_ui_router::prelude::{
    AuditEntityId, AuthorityContentMapping, AuthorityFrameMapping, AuthorityHostMapping,
    AuthoritySlotMapping, RouterAuthorityAuditSnapshot, RouterAuthorityAudits,
    RouterLifecycleAuditEvent, RouterLifecycleAuditEventKind,
};

#[test]
fn root_test_support_exposes_only_root_authority_observations() {
    fn public<T>() {}
    public::<AuditEntityId>();
    public::<AuthorityContentMapping>();
    public::<AuthorityFrameMapping>();
    public::<AuthorityHostMapping>();
    public::<AuthoritySlotMapping>();
    public::<RouterAuthorityAuditSnapshot>();
    public::<RouterAuthorityAudits>();
    public::<RouterLifecycleAuditEvent>();
    public::<RouterLifecycleAuditEventKind>();
}
