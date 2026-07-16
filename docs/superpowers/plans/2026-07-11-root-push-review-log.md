# Root Push Review Log

**结果**: Root Push Task PASS

**日期**: 2026-07-16

**范围**: `b56ce1c` 起的 Root Push Tasks 1–12

**边界**: 不声明 Nested Journey PASS 或 MVP PASS

## 1 Completion evidence

最终新鲜门禁

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| workspace all-target/all-feature check | PASS |
| workspace all-feature nextest | 442/442 PASS，0 skipped |
| workspace all-feature Cargo tests + doctests | PASS |
| facade export/manifest/source/positive gate | 45/45 PASS |
| facade negative/authority gate | 39/39 PASS |
| Root public journeys | 4/4 PASS |
| ECS navigation lifecycle tests | 30/30 PASS |
| authority verifier | catalogue=19，registered=19，passed=19 |
| `git diff --check` | PASS |

补充事实

- locked Bevy source 只有 exact revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`
- `ghost_nodes` 不存在
- facade default graph 保持 Bevy-free
- ECS normal `bevy_ui/bevy_picking` 与 dev-only `bevy_internal` feature graph 符合 Root contract
- `Cargo.lock` 无漂移

## 2 Environment and execution notes

- 用户 Cargo 配置的 `sccache` 在沙箱不可执行，最终 Cargo 命令使用 `--config "build.rustc-wrapper = ''"`
- parallel workspace build 曾触发 Windows resource/OOM，最终 nextest 使用 `--build-jobs 1`，Cargo test 使用 `-j 1`
- authority verifier 首次受 PowerShell execution policy 拒绝，最终使用进程级 `-ExecutionPolicy Bypass`
- nextest 首次发现两个 test binaries 并发写同一 proc-macro fixture DLL，改为每进程、每次调用唯一输出目录后，同名并发测试 2/2 与完整 nextest 442/442 通过

历史计划中的 `facade_features`、`facade_compile_fail_boundary`、ECS `apply_bridge`、View `compile_fail_boundary`、`root_navigation_surface` 与 Task 9 `cutover` filter 已由 Task 11 固定 target 和 `autotests = false` 取代

这些旧命令不再作为零测试或不存在 target 伪装执行，等价合同由当前固定 facade、package、workspace 与 isolated-consumer gates 覆盖

## 3 Independent reviews

| Role | Initial result | Disposition | Final result |
| --- | --- | --- | --- |
| Bevy/manifest facts | 1 Important | 固定 gate 遍历 normal/dev/build 与全部 target tables，识别 package alias，冻结 exact dependency graph | PASS |
| Authority/lifecycle | 3 Important，复审追加 2 Important | pending/leaving role settlement、provisional host cleanup owner、foreign descendant focus closure、protected foreign component quarantine 与 atomic focus release | PASS |
| Implementation/test adequacy | 3 Important，复审追加 1 Important | production stale-report preflight、public host fingerprint、native target event、opaque report identity | PASS |
| Facade capability closure | 3 Important，多轮复审发现合法 Rust source forms | 统一 `syn::Visit` production walker、logical module graph、lexical binding、macro/attribute classification、完整 trait type graph 与 compiler-backed mutations | PASS |

所有 finding 均先由可达路径或 compiler-backed mutation 复现，再修正并交回独立审查者复核

## 4 Root PASS boundary

Root Push 已完成

- cleanup terminality、fallback、diagnostics 与重复 push
- 公开 facade journeys 与 app-owned host zero-write 证明
- Root-profile facade、manifest、feature、signature 与 authority catalogue closure
- single lifecycle authority、exact claims、registry-first staging、post-write validation、atomic mutation 与 input/focus isolation

仍待后续切片

- Nested
- Reconciliation
- History replace/back
- Recovery
- Authority closure
- final MVP gate

Task 10 implementation commit: `ba40f56`

Task 11 facade commit: `8ff82dc`

Task 12 completion: 本 review log 所在提交
