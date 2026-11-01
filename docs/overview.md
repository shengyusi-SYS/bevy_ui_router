# Overview

English | [简体中文](overview.zh-CN.md)

Bevy UI Router brings page organization, navigation, and back history into one routing system. Your application supplies page Scenes and a host that displays them. Router selects content from the route tree and maintains the current page and visit history.

## Pages, layouts, and visits

For routes such as `/projects` and `/projects/:id`, a parent layout can hold navigation controls while a child page displays a list or project details. Moving between child pages preserves the shared layout and its local state. `RouteInputs` lets that reused layout read the new path, its own route-local parameters, and query information.

`push` adds a visit, `replace` replaces the current visit, and `back` returns to the previous history entry. History is kept in memory. Visits and page instances are separate: several visits can reuse the same page instance while keeping their own scroll positions.

| Concept | Meaning |
| --- | --- |
| Route | An entry in the route tree that defines matching and a view |
| Scene / factory | Page construction content / a function producing a new Scene for each build |
| Host | A UI container supplied by the application |
| Outlet | A place inside a layout for its child page |
| Frame | One page or layout instance managed by Router |
| History entry | A visit recorded by successful navigation |
| Publication | The point when a prepared page becomes the current presentation |

## Available features

- Static paths, path parameters, trailing wildcards, index routes, and pathless layouts.
- A single nested branch with one default outlet per layout; shared prefixes stay in place while changed suffixes are replaced.
- `push`, `replace`, and `back`, plus current-state, request-result, and diagnostic snapshots.
- Synchronous, code-authored Bevy Scene/BSN pages, route error views, and a Router fallback.
- Independent keep-alive choices for layouts and child pages, with capacity and parent-dependency management. The default inactive capacity is zero.
- Per-visit two-dimensional scroll restoration, including late containers, explicit content readiness, cancellation, and application-initiated scrolling.
- Page activity queries, native pointer isolation, gesture revocation on exit, and focus release. Keyboard and gamepad handlers use an explicit input gate.
- Optional [transition lifecycle](guides/transitions.md): preparation before first display, exit presence, and completion/cancellation; applications implement the effects.
- Optional mouse back-button and Android legacy Back input, with the application selecting a Router for the source window.

Application data loads through ordinary Bevy tasks and systems. Tasks return data; systems install it in the World. The application configures windows, themes, cameras, business data, and background-work policies.

## Choose an integration layer

For Bevy UI, start with the `bsn` feature of `bevy_ui_router`. Core makes routing decisions, View defines renderer-neutral contracts, ECS changes entities, BSN constructs Scenes, and Platform carries platform back intents. The default `view` feature and Core/View use types independent of Bevy.

Select a Router version using the [compatibility table in the integration guide](guides/integration.md#bevy-version-requirements). See [verification](development/verification.md) for toolchain and platform coverage.

The [roadmap](roadmap.md) also covers browser URL/history integration, asynchronous Scene loading, typed routes, and multiple outlets. Start with [integration](guides/integration.md), or consult the [behavior contracts](contracts/README.md) for detailed rules.
