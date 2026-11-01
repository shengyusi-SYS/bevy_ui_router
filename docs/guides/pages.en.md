# Page state and scrolling

English | [简体中文](pages.md)

After the [two-page window tutorial](integration.en.md), use the features below as needed. Snippets reuse the tutorial's `page_content`; import types from `bevy::prelude::*` and `bevy_ui_router::prelude::*`.

## Read the current path and parameters

Use `RouteInputs` to read the current path and parameters. The page-building closure receives values **at creation time**; those values may change when a layout is reused or a retained page returns.

This system updates text carrying a `RouteLabel` marker. Add that marker to the page's `Text` entity where you want the path displayed:

```rust
#[derive(Component, Clone, Default)]
struct RouteLabel;

fn update_route_labels(
    inputs: RouteInputs,
    mut labels: Query<(Entity, &mut Text), With<RouteLabel>>,
) {
    for (entity, mut text) in &mut labels {
        let Some(input) = inputs.current(entity) else { continue };
        text.set_if_neq(Text::new(input.location().path()));
    }
}
```

Register it after snapshots are published to read this update's navigation state:

```rust
app.add_systems(
    Update,
    update_route_labels.after(RouterScheduleSet::PublishSnapshots),
);
```

Within `page("items/:id", ...)`, use `input.params().get("id")`. Query strings and fragments are available through `input.location().search()` and `.fragment()`. Parameters belong to the current route level; an outer layout does not automatically receive its child's parameters.

`current(entity)` returns input only for active pages. If you only need a yes/no answer, use `RouteActivity::is_active(entity)`.

## Retain page state

Suppose Settings contains an input field and should keep its contents when you leave and return. Change its declaration to:

```rust
page("settings", |_| Ok(page_content("Settings"))).keep_alive()
```

Then add `.keep_alive_capacity(2)` to `BsnRouter`. Both settings are needed: the first opts a page into retention, and the second limits the number of inactive instances. Capacity defaults to 0. A retained hit reuses the original Entities and components without calling the page-building closure again.

The tutorial's outer layout stays present, so Settings can be retained beneath it. In deeper nesting, parents and children opt in separately and each counts toward capacity. Clearing a parent also clears retained pages beneath it. A root page attached directly to the host, with no parent route layout, cannot be retained.

Shared layouts already stay in place when switching between their child pages; that reuse does not require `.keep_alive()`. See the [retention reference](../contracts/navigation.en.md#retained-instances) for capacity and eviction rules.

### Keep a page when its query changes

Query changes rebuild pages by default. To keep `/items?sort=name` when switching to `/items?sort=date` and update the list yourself, add `.preserve_on_search()` to its `page(...)` declaration.

Enable it at every level you want to reuse, from the outer layout to the target page. Current and retained pages follow the same policy. Update reused content through `RouteInputs`, as shown above. Path-parameter changes or changes to the deepest page's fragment can still rebuild content; see the [navigation reference](../contracts/navigation.en.md#matching-and-location-updates).

## Load content and reactivate pages

Use your application's Bevy tasks and systems to load data: create a page or loading indicator, then apply the completed data.

Retained pages remain in the World. Ordinary queries and background tasks do not automatically pause when a page leaves. A page system can check `inputs.current(entity)` and skip the update when it returns `None`. Before applying an async result, check that it still belongs to the intended page and data version. Copy paths or parameters into owned values before carrying them across `await`.

The [nested example](../../crates/bevy_ui_router/examples/bsn_nested.rs) demonstrates delayed loading.

## Restore scroll positions

Router can restore the position from **that visit** when navigating Back. Separate visits to the same page keep separate positions.

Add the marker to an **existing scroll container inside a routed page**:

```rust
commands.entity(scroll_container).insert(RouteScroll::new("body"));
```

`scroll_container` is that container's Entity. It should already have `ScrollPosition` and a `Node` with `overflow: Overflow::scroll_y()` (or horizontal scrolling as appropriate). Content, dimensions, and wheel handling stay in your existing implementation.

`"body"` names the container and must be unique and stable within that page; multiple containers might use `"body"` and `"sidebar"`. By default, restoration is attempted once on the first normal layout after returning, clamped to the scrollable range. Push/Replace do not apply Back's saved positions.

### Wait for content before restoring

For a list that needs data first, use explicit restoration:

```rust
RouteScroll::new("body")
    .with_restore_policy(RouteScrollRestorePolicy::Explicit)
```

Send ready after your application creates the content. This example tracks loading with an application-owned `PageState`:

```rust
#[derive(Component)]
struct PageState {
    content_ready: bool,
}

fn allow_restore(
    restores: RouteScrollRestorations,
    mut commands: Commands,
    pages: Query<(Entity, &PageState)>,
) {
    for (entity, page) in &pages {
        if page.content_ready
            && let Some(restore) = restores.pending(entity, "body")
        {
            commands.ready_route_scroll(restore.id());
        }
    }
}
```

Add `PageState` to an entity inside the page and register `allow_restore` in `Update`. Set `content_ready` to `true` after creating the content. The query accepts an entity inside the page; the scroll container need not exist yet.

If the user scrolls while waiting, cancel the restoration in your wheel handler using the queried ID:

```rust
commands.cancel_route_scroll(restore.id());
```

Cancel even at a scroll boundary where the position has not changed, so content does not jump back later. Both ready and cancel use the ID you just queried. Restoration runs once and does not keep chasing the saved position as content grows.

<details>
<summary>Ordering custom systems</summary>

- Create UI and apply Commands in `Update` or before `UiSystems::Prepare`, allowing camera information to propagate.
- Send ready/cancel before `RouterScrollSet::Prepare`.
- To override this restoration's position, write after `RouterScrollSet::Prepare` and before Layout.
- Read the clamped position after normal Layout.

Normal UI layout and a valid camera are required. Navigation completion does not mean layout has finished for the update; see the [scroll reference](../contracts/navigation.en.md#scroll-restoration-by-visit).

</details>

## Input and focus

Ordinary mouse interaction uses Bevy UI picking. Router excludes inactive pages from hits and cancels gestures that began on departing pages.

A page control's keyboard or gamepad handler checks its target with `RouterInputGate::allows_keyboard(entity)` or `allows_gamepad(entity)`. Schedule it in `PreUpdate` after `RouterInputSet::Ready`. Add, hide, or move content out before input refresh. Application-wide shortcuts use the application's own handlers.

Install `InputFocusPlugin` when using focus and choose the initial target. Router releases focus when a page leaves. If using `TabNavigationPlugin` or `InputDispatchPlugin`, check the same input eligibility during target selection and event dispatch; those two integrations are not automatic.

See the page keyboard handler in the [windowed example](../../examples/windowed/src/main.rs) and the interaction checks in [debugging](debugging.en.md).
