# Router integration implementation plan

[简体中文](plan.md) | English

Status: in progress. Scope and semantics are in the [design](design.en.md).

1. **Declaration and startup:** implement BSN declaration compilation, shared installation, and one-shot initial navigation. Preserve defaults and explicit hosts. Validate declaration errors, retry after failure, multiple routers, and real nested pages.
2. **Navigation and addresses:** add internal-address splitting and Commands navigation. Validate reading inputs, mutating components, and navigating in one page system, including deferred error handling.
3. **Complete integration:** migrate quick start, nested/window examples, and an independent consumer. Establish one recommended entry, complete common exports, and update bilingual contracts and flow descriptions. Run relevant crates, public journeys, consumers, formatting, and static checks.
4. **Dynamic pages and animation:** preserve current-input and explicit-scroll contracts while reducing example repetition. Verify a mature animation dependency's start, completion, and synchronous cancellation before adding an optional adapter and application-defined effect examples. Record concrete dependency blockers without expanding Router responsibilities.
5. **Finish:** validate combinations according to behavior risk, check feature dependencies, examples, and bilingual documents. Migrate lasting rules and remove completed plans. Commit independent deliveries; the main agent owns all builds and commits.

Generic RouteQuery, typed-route macros, business-loading frameworks, and new lifecycle state machines are outside this work. Performance checks target actual scheduling costs of the new entry points instead of repeating previous full measurements.
