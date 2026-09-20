# `morph-codegen`: The Crate That Writes C++

**Part of:** [Dev Docs](../architecture/overview.md)

`morph-codegen` (`crates/morph-codegen/src/`) is the last Rust your code meets. It takes `IRWindow` trees from `morph-ir` ([crate-ir](crate-ir.md)) and writes the actual deliverables: `app.cpp`, `_morph_state.h`, and `morph_api.h` — plus the `MORPH_FEATURE_*` selection that decides how much runtime your binary ships. Two emitters do the writing, one scanner decides the budget, and a template holds the skeleton. This page maps all four, function by function.

## The assembly line (`cpp/mod.rs`): `CppEmitter::emit`

`CppEmitter::new(windows).emit(output_dir)` runs the whole backend in order:

1. **Scan features.** `FeatureSet::scan(windows)` → `required_headers()` + `required_defines()`. Only used runtime features get compiled in — the guest-list model from [The Build Machine](../architecture/build-system.md).
2. **Collect state decls.** Every `state_vars` entry becomes `{signal_name: __st_<name>, type, init}` (via `infer_cpp_type`; array-literal `JsArray` inits normalize to `JsArray{}`). Shared vars resolve through `shared_expr(ns, accessor)`; module bindings (functions/vars/classes) rewrite to their defining namespaces (aliases contribute nothing — they were never local).
3. **Dedupe events.** `event_decls` deduped by identity key; then the **channel lowering**: every `morph::channel("<id>")` placeholder (shared with the dev TU) is string-replaced with its static accessor expression (`event_expr(ns, accessor)`), so *no string lookup survives in `app.cpp`*. The build TU speaks in statics; only the dev TU keeps the string registry.
4. **Build the `state_map`.** Getters → `__st_<name>.get()` (or `{shared}.get()`), setters → `__st_<name>.set` (or `{shared}.set`). This map is the *only* place user-visible names become C++ — every downstream emitter just substitutes through it.
5. **Render the template.** `app_main.cpp.tera` (baked in via `include_str!("../../templates/app_main.cpp.tera")`) filled with window code from `node_emitter`, logic from `logic_emitter`, `shared_decls`, keyframes, and the self-test (`generate_self_test` — the `--morph-self-test` body).
6. **Write the headers.** `generate_morph_api_header` (plus `generate_morph_api_header_dev` for the dev TU): shared-store accessors, `emit_`/`notify_` channel wrappers, `MID_*` constants with `mid_header_entries`, `using`-aliases for re-exports (`alias_wrapper`), `extern` var decls, with class entries relocated correctly (`move_class_entry`, `premain_names`).

## The scene painter (`node_emitter.rs`): `emit_node`

`emit_node` / `emit_node_with_state` turn each `IRNode` into C++ node construction. The supporting cast:

| Function group | Job |
|---|---|
| `set_style` | The big one: every `IRStyle` field → setter calls (plus `emit_hover_style`, `emit_active_style`, `conditional_flex_passthrough`) |
| `emit_conditional`, `emit_list` | Branch and keyed-list emission (`list_key_uses_actual_callback_params`, member access normalized to subscripts) |
| `emit_transition`, `anim_easing` / `anim_direction` / `anim_fill` / `anim_iterations`, `emit_animations`, `emit_hover_animations`, `keyframe_registration_code`, `keyframe_style_value` | Transition + `@keyframes` wiring |
| `emit_reactive_effects` | Reactive text/class/style/attr effects with state substitution that skips string literals (and never double-qualifies overlapping keys) |
| `translate_dynamic_expr`, `translate_js_value`, `js_split_top`, `js_object_members`, `js_array_elements`, `js_transform_leaf`, `js_is_whole_literal` | JS-expression shearing for dynamic positions |
| `fmt`, `color4`, `parse_color_val`, `keyword_literal`, `raw_prop_to_enum`, `effect_captures` | Formatting primitives and enum-literal tables (pinned by tests mirroring the C++ parse tables) |

State substitution deserves emphasis: reactive expressions rewrite names through the `state_map`, but string *literals* inside those expressions are skipped — your `"count"` string stays a string while your `count` variable becomes `__st_count.get()`. The tests (`state_substitution_skips_string_literals`, `overlapping_keys_never_double_qualify`) guard exactly this.

## The electrician (`logic_emitter.rs`): `emit_logic`

`emit_logic(windows)` → `LogicOutput` wires every reactive behavior. The sections, in emission order:

