# `morph-ir`: The Crate That Thinks

**Part of:** [Dev Docs](../architecture/overview.md)

If `morph-parser` ([crate-parser](crate-parser.md)) is the stenographer, `morph-ir` (`crates/morph-ir/src/`) is the analyst who reads the transcript and writes the battle plan. `IRBuilder` consumes every `MxSource` in the module graph plus the parsed CSS and produces `IRWindow` / `IRNode` trees: structure, resolved styles, reactive wiring, animations — everything codegen needs, nothing it doesn't. This crate is also where your friendly names die and qualified C++ expressions are born (`count` → `app::<namespace>::count().get()`), where Tailwind classes become numbers, and where event subscriptions get rewritten before anyone notices.

At 6,300+ lines, `builder.rs` is the largest single file in the Rust workspace. Do not read it top to bottom. Navigate by the map below.

## The data (`node.rs`, `style.rs`): the plan's vocabulary

**`IRNode`** is one UI node, fully described: identity (`node_id`, `node_type`), three style buckets (`style`, `hover_style`, `active_style`), children, events (`IREvent`: trigger/action/target), text (`text_content` plus `reactive_text` for state-bound strings), attributes (`attrs` plus `reactive_attrs` / `reactive_class` / `reactive_style` for the dynamic halves), conditional branches (`condition_expr` + `then_nodes` / `else_nodes`), list rendering (`list_expr`, `list_key_expr`, `item_template`, `list_item_param`, `list_index_param`), animations (`animations`, `hover_animations`), geometry (`x/y/w/h`), and transitions (`transition_duration`, `transition_easing`).

**`IRWindow`** is the whole app: window chrome (id, title, size, min/max, `visible`, `modal`, `renderer`), the node forest, `startup_logs`, `premain_functions` / `extra_headers` (user code that rides along), and the binding contracts codegen trusts blindly:

| Field | Contract |
|---|---|
| `state_vars` | Component state slots |
| `shared_vars` | The *complete* list of `morphShared` signals to declare — missing here means no C++ emitted, no apology |
| `event_decls` | One entry per visible event binding: identity `key`, namespace `ns`, `evt_<name>` accessor (build TU emits one static `Channel` each; dev TU keeps the string registry) |
| `channel_subs` | Subscriptions as `{channel, body}` pairs — body is a `[](const JsValue&)` lambda *only* (the emitter adds the single `.on()` wrap) |
| `mid_assignments` | `mid` native-index assignments per tagged instance (`MID_*` constants) |
| `module_bindings` | Universal bindings (functions/vars/classes + re-export aliases) with defining namespaces |
| `cpp_imports`, `keyframes` | Native edges and animation data |

**`IRStyle`** is the computed-style value object: colors as `[f32; 4]`, box fields (`margin`/`padding` as 4-arrays, `margin_auto` as 4-bools), sizing (`width/height/min/max` as `Option<f32>`), flexbox (`flex_dir`, `grow/shrink/basis`, `gap`, `wrap`, `justify_content`, `align_items`), positioning, borders, `z_index`, `opacity`, scrollbar fields, and transform state (`transform_ops`, composed `transform_matrix`, `transform_origin`). `IRStyle::new()` sets web-sane defaults (`display: block`, `font_size: 16.0`, `opacity: 1.0`, …); `is_empty_style()` detects untouched pseudo buckets via exact-equality field comparison (float `==` is intentional here — change detection, not math).

## The registries (`css_registry.rs`, `tailwind.rs`)

**`css_registry.rs`** is the single source of truth for supported CSS: ~86 `KNOWN_PROPERTIES`, the `CSS_TO_IR` name map (`background-color` → `bg_color`, `flex-direction` → `flex_dir`, …), and `property_feature` mapping properties to `MORPH_FEATURE_*` flags (flex, radius, opacity, transform, animation, scroll, positioning). Unknown properties are rejected here, not deep in layout code.

**`tailwind.rs`** (`TailwindResolver::resolve` / `resolve_many`) lowers 500+ utility classes to style values with no Node.js involved: a `static_map` for the plain cases, `resolve_negative` for `-mt-4`-style negation (numeric negation, not string fiddling), `parse_arbitrary` for `w-[37px]`-style escapes (every prefix covered, pinned by tests), and `transform_fn` + `negate_value` for transform utilities. Test names tell you the guarantees: `static_entries_cover_all_tiers`, `negative_utilities_negate_numerically`, `arbitrary_values_cover_every_prefix`, `unknown_and_malformed_classes_resolve_empty`. If a Tailwind class silently does nothing, this file (or the class spelling) is the suspect — never the renderer.

## The math (`transforms.rs`)

`parse_transform` turns a CSS transform list into `Vec<TransformOp>` (angles accepted in `deg`/`rad`/`grad`/`turn`, lengths with `%`-of-own-box support), and `compose_transform` folds the ops into a 4×4 matrix (`[f32; 16]`) with per-op composers (`rotate_x/y/z`, `rotate_axis`, `skew_x/y`, `perspective`, generic `multiply`) plus `parse_transform_origin` (keywords and percentages). The test suite checks parsing edge cases, rotation against real math, chain *order* (transforms compose left-to-right — get this backwards and everything shears), and parity with the old Python output (`compose_matches_python_output`). New transform functions land here as a `build_op` case plus a composer plus a test.

