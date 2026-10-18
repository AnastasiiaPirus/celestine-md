# Reactive idle rendering

Celestine should settle to near-zero CPU use when a visible editor has no work to
perform. The app should request frames for input, file-system events, state
changes, and active animation, then sleep while idle. A visible caret blink and
other active animations are legitimate work; a fully settled graph and overlays
should not keep the UI busy.

File-system watching supports this contract without making correctness depend on
the watcher. Check behavior by creating, editing, renaming, and deleting Markdown
files and images, and by adding or removing directories while the app is idle.
Updates should appear without keyboard input. If a watcher fails or the OS limits
watch resources, the app must reconcile the vault so changes are eventually
visible. Watcher recovery and event delivery are separate checks from idle CPU.

## Reproducible CPU measurement

1. Use the same visible 1280×800 window and a copied 27-note vault, with the
   same note open in edit view and both sidebars visible. Keep focus outside
   the text editor so no caret blinks, and wait for startup work to settle.
   Do not type or interact during a sample.
2. Measure process CPU time and monotonic wall time over a 20-second interval:
   `100 × Δprocess CPU seconds / Δmonotonic seconds`. Report the result as
   percent of one core; 100% means one core was fully occupied.
3. Take three 20-second samples under the same conditions and report all three
   results and their median. Record the OS, build type, window visibility, and
   whether the app was unobscured.
4. Profile separately to identify remaining work. Repeat with the window
   occluded or blocked as a distinct condition; do not compare that result with
   the visible idle measurement.

## Local release-build result

Measured on macOS 26.5.1 (Apple silicon), using the upstream `80936b1` baseline
and this implementation, both optimized with release debug information. Each
sample lasted 20 seconds; updates were disabled with `CELESTINE_NO_UPDATE=1`.
The copied vault contained 27 Markdown notes totaling 56,670 bytes.

| Build | CPU samples (% of one core) | Median |
| --- | --- | --- |
| Upstream baseline | 13.22, 12.66, 12.72 | 12.72% |
| Reactive rendering | 0.00, 0.00, 0.00 | 0.00% |

Readings near zero are the measurement floor, not proof of zero CPU work. This
is a small-vault, visible-idle result; it does not predict large-vault startup,
active editing, or degraded watcher-mode costs. Re-run the same workload when
changing the watcher or frame scheduler.