| Function | Emits |
|---|---|
| `emit_signal_statics` | `__st_*` signal statics (with `clean_init` / `infer_cpp_type` / `array_init_to_cpp` shaping inits) |
| `emit_shared_entries` (+ `shared_static`, `shared_ns`, `shared_ref`, `shared_backing_ref`) | Shared-store statics + accessors inside per-module namespace blocks |
| `emit_factories` (+ `emit_list_factory`, `collect_list_nodes`) | List item factories for keyed reconciliation |
| `emit_node_effects` (+ `emit_text_effect`, `emit_conditional_effect`, `emit_list_wiring`, `emit_style_effects`, `emit_font_size_effects`, `emit_attr_effects`, `emit_class_effect`, `emit_conditional_class_effects`) | One effect per reactive facet, with deps from `effect_dep_exprs` |
| `emit_node_events` (+ `translate_handler`, `translate_cond`, `translate_expr`, `event_member`) | Event wiring through the channel registry (comparison helpers hoisted out of handler lambdas) |
| `emit_rewire` | `morph_logic_rewire`: clears channels, re-registers subscriptions (the hot-reload path — see [Dev Mode](../architecture/dev-mode.md)) |
| `emit_init` | Startup wiring |
| `emit_includes` (+ `dev_features`) | Include set: dev TU gets everything, build TU gets the feature subset |
| `collect_premain`, `emit_native_block`, `strip_static_function` | User functions/classes hoisted before `main`, native blocks spliced in, dev TUs get `MID` dispatch defs |
| `generate_state_header` | `_morph_state.h`: signal declarations + per-module wrappers (skipping shared wrappers owned by `morph_api.h`, instance signals, and lambdas/`main` during function extraction) |

Ambient context flows through `AmbientMaps` (`ambient_maps`): the aggregated state/event/module maps every `translate_*` reads. `has_input` detects input nodes for special handling; `split_top_level` / `has_word` are the textual scalpels.

## The budget officer (`feature_set.rs`): `FeatureSet`

`scan(windows)` walks every node style and records which `MORPH_FEATURE_*` the app earns: `radius` (any rounding), `bold`, `scroll` (overflow or custom scrollbar fields), `position` (non-static or offsets), `zindex`, `opacity`, `display_none`, `inline`, `margin_collapse` (any nonzero margin), `min_max`, `border_box`, `flex` (flex display *or* any flex property deviating from default — `justify_content`, `gap`, `grow`, …), `cursor`, `border` (width, style, *or* a lone border-color, e.g. hover-only), `transform`. `scan_reactive` extends the set from `reactive_style` keys (`reactive_feature`: `z-index` → zindex, `display` → flex+display_none+inline, …), because a feature used only at runtime still needs compiling in. `required_headers()` / `required_defines()` convert the set into the compiler invocation.

The design bet: **style-gated compilation**. Your app pays for what it styles, to the exact define. Adding a runtime feature without a `feature_set` rule means production builds silently lack it while dev builds (all features on) work fine — the cruelest kind of "works on my machine". Always add both halves.

## The intern (`rust/mod.rs`): experimental Rust emission

Twenty lines. It exists, it is not production, and this paragraph is its entire documentation until someone adopts it. (The `versions/runtime/rust.json` placeholder tells the same story from the release side.)

## The skeleton (`templates/app_main.cpp.tera`)

The Tera template every app hangs on: window setup, node construction slots, logic slots, shared declarations wrapped in their conditional namespace blocks, keyframe registration, self-test hook. Emitters fill slots; nobody hand-edits generated output. If generated `app.cpp` looks structurally wrong (right nodes, wrong scaffolding), the template — not an emitter — is the suspect.

## The dev/build TU split, once more with feeling

Both TUs emit the same function-local statics so hot-reload and AOT agree on identity — but they differ deliberately: dev keeps the string channel registry (rewire needs names), build lowers every channel to a static accessor; dev includes everything, build includes the feature subset; dev emits `MID` dispatch defs, build bakes them. When dev and build disagree, diff the TU-specific paths first (`generate_morph_api_header_dev`, `dev_features`, `emit_rewire`): the shared emitters are usually innocent.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Change node construction output | `node_emitter.rs` `emit_node` + `set_style` |
| Change reactive behavior wiring | `logic_emitter.rs` `emit_node_effects` family |
| Change state/signal declarations | `generate_state_header` / `emit_signal_statics` (+ `cpp/mod.rs` `state_decls`) |
| Change the native contract | `generate_morph_api_header` (+ `_dev` variant) — this header is sacred, see the interop guides |
| Add a compilable feature | Runtime code + `feature_set.rs` rule + `scan_reactive` entry + fixture exercising it statically *and* reactively |
| Fix dev/build disagreement | TU-split functions first, shared emitters second |

## Verify by

```bash
cargo test -p morph-codegen
cargo test --workspace
rm -f .morph/output/<name>* && <repo>/target/debug/morph build --no-upx
<binary> --morph-self-test && ./tests/runtime/run-selftests.sh
```

Codegen changes are guilty until a fixture proves them innocent: rebuild the affected fixtures, run the self-tests, and screenshot anything visual. The C++ compiler is the final linter — if it compiles, links, and reports `0 failures`, the words became flesh successfully.