## The builder (`builder.rs`): `IRBuilder` in stages

`IRBuilder::new(graph)` + `build()` / `build_with_graph()` drive the whole show. The stages, in order:

1. **Namespace validation** (`validate_namespaces`, `ns_of`): every module's C++ namespace must be unique and well-formed before anything else. Fail fast, fail clearly.
2. **Sibling preseeding** (`preseed_sibling_names`): names visible across the module before per-module work begins.
3. **Per-module, per-instance expansion** (`expand_component_sources`, `expand_instance`): components instantiate once per use site, inside an `InstanceFrame` (`root` for the entry frame, `instance` for nested). Props bind here (`bind_props`), call-props adapt (`translate_call_prop`, `synthesize_adapter`), `mid` constants assign (`validate_mid`, `mid_const_name`, `mid_assignments`).
4. **State/event seeding** (`seed_module_bindings`, `seed_state_var`, `register_binding` / `register_module_binding` / `register_import` / `register_alias`, `register_event`): the identity machinery from [State & Event Internals](../state/state-events-internals.md) — canonical `module::binding` keys, per-module namespaces, `frame.vars` / `frame.events` ambient maps, deduped `shared_vars` entries. Import resolution threads through re-exports (`seed_named_import`, `seed_default_import`, `resolve_through_reexports`, `seed_re_exports`, `resolve_module_path`).
5. **Event rewrite** (`translate_event_sub`, `rewrite_event_emits` + `rewrite_emit_for_js` / `rewrite_emit_for_cpp`, `rewrite_emit_calls`, `rewrite_channel_access`): the order-sensitive textual rewrites — emit → `morphEmit` placeholder, `.on(handler)` → lambda-only body with fresh `__ch_N` params.
6. **Node building** (`build_node` / `build_node_in`): JSX → `IRNode`, with selector matching (`match_selector_compound`, `match_sequence`, `match_selector_detailed`, `selector_specificity`, `split_selector_sequence`, `split_trailing_pseudo`), property application (`apply_css_prop`, `parse_flex_shorthand`), UA defaults (`ua_defaults`, `ua_hover_defaults`, `ua_active_defaults`, `apply_ua_defaults`), dynamic classes (`analyze_dynamic_class`, `string_branch`, `resolve_branch_classes`), keyframes (`convert_keyframes`), and helpers for member validation (`validate_member_names`), event triggers (`event_trigger`), and type plumbing (`ts_type_class`, `parse_fn_type`, `parse_callable_source`, `prop_zero_value`, `infer_state_type`, `strip_static_linkage`).
7. **Logic translation** (`translate_logic`, `translate_module_globals`, `push_snippet`): component logic lowered with the frame's maps, ready for codegen.

String surgery at this scale needs safe substitution primitives, and the builder has them: `subst_props_refs`, `rename_symbols` (identifier-aware via `is_ident_start` / `is_ident_char_at`), `capture_raw`, `prepare_source`, `copy_template`, plus call-arg splitting that respects nesting (`split_call_args`, `find_emit_callee`, `split_top_level_commas`, `split_paren_body`, `find_top_level_arrow`). If you add a rewrite, use these — hand-rolled substring replacement in this file is how double-`.on()` bugs are born.

## The serializer (`serializer.rs`): one shape, two customers

`to_dict` / `to_json` convert windows (and an optional `logic_so_path`) into the JSON the dev runtime consumes: `window`, `node`, `style`, `transform_op`, `animations`, `keyframes_dict`, `keyframe` (with `declared` fields and `raw` values kept separate). Non-finite floats become `null`; unset optionals serialize as `null`; the shape is pinned by tests (`node_shape_matches_dev_protocol`, `to_json_round_trips`, `keyframes_keep_only_declared_fields`). The build pipeline and the hot-reload pipe ([Dev Mode](../architecture/dev-mode.md)) consume the *same* bytes — one serialization, two destinations, zero drift.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Add an IR field | `node.rs` / `style.rs` + `serializer.rs` shape + both emitters' handling + shape tests |
| Support a CSS property end-to-end | `css_registry.rs` entry → `apply_css_prop` case → `IRStyle` field → serializer → `node_emitter::set_style` → `property_feature` flag |
| Add a Tailwind utility | `tailwind.rs` (`static_map` / negative / arbitrary) + tier-coverage test |
| Add a transform function | `transforms.rs` (`build_op` + composer + order test) |
| Change state/event identity | `binding_identity`, `module_namespace`, `event_channel_id` — and the codegen side that must agree |
| Change emit/on lowering | `rewrite_event_emits` / `translate_event_sub` — keep the lambda-only contract |
| Change instantiation or prop binding | `expand_instance` / `bind_props` — re-run *all* runtime fixtures |

## Verify by

```bash
cargo test -p morph-ir
cargo test -p morph-parser -p morph-ir -p morph-codegen
cargo test --workspace
```

The IR-shape tests are the review: any changed `app::…` accessor string means a relink of every store/event. Confirm the diff is the one you intended, then run `morph check` + `morph build` on `tests/runtime/component-test` and `examples/components`.
