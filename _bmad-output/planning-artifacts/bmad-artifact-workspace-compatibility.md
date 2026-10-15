---
id: bmad-artifact-workspace-compatibility
artifact_type: plan
status: draft
classification: repository-visible
owner: repository-don-001
scope: repository/bmad-governance/artifact-workspaces
created: 2026-09-04
---

# BMAD artifact-workspace compatibility migration

## Decision state

The repository Don authorized preparation of this prerequisite migration and
confirmed `repository-visible` classification on 2026-09-04. This draft does
not authorize implementation. Implementation requires explicit approval of the
completed plan and approval evidence recorded through the artifact index.

## Why

The installed BMAD workflows and the repository artifact verifier currently
disagree about durable output shape. The verifier accepts recursively nested
Markdown only when every file is an individually indexed artifact whose
filename stem equals its globally unique lowercase ID. Standard workflows use
run folders with repeated primary names and contract or operational companions,
including `prd.md`, `ARCHITECTURE-SPINE.md`, `.memlog.md`, review files, and
validation reports. Epics and sprint planning also produce mutable singleton
files, `epics.md` and `sprint-status.yaml`. Those outputs cannot satisfy the
current per-file identity rule, so the required full-BMM planning chain cannot
complete while governance verification remains enabled.

## Outcome

Migrate artifact governance from schema version 1 to version 2 so installed
BMAD workflows can create governed run workspaces and explicit operational
state without weakening authority, lifecycle, classification, path,
sensitive-content, protected-study, or public-documentation controls. After
this migration is separately approved, implemented, and verified, create the
Rust dependency-remediation initiative through the required PRD, architecture,
epic/story, and sprint-readiness sequence.

## Version-2 data model

1. Preserve the current artifact-record schema and flat layout. An artifact not
   referenced by a workspace remains a flat artifact and keeps its exact
   repository-relative path, `.md` suffix, and filename-stem-equals-ID rule.
2. Add exact top-level `workspaces`, `workspace_rules`, `operational_files`,
   `operational_file_rules`, `approval_commitments`,
   `approval_commitment_rules`, `story_pins`, `story_pin_rules`,
   `context_reviews`, `context_review_rules`, `draft_context_bindings`,
   `draft_context_binding_rules`, `transition_evidence_rules`, and
   `delegated_write_rules` fields. Do not add workspace-only, operational,
   pin-only, context-review-only, draft-binding-only, transition-only, or
   delegation-only fields to every artifact record.
3. Each workspace has exactly `id`, `artifact_id`, `workflow`, `path`,
   `lifecycle_files`, `lifecycle_dependencies`, `context_sources`,
   `support_files`, and `lifecycle_digest`. Its lowercase hyphenated ID equals
   its directory basename; its path is below one canonical root; its workflow is
   an installed canonical skill ID; and it references exactly one artifact
   record.
4. An artifact referenced by exactly one workspace is *workspace-backed*. Its
   artifact `path` equals the repository path formed by joining the workspace
   path to its sole `primary` lifecycle-file path. That primary may therefore
   use a workflow-owned fixed name such as `prd.md`, `SPEC.md`, or
   `ARCHITECTURE-SPINE.md` and is exempt from the flat filename-stem rule. The
   artifact-to-primary link is one ownership claim, not a duplicate claim.
   Unknown, unbound, or multiply bound artifacts and workspaces fail.
5. Each operational-file record has exactly `id`, `governing_artifact_id`,
   `workflow`, `path`, `role`, `validator`, `binding_revision`,
   `binding_history`, `content_sha256`, `source_artifacts`, and
   `source_context_review`. It reserves one exact path
   below a canonical root for mutable workflow state, inherits owner, scope, and
   classification from its governing artifact, and has no approval lifecycle
   or effectiveness semantics of its own. IDs are lowercase hyphenated and
   unique. Its governing artifact must be the single effective approved record
   for the operation's scope. Version 2 initially permits only these complete
   owner/role/validator/path contracts: `bmad-create-epics-and-stories` /
   `working-document` /
   `epics-working-document-v1` /
   `_bmad-output/planning-artifacts/epics.md`, and `bmad-sprint-planning` /
   `tracking` / `bmad-sprint-status-v1` /
   `_bmad-output/implementation-artifacts/sprint-status.yaml`, plus
   `bmad-build` / `derived-context-cache` / `bmad-epic-context-v1` at an
   exact per-record path matching
   `_bmad-output/implementation-artifacts/epic-[1-9][0-9]*-context.md`. The first
   validator requires UTF-8 Markdown with mapping frontmatter containing
   `stepsCompleted` and `inputDocuments` arrays. The second is a
   verifier-native, Python-standard-library parser for the restricted YAML shape
   emitted by the locked sprint generator and is semantically equivalent to a
   `sprint_plan.py validate` result of `valid: true` with an empty `problems`
   array. The generated PEP 723 script is a parity-test oracle only; the public
   verifier never imports it, invokes `uv`, downloads dependencies, or requires
   `ruamel.yaml`. The third requires UTF-8 Markdown whose first line is exactly
   `# Epic N Context:` plus a non-empty title and the same positive integer N
   as its filename. Operational rules allow only the owner workflow to write,
   except that `bmad-build` and `bmad-retrospective` are explicit writers of
   sprint status.
   Validators are symbolic verifier contracts, never executable paths supplied
   by index data. A reserved target may be absent before or after a run.
   `content_sha256` is null when absent and otherwise equals the lowercase
   exact-byte digest. `source_artifacts` is a sorted list of exact
   `{artifact_id,approval_snapshot_sha256}` commitment references;
   `source_context_review` is null or exact `{id,digest}`. At most 128 source
   artifacts may be named; an oversized dependency closure halts before source
   bundling or writes. When an epics working document exists its sources are all
   validated `inputDocuments`, including PRD and architecture, and its context
   review is non-null. When sprint status exists its source list is exactly the
   governing epics-handoff commitment and its context review is non-null. When
   an epic-context cache exists its exact closed source set is defined in Build
   routing below and its context review is non-null. For an absent target the
   same source fields describe the reserved binding and may be empty/null only
   before its first successful owner write. If present, the target
   must be regular, non-symlink, exactly claimed, safety-scanned, accepted by its
   named validator, and consistent with those source bindings. Every other
   tuple or unregistered operational file fails; adding one requires reviewed
   policy migration rather than a skill-name bypass.
6. An operational binding starts at integer revision 1. `binding_history` is a
   non-empty append-only list whose entries have exactly `revision`,
   `governing_artifact_id`, `bound_at`, `authorizer`, `evidence`,
   `evidence_sha256`, `previous_entry_sha256`, and `entry_sha256`;
   revisions are consecutive, timestamps are UTC RFC 3339 and increasing,
   authorizers are active and scope-eligible, evidence is an existing
   repository-relative file, and the current governing ID and revision equal the
   final entry. For an initial binding the authorizer is eligible for the new
   governing scope. For every rebind the one authorizer must be eligible for both
   the prior and new governing scopes, and the evidence must be the canonical
   binding receipt below, identify and hash the disposed prior target, and
   authorize the new binding; otherwise a universally scoped Don is required.
   The first previous-entry digest is null;
   each later one equals
   the prior entry digest. Evidence hashes exact bytes, and each entry digest
   hashes the other seven entry fields (excluding `entry_sha256`) as
   recursively key-sorted compact UTF-8 JSON plus LF using the lifecycle
   serializer. Rebinding any reusable operational path, including each exact
   `epic-N-context.md` cache path, is allowed only through the journaled
   transaction after candidate verification. If the target is absent, evidence
   records that preimage; if present, the transaction hashes and preserves its
   exact old bytes for rollback and the dual-scope authorization evidence names
   that digest before replacement.
   Rebinding appends history and never overwrites it; deleting a live target,
   silently changing a governing ID, breaking the hash chain, or rebinding to a
   draft, superseded, or archived artifact fails. Each operational record has at
   most 64 binding-history entries; reaching the cap requires a separately
   approved migration rather than truncation or rollover.
7. Each `story_pins` entry has exactly `spec_artifact_id`, `spec_lineage_id`,
   `story_id`, `governance_story_key`, `story_artifact_id`, `dispatch_history`,
   `pinned_at`, `authorizer`, `evidence`, and `evidence_sha256`. `story_id` preserves the exact non-empty valid ASCII
   letters/digits/dashes string from `stories.yaml` and is at most 256 ASCII
   bytes. A longer ID that the installed workflow could otherwise parse is not
   repository-governable and must halt before any artifact or code write.
   `spec_lineage_id` is the unique oldest artifact in the source Spec's
   complete supersession lineage. `governance_story_key` is exactly `sid-` plus the
   lowercase, unpadded 52-character RFC 4648 base32 encoding of
   `SHA256(UTF8(spec_lineage_id) || 0x00 || UTF8(story_id))`, matching
   `^sid-[a-z2-7]{52}$`; a digest collision between unequal lineage/ID tuples
   fails closed. The pair `(spec_lineage_id, story_id)`, the pair
   `(spec_lineage_id, governance_story_key)`, and every `story_artifact_id` are unique.
   `spec_artifact_id` remains the exact workspace artifact whose dispatch entry
   created the pin, while the derived lineage identity makes the key stable
   across its successors and distinct for the same raw ID in another Spec.
   `pinned_at` is UTC RFC 3339;
   `authorizer` is an active authority eligible for both the source spec and new
   story scopes; and the exact evidence path and digest use the closed story-pin
   receipt defined below. `spec_artifact_id` identifies a
   workspace-backed `bmad-spec` artifact whose lifecycle inventory contains
   `stories.yaml` with role `dispatch`; `story_artifact_id` identifies the first
   external flat Build story record created from that exact dispatch entry. The
   repository transaction installs the pin with that first story file and draft
   record, never before or after it. `dispatch_history` is a non-empty,
   append-only list of at most 64 entries, each having exactly
   `spec_artifact_id`, `dispatch_digest`, and `approval_snapshot_sha256`. The
   digest is lowercase SHA-256 of the exact materialized six-field
   `bmad-stories-v1` record serialized alone with the lifecycle JSON serializer;
   the approval digest identifies that Spec revision's immutable commitment.
   The first entry names the pin's source Spec. Every later approved Spec
   successor that retains the raw ID atomically appends one entry naming itself,
   whether or not the dispatch digest changed; entries must follow the unique
   Spec supersession lineage without gaps or forks. Reordering a stories array
   with byte-equivalent materialized record preserves the dispatch digest.
   Reaching 64 versions halts for a separately approved migration. Pin base
   fields are immutable and its dispatch-history prefix is immutable. Pins survive all artifact
   lifecycle transitions, and apply from their source through both the spec
   supersession lineage and the Build-story successor lineage. Story Breakdown
   treats each applicable external pin exactly as the installed
   `stories/<id>-*.md` signal: a pinned story keeps its ID; omission from an
   approved spec successor retires the ID; and a retired ID can never reappear,
   be rebound, or name another story. A staged update must present each prior
   pinned entry and proposed keep-or-retire result for explicit human
   confirmation and record that decision in the successor memlog. It must not
   create a compatibility or shadow file below `stories/`. A pinned dispatch is
   *active* only while its exact raw ID and materialized record remain present
   in the unique effective approved Spec head for that lineage and that record's
   digest plus Spec approval commitment equal the final dispatch-history entry.
   A retained ID whose final `dispatch_digest` differs from the digest bound by
   the current Build head is active but changed, not retired; it requires the
   dispatch-reconciliation Build successor and fresh approval defined below.
   A later Spec history entry with the same dispatch digest remains compatible
   without mutating the approved Build metadata.
   Only omission retires the ID. Initial Build
   dispatch, every resume or re-entry, Checkpoint 1, and the immediate pre-Step
   3 gate must revalidate that active record and its pin. Retirement before
   accepted Build completion cancels further execution: the installed revert
   path runs when applicable and the workflow halts without changing the story
   lifecycle; a new Spec story ID, pin, Build artifact, and approval are
   required. A Build story already accepted as complete remains retained
   history after retirement but cannot be resumed or used as a new dispatch.
8. Each `context_reviews` entry has exactly `id`, `path`, `digest`,
   `source_identities`, `reviewed_by`, `reviewed_at`, `evidence`, and
   `evidence_sha256`. Its ID is a
   unique lowercase-hyphenated `project-context-rN`; path is exactly
   `_bmad-output/project-context.md`; digest is the lowercase SHA-256 of that
   file's exact reviewed bytes. `source_identities` is a recursively
   key-sorted mapping whose key set equals the exact machine-policy
   authoritative-source path list and whose values are the lowercase exact-byte
   SHA-256 identities recorded for those paths in the reviewed file. Creation
   validates that key set against the then-current policy, and the latest review
   validates it against current policy; a retained historical review keeps its
   exact receipt-protected mapping rather than being reinterpreted after a later
   approved policy migration. Reviewer and UTC timestamp equal the file's
   review frontmatter and name an active verified human eligible for repository
   scope; and the evidence path and digest use the closed context-review receipt
   below. Entries and their receipts are append-only. The unique latest entry by
   increasing `rN` and `reviewed_at` must match the current project-context
   bytes, metadata, and source-identity mapping. Prior entries remain so
   historical snapshots can resolve the exact review identity, recorded digest,
   authoritative-source allowlist and digests, and immutable receipt without
   pretending the old source bytes still occupy the current path. A same-metadata body change, missing
   receipt, digest drift, reordered/reused revision, or history truncation
   fails.
9. Each `draft_context_bindings` entry has exactly `artifact_id`,
   `context_review_id`, and `context_digest`. It is the sole durable context
   association for one flat draft created under version 2; workspace drafts use
   their `context_sources` field instead. Artifact IDs are unique in this list,
   reference an existing flat artifact whose status is `draft`, and the review
   ID/digest resolve one retained `context_reviews` entry. A newly staged flat
   draft binds the unique latest review before its first write. A stale retained
   binding may remain visible, but no writer may resume, approve, or consume the
   draft until its closed reconciliation route reconciles it against the latest
   context in staging and atomically replaces the binding. Approval requires the
   binding to identify the unique latest review with its exact digest and atomically removes it while
   installing the identical source identity in the approval commitment.
   Merely changing the binding without presenting the context change and
   reconciled artifact for human confirmation is forbidden. The activation
   migration enumerates every existing version-1 flat draft except its own
   then-draft schema-migration ID as an inert legacy draft with no binding; one cannot enter a writer until the same reconciliation
   transaction installs its first current binding. To distinguish that closed
   cohort from an invalid unbound version-2 draft, `draft_context_binding_rules`
   contains `legacy_unbound_drafts`, an artifact-ID-sorted list whose entries have
   exactly `artifact_id`, `artifact_path`, `artifact_sha256`, and
   `artifact_record_sha256`, `reconciled_at`,
   `reconciliation_binding_sha256`, `reconciliation_evidence`, and
   `reconciliation_evidence_sha256`. Each entry names one flat draft present at the
   version-1 cutoff, records its exact path, hashes its exact file bytes, and
   hashes its complete version-1 artifact record with the lifecycle JSON
   serializer; its last four fields are null at activation. The migration
   artifact embeds the same list. Activation requires exact equality among that
   list, the cutoff index's unbound flat drafts after excluding the migration's
   own then-draft ID, and the corresponding live draft bytes and records; any extra, missing, changed,
   workspace-backed, or non-draft entry aborts activation. Under version 2 only
   an entry whose four terminal fields remain null and whose exact retained
   bytes and record still match may authorize an unbound inert draft; any file,
   path, metadata, or lifecycle change first requires the owning reconciliation transaction to install a
   current binding. The list order and first four fields are immutable history
   and cannot be extended after activation; a newly created flat draft always
   requires a binding. The first reconciliation atomically changes all four
   terminal fields from null exactly once: time is UTC RFC 3339,
   `reconciliation_binding_sha256` hashes the exact three-field binding being
   installed, and the evidence path/digest resolve the canonical legacy-draft
   receipt below. Once non-null they are immutable and the history gate rejects
   rollback. After one listed artifact is reconciled, its entry remains only as history: the live
   artifact follows normal binding/commitment rules and is no longer compared
   to the old file/record digests, while that entry can never authorize an
   unbound state again.
   Reconciliation ownership is
   exact: `epics-{key}[-rN]` belongs to
   `bmad-create-epics-and-stories`, `readiness-{key}[-rN]` to
   `bmad-sprint-planning`, every `story` to `bmad-build`, and every
   `retrospective` to `bmad-retrospective`. A human-authored flat `plan` not
   matching those workflow forms, `urgent-security-incident`,
   `trivial-exemption`, or `schema-migration` uses a repository-owned human
   reconciliation transaction that accepts only a reviewed full candidate,
   preserves every type-specific immutable fact (including an incident's
   validated deadline), displays old/new context and bytes, and performs the
   same reservation, CAS, and rollback protocol without invoking an unsupported
   BMAD writer. Every other legacy flat-draft type is inert until a separately
   approved type-specific route exists. No approved, superseded,
   archived, workspace-backed, missing, or multiply bound artifact may appear.
   `draft_context_binding_rules` contains exactly these cardinality, lifecycle,
   reconciliation, and approval-transfer rules.

## Workspace integrity and lifecycle

1. Split every workspace's complete exact inventory into `lifecycle_files`
   entries having exactly `path` and `role`, and `support_files` entries
   having exactly `path`, `role`, and `digest`. Lifecycle roles are `primary`,
   `companion`, `memory`, and `dispatch`; support roles are `review`,
   `reconciliation`, `validation-report`, `working`, `import`, `mockup`,
   and `wireframe`. Require exactly one primary. Standard formats are allowed
   only when individually listed; no glob grants file ownership or governance.
   A Spec lifecycle entry named `stories.yaml` with role `dispatch` must satisfy
   the repository-native `bmad-stories-v1` validator: exact output of the
   lifecycle JSON serializer (therefore also valid YAML 1.2), top-level array,
   and every record materializes exactly `id`, `title`, `description`,
   `spec_checkpoint`, `done_checkpoint`, and `invoke_dev_with`; omitted installed
   optionals become respectively `false`, `false`, and `""` before
   serialization. It enforces the installed types, quoted-string semantics,
   unique and prefix-free
   IDs, the 256-byte repository limit, and no `status`. The Spec team instruction
   requires Story Breakdown to emit that form directly; a repository helper
   validates but never guesses how to convert general YAML or folded scalars.
   Noncanonical output halts in staging before approval. The stdlib verifier
   parses every dispatch even when no pin exists, and golden/parity tests cover
   key ordering, escaping, LF termination, every
   installed field, optional/default value, ID retirement, and rejection of
   general-YAML-only syntax.
2. Each `lifecycle_dependencies` entry has exactly `artifact_id` and `digest`.
   It names a separately indexed artifact whose current bytes are an adopted,
   load-bearing input. The dependency scope may equal the dependent artifact's
   scope or be its slash-delimited ancestor. The only cross-sibling exception is
   between descendants of the same unique effective approved governing-root
   lineage: architecture or UX may depend on that lineage's PRD, and Spec may
   depend on that lineage's PRD, architecture, or UX. Both parent chains must
   reach the same root without cycles or ambiguity; same-looking scopes under a
   different root do not qualify. Adding another type relation requires a policy
   migration. A public dependency may be consumed
   by a public or repository-visible artifact; a repository-visible dependency
   may be consumed only by a repository-visible artifact. While the dependent
   artifact is draft, every dependency must be currently effective and approved
   and its recorded digest must match; no comparison is made to the draft's null
   `approved_at`. The dependent approval transaction then requires each
   dependency's `approved_at` to be no later than the proposed dependent
   `approved_at` and requires no dependency supersession or archive transition
   at or before that instant. Thereafter, any recorded dependency
   `superseded_at` or `archived_at` must be later than the dependent approval.
   A later transition does not invalidate historical use when the artifact stays
   indexed and its recorded digest still matches. The entry digest equals the
   dependency workspace's
   `lifecycle_digest`, or the exact-byte SHA-256 for a flat artifact. An adopted
   spec companion outside the spec workspace must resolve to the flat path or a
   lifecycle path of exactly one listed dependency. The reference and digest
   remain frozen if that dependency is later superseded or archived; missing,
   unindexed, mutable-draft, support-only, or digest-mismatched dependencies
   fail. An empty dependency list is valid.
3. Each `context_sources` entry has exactly `id`, `path`, `digest`,
   `reviewed_by`, and `reviewed_at`. Version 2 permits only the governed source
   ID `project-context` at the policy-fixed path
   `_bmad-output/project-context.md`, and every workspace contains exactly one
   such entry. It must remain a regular, non-symlink,
   repository-visible file whose reviewer is an active verified human and whose
   review time and internal source identities pass the existing project-context
   verifier. This exact five-field object is the canonical
   `project-context-source-v1` projection: `id` is `project-context`, `path` is
   the policy-fixed path, and `digest`, `reviewed_by`, and `reviewed_at` are
   projected from one exact retained `context_reviews` record. Exactly one
   retained review must match that digest/reviewer/time projection; its persisted
   `source_identities` supplies the historical authoritative-reference
   allowlist without requiring old project-context bytes. Its reviewer and
   time also equal the source frontmatter's `review.reviewer_identity` and
   `review.reviewed_at`. A newly staged workspace draft binds the unique latest
   review. If a later
   review changes the current source, the stale draft entry may remain bound to
   its retained review and receipt so global verification stays possible, but
   no workflow may resume, approve, or consume it until the owning workflow
   reconciles a staged copy and atomically replaces the entry with the latest
   review. Merely replacing the entry without presenting the context change and
   reconciled workspace for human confirmation is forbidden. The unique latest
   global review record must always match the current source. After approval,
   that workspace entry instead remains
   bound to its retained historical review and immutable receipt and is not
   compared to later current source bytes or frontmatter; superseded and archived
   workspaces follow the same historical rule. In the approval transaction that
   digest and its
   immutable review receipt must still match, and `reviewed_at` must be no later
   than the proposed artifact `approved_at`. Approval freezes that
   source identity in the approval commitment; later context review does not
   rewrite historical commitments. Historical verification validates only the
   retained review record, immutable receipt, and recorded digest; it does not
   claim that superseded source bytes can be reconstructed from the current
   path. Before any approved artifact is executed, its retained context digest
   must equal the unique latest current review digest. Any digest change,
   including one judged not to affect prior decisions, requires an explicit
   reconciliation recorded in a draft successor that binds the current review;
   only that successor may proceed to execution after normal approval. There is
   no no-op execution path against an older context binding. The successor
   route is closed: a workspace uses the fork transaction below; a Build story
   uses the context-reconciliation branch of Build routing item 13; and an
   epics, readiness, or retrospective flat artifact uses that workflow's next
   bounded `-rN` successor with the complete record mapping defined below. No
   other approved flat artifact has a generic writer or inferred successor
   route: after context drift it remains historical and cannot govern or enter a
   workflow until a separately approved type-specific route exists. A new
   initiative may instead use the root-PRD bootstrap. An urgent incident is an
   immutable temporary routing record rather than executable planning content;
   PRD bootstrap may retain it only as parent and preserve its fixed deadline,
   while the new PRD itself binds the latest reviewed context.
   When `SPEC.md` lists project context as an adopted companion, this exact
   governed-source entry owns the reference; no general outside-root companion
   exception exists.
4. Reject non-canonical paths, backslashes, dot segments, traversal, absolute
   paths, non-NFC names, symlinks or special files and directories, missing
   inventoried targets, duplicate claims, NFC-plus-casefold portable-path
   collisions, overlapping or nested workspace roots, and every file of every
   extension inside a workspace but absent from its exact inventory. Every file
   of every extension anywhere below a canonical root must be a flat artifact, a
   workspace file, a registered operational file, or exactly one of the existing
   governance surfaces `_bmad-output/planning-artifacts/README.md` and
   `_bmad-output/implementation-artifacts/README.md`. `workspace_rules` must
   contain only those two README exemptions; both remain tracked, regular,
   non-symlink, and subject to instruction, sensitive-content, study, and
   publication checks. Any other unindexed README fails. Every directory below
   a canonical root must be a registered workspace directory, a descendant of
   exactly one workspace needed by an inventoried target, or a necessary
   ancestor of a claimed flat/operational target. Empty, unclaimed, multiply
   owned, symlink, special, or deeper-than-policy directories fail. The verifier
   walks with bounded `os.scandir` accounting and rejects a count/depth overflow
   before descending further; it does not use an unbounded recursive glob.
5. A draft workspace may evolve only while its exact inventory stays
   synchronized. Approved, superseded, and archived workspace lifecycle bytes
   are immutable. For each support file in the complete transitive local-link
   or resource closure originating from any lifecycle file, `digest` is its
   current lowercase exact-byte SHA-256; every other support entry has null
   `digest`. The closure includes Markdown links and inert HTML/CSS/SVG
   references and may contain linked Excalidraw support. A non-null support
   digest is included in the lifecycle digest below and becomes immutable at
   approval even though the file retains its support role. A null-digest support
   file remains mutable, exactly inventoried, and safety-checked, but cannot be
   referenced directly or transitively by lifecycle content. Closure, digest,
   or nullability mismatch fails.
6. Compute `lifecycle_digest` as follows. For each lifecycle file, hash its exact
   bytes with SHA-256 and encode the digest as lowercase hexadecimal. Use the
   workspace-relative canonical POSIX path, which must already be Unicode NFC,
   and its declared role to form an entry
   `{"path":PATH,"role":ROLE,"sha256":HASH}`. Form an identical entry for
   every non-null-digest support file and call that list `linked_support`. Form
   each dependency entry as `{"artifact_id":ID,"digest":HASH}`. Sort files and
   linked support by the UTF-8 byte sequence of `PATH`, sort dependencies by the
   UTF-8 byte sequence of `ID`, and wrap them as
   `{"dependencies":[DEPENDENCY,...],"files":[ENTRY,...],"linked_support":[ENTRY,...]}`. Serialize with
   Python 3.10 `json.dumps(payload, ensure_ascii=False, sort_keys=True,
   separators=(",", ":"))`, append exactly one LF, encode as UTF-8, then hash
   those bytes with SHA-256 and encode lowercase hexadecimal. Solidus remains
   `/`. Backslash, double quote, C0 controls, and DEL are invalid in path
   segments before serialization, so they have no alternative digest spelling.
   Golden vectors must fix expected hashes for empty dependency/linked-support
   lists, a linked UX support closure, nested paths, and non-ASCII NFC paths,
   plus rejection vectors for forbidden characters.
   Draft workspaces require a null digest; approved, superseded, and
   archived workspaces require the computed digest. Lifecycle content, path,
   role, dependency, linked-support closure, or inventory drift invalidates the
   digest. Support path or inventory drift always fails exact-inventory
   validation; only null-digest, unreferenced support bytes may change without
   changing `lifecycle_digest`.
7. Preserve all existing authority, approval independence, chronology,
   parentage, supersession, archival, classification, and urgent-security rules
   for both flat and workspace-backed artifacts. A workspace inherits its
   artifact lifecycle. Its support files inherit scope and classification but
   cannot be approved or published independently through the workspace record.
8. Never update, reconcile changed context, or run a report-writing Validate
   operation against an approved workspace in place. An exact repository-owned
   fork transaction first chooses
   a new unique artifact ID and workspace directory as `fork-` plus the lowercase
   unpadded 52-character base32 of
   `SHA256(UTF8(predecessor_id) || 0x00 || UTF8(decimal fork_revision))`, where
   revision is the next positive integer capped at 9,999; a digest collision
   fails closed. It copies predecessor lifecycle files and all inventoried
   support files to a temporary directory outside the canonical roots. Every
   copied support file is revalidated under its content class and retains its
   support role, so UX mockup/wireframe continuity is preserved without making
   unreferenced support load-bearing; any copied support reached from lifecycle
   content remains in the recomputed `linked_support` digest closure. Before candidate
   validation, the owning workflow must remove or rewrite any intentionally
   dropped relative reference in staging, so no lifecycle file, CSS, HTML, SVG,
   or copied support file has a dangling or outside-workspace reference. The transaction preserves capability/decision
   IDs, appends a fork event to the copied memlog through the locked memlog
   helper, sets the new workspace digest to null, and prepares a new
   artifact/workspace record.
   The successor retains the predecessor's artifact type, title, owner, scope,
   and classification. A null root parent remains null; a non-null parent is
   retained only while effective, otherwise it is replaced by the one compatible
   effective approved successor in that exact parent lineage after explicit
   reconciliation. Missing, forked, unrelated, or scope-incompatible parent
   succession aborts. The successor uses that resolved parent, the new
   ID/workspace and primary path; has
   status `draft`; and sets every approval, transition, independence, and archive
   field to null. Before candidate validation it
   runs dependency reconciliation in that temporary copy: carry each dependency
   that is still effective and approved; for each superseded or archived
   dependency, present the historical binding and eligible effective successors,
   require explicit human selection, and use the owning workflow against the
   staged `doc_workspace` to update adopted-companion references and append the
   replacement decision to the copied memlog. No replacement is inferred. If
   there is no approved compatible replacement or the owning workflow cannot
   reconcile the staged contract, abort and discard the temporary copy. It then
   validates the complete candidate with only currently effective dependencies
   in a disposable checkout and installs the new directory and index with
   rollback on either failure; the predecessor stays byte-identical. The
   workflow continues the requested Update, context reconciliation, or Validate
   operation only against this draft successor; Validate report names are
   inventoried there and never added to the approved predecessor. A later explicit
   approval connects it to the predecessor through the existing
   `supersedes`/`superseded_by`, authorizer, evidence, and chronology rules.
   Team activation guards must block PRD, architecture, and spec operations from
   binding an approved path and require this fork before any workflow write.
9. Before approving, superseding, or archiving any artifact, scan every
   operational record governed by it and every active Build execution lease
   that names it as story, Spec dispatch source, parent, or handoff. A
   lease-conflicting lifecycle or pin-history transaction waits or halts without
   changing state. A transaction that would make that
   governing artifact non-effective must either append an authorized binding to
   the one selected effective approved successor in the same journaled
   transaction or refuse the lifecycle transition. This applies when the target
   exists and when it is absent; absence never permits a stale governing ID.
   Rebinding uses the canonical receipt, dual-scope authority, preimage, CAS,
   rollback, and history rules above. An archive with no eligible successor is
   blocked while any operational record remains bound. Tests cover successor
   approval plus predecessor supersession, standalone archive refusal, present
   and absent targets, and failure rollback.

## Approval and history commitments

1. Every version-2 approved artifact has exactly one `approval_commitments`
   entry with exactly `artifact_id`, `commitment_index_revision`,
   `context_sources`, `approval_snapshot_sha256`, and
   `approval_evidence_sha256`. Every approval
   first made under version 2 sets `commitment_index_revision` to the positive
   integer index revision that installs both approval and commitment, and uses
   `_bmad-output/governance-evidence/approval-{approval-key}.json`, where
   `approval-key` is the lowercase unpadded 52-character RFC 4648 base32 of
   `SHA256(UTF8(artifact_id) || 0x00 || UTF8(decimal commitment_index_revision))`;
   unequal tuples producing the same key fail closed.
   For a version-2 approval, `context_sources` contains exactly one canonical
   five-field `project-context-source-v1` projection of the latest eligible
   `context_reviews` entry used by the approval transaction. For a
   workspace-backed artifact it equals the workspace's one-entry list
   byte-for-byte after lifecycle serialization. For a flat artifact, the
   approval transaction resolves its three-field draft binding against the
   retained review, requires `context_review_id` to identify the record from
   which the projection is formed and `context_digest` to equal its projected
   digest, removes the binding, and persists the five-field
   projection in the commitment; the differently shaped objects are never
   compared for structural equality. The commitment becomes the durable
   association from which the historical snapshot is reconstructed. Bootstrap commitments for artifacts approved
   under version 1, including the migration artifact, use exactly the empty list
   to record that version 1 made no cryptographic context-binding claim; only
   the closed migration cohort may use that legacy sentinel.
   The approval receipt is recursively key-sorted compact UTF-8 JSON plus LF
   with exactly `artifact_id`, `commitment_index_revision`, `approved_at`,
   `approver`, `classification`, `decision` fixed to `approved`, and
   `approval_snapshot_sha256`, plus the four existing independence-exception
   fields. Those four fields are all null when no exception applies. When the
   Don exception applies, its evidence path equals this receipt path and the
   receipt states the scope-eligible Don, timestamp, and non-empty rationale.
   Its exact-byte SHA-256 equals
   `approval_evidence_sha256` and the artifact's `approval_evidence` equals its
   path. The human-reviewed evidence is the authority-model trust anchor, not a
   confidentiality boundary.
2. `_bmad-output/governance-evidence` is a closed, human-owned evidence surface,
   not a third artifact root. It is flat and permits only
   `approval-{52-lowercase-base32}.json`,
   `binding-{52-lowercase-base32}.json`,
   `supersession-{52-lowercase-base32}.json`,
   `archive-{52-lowercase-base32}.json`,
   `story-pin-{story-artifact-id}.json`,
   `context-review-{context-review-id}.json`, and
   `legacy-draft-{52-lowercase-base32}.json`. A story-pin receipt has exactly
   `spec_artifact_id`, `spec_lineage_id`, `story_id`, `governance_story_key`,
   `story_artifact_id`, `initial_dispatch`, `pinned_at`, `authorizer`, and
   `decision` fixed to `pin`, where `initial_dispatch` equals the first complete
   dispatch-history entry; a context-review
   receipt has exactly `id`, `path`, `digest`, `source_identities`, `reviewed_by`,
   `reviewed_at`, and `decision` fixed to `reviewed`; its source mapping equals
   the review record byte-for-byte after lifecycle serialization. A legacy-draft
   key is the lowercase unpadded base32 of `SHA256(UTF8(artifact_id))`, with
   unequal-ID collision rejected. Its receipt has exactly `artifact_id`,
   `artifact_path`, `artifact_sha256`, `artifact_record_sha256`, `binding`,
   `reconciled_at`, `authorizer`, and `decision` fixed to `bind-legacy-draft`;
   `binding` is the exact three-field draft binding, the authorizer is active and
   scope-eligible, and its path/digest equal the list entry's evidence fields. A binding key is the lowercase unpadded base32
   of `SHA256(UTF8(operational_id) || 0x00 || UTF8(decimal revision))`. Its
   receipt has exactly `operational_id`, `revision`,
   `prior_governing_artifact_id`, `new_governing_artifact_id`,
   `prior_target_state`, `prior_target_sha256`, `bound_at`, `authorizer`, and
   `decision`. Revision 1 uses a null prior ID,
   `prior_target_state` fixed to
   `absent`, a null prior digest, and decision `bind`; a later revision uses the
   prior ID, state `absent` or `present`, a null or lowercase SHA-256 digest
   respectively, and decision `rebind`. Its path and digest equal the matching
   binding-history evidence fields. A transition key is the lowercase unpadded
   base32 of
   `SHA256(UTF8(artifact_id) || 0x00 || UTF8(event))`, where event is exactly
   `supersession` or `archive`. A supersession receipt has exactly `artifact_id`,
   `successor_artifact_id`, `authorized_at`, `authorizer`, and decision
   `supersede`; an archive receipt has exactly `artifact_id`, `archived_at`,
   `authorizer`, and decision `archive`. The artifact's corresponding evidence
   path for a transition first made under version 2 is the canonical keyed path
   and its lifecycle tuple must exactly equal the receipt. A transition already
   present in the closed version-1 cohort retains its enumerated legacy evidence
   instead and may never be rewritten. Pin evidence must equal
   `_bmad-output/governance-evidence/story-pin-{story-artifact-id}.json`; review
   evidence must equal
   `_bmad-output/governance-evidence/context-review-{context-review-id}.json`.
   Each receipt uses the lifecycle JSON
   serializer, is regular and non-symlink, and is referenced one-to-one by its
   matching commitment, binding entry, artifact transition, pin, or review
   record. Commitments, bindings, pins, and reviews persist its exact digest;
   transition receipt bytes are deterministically reconstructed from the closed
   artifact tuple and then protected by the history gate. A receipt is
   provisional while staged or journaled and may be removed only by CAS-protected
   rollback of the transaction that created it. After successful post-install
   verification and any workflow-required durable checkpoint commit complete,
   it is immutable, never deleted or reused, and append-only
   history protects its path and bytes. Missing, extra, multiply referenced,
   noncanonical, non-JSON, schema-invalid, or prematurely final evidence fails.
   Every receipt remains covered by sensitive-content, protected-study,
   local-state, and publication checks.
3. The approval snapshot payload has exactly `artifact`, `content`,
   `workspace`, and `context_sources`. `artifact` contains the complete
   record as it stood at approval, including identity, type, title, path, owner,
   approver, approval time/evidence, scope, parent, `supersedes`,
   classification, and independence-exception fields, with status fixed to
   `approved` and later supersession/archive fields omitted. `content` is
   exactly `{"kind":"flat","digest":SHA256}` for an immutable flat artifact,
   `{"kind":"workspace","digest":lifecycle_digest}` for a workspace-backed one,
   or
   `{"kind":"bmad-build-story-v2","dispatch_binding":BINDING,"frozen_digest":SHA256,"immutable_frontmatter_digest":SHA256}`
   for a Build story. The immutable frontmatter digest uses the lifecycle JSON
   serializer over exactly `title`, `type`, `created`, `baseline_commit`,
   `governance_dispatch`, `governance_context_review`,
   `governance_authority_paths`,
   `governance_successor_reason`, and the ordered `context` path array as they
   stood at approval. `governance_context_review` is exactly `{id,digest}`. Its
   two values equal the `context_review_id`/`context_digest` of the flat draft
   binding before approval. Afterward, its ID identifies the unique retained
   review from which the commitment's five-field context source is projected,
   and its digest equals the projection's digest; these shapes are not
   structurally equal.
   `governance_successor_reason` is null for a base story or exactly
   `intent-gap`, `context-reconciliation`, or `dispatch-reconciliation` for its
   successor. `governance_authority_paths` is the sorted unique list of exact
   machine-policy authoritative-source paths this story may change, empty by
   default; the human explicitly confirms it at Checkpoint 1 and no prose
   inference may add a path. At Checkpoint 1 the team hook captures current Git `HEAD` into
   `baseline_commit`, requires a lowercase 40- or 64-hex commit object reachable
   from the checkout, and refuses `NO_VCS`; locked Step 3 then preserves it.
   `governance_dispatch` is the same closed union as `dispatch_binding`; it and
   all governance fields are added in staging before Checkpoint 1 and let every
   draft/resume restore native routing without filename inference. Those fields
   are immutable. Each `context` value preserves the installed literal syntax
   `{project-root}/` followed by one canonical repository-relative POSIX suffix.
   The ordered array is never empty: its first and exactly-once entry is
   `{project-root}/_bmad-output/project-context.md`, so the locked context-free
   implementation handoff loads the same reviewed authority and protected-study
   rules as its parent. Remaining entries are unique, route-bound governed or
   authoritative sources and may not shadow that canonical fact.
   Validation strips only that exact token, applies the full contained
   regular/non-symlink governed-or-authoritative path checks and exact-byte
   safety scan to the suffix target, and rejects every other token, absolute
   spelling, backslash, dot segment, or unresolved path. Only
   `status` and `review_loop_iteration` frontmatter fields are workflow-mutable.
   The frozen digest covers the exact
   inclusive frozen-intent byte region defined below. Outer Code Map, task
   checklist, change log, design, verification, evidence, and review sections
   remain workflow state outside the human-owned intent commitment and must obey
   their installed mutation rules; the commitment makes no broader byte-
   immutability claim for them. `dispatch_binding` is exactly one closed union:
   `{"kind":"spec","story_pin":{"spec_lineage_id":ID,"story_id":STRING,"governance_story_key":KEY,"story_artifact_id":ID},"dispatch_version":{"spec_artifact_id":ID,"dispatch_digest":SHA256,"approval_snapshot_sha256":SHA256}}`
   with immutable pin identity and the exact final dispatch-history entry used
   for this Build approval;
   `{"kind":"sprint","epics_handoff_artifact_id":ID,"native_story_key":KEY,"sprint_status_operational_id":ID}`
   with the exact validated full sprint key; or `{"kind":"adhoc"}`. It is
   retained by every Build successor. On resume the preflight restores the
   installed native runtime variables from this binding and revalidates their
   handoff/status source; it never derives them from a hashed successor filename.
   `workspace` is null for flat artifacts and otherwise contains the complete
   workspace record, including workflow, root path, lifecycle/dependency/support
   inventories, and approved digest. `context_sources` is the exact approved
   context-source list and must equal the commitment's persisted
   `context_sources`. Hash the payload with the lifecycle JSON serializer.
4. The verifier reconstructs that snapshot from current immutable fields and
   rejects identity, workflow, root, inventory, approval-metadata, content, or
   context-baseline drift even if `lifecycle_digest` was recomputed. Legal
   supersession/archive fields do not rewrite the original commitment. A new
   successor gets a new commitment; artifact IDs and commitments are never
   reused or replaced.
5. Add a repository-content-read-only history gate with one trusted base for
   every supported CI
   event. Pull requests use the exact `github.event.pull_request.base.sha`;
   direct pushes to `master` use the exact `github.event.before`. The removable
   BMAD CI marker rejects an empty, malformed, all-zero, unfetchable, non-commit,
   non-ancestor, or event-inappropriate value and rejects every unsupported
   trigger instead of falling back to local checks. It fetches only that exact
   commit read-only, then rejects modification or removal of any prior approval
   commitment, story-pin base field, story-pin dispatch-history prefix,
   context-review entry, operational binding-history
   prefix, legacy-unbound-draft list/base field, previously non-null legacy-draft
   reconciliation tuple, previously non-null supersession/archive tuple, or finalized
   governance-evidence path or bytes. It also preserves every retained legacy
   approval, independence-exception, supersession, and archive evidence path and
   exact bytes. New commitments, transitions, pins, context reviews, receipts,
   and appended binding entries must satisfy current authority and evidence
   rules.
   The local verifier always checks internal hashes; CI additionally checks
   append-only history against this event-derived base revision. This is
   integrity within the repository's existing Git review and authority model,
   not protection against an actor who controls both repository history and its
   human approval process.
6. The migration artifact is registered as `draft` in a version-1 cutoff index
   before its bytes are finalized. It records that cutoff's exact positive
   `index_revision` and exact index SHA-256 and enumerates every artifact already
   carrying a retained approval lifecycle at that cutoff—status `approved`,
   `superseded`, or `archived`—explicitly excluding only its own then-draft ID.
   Each enumerated entry contains the artifact ID, complete proposed approval
   snapshot digest, every retained original approval, independence,
   supersession, and archive evidence path with its exact digest or null, and
   every retained transition tuple. It separately records the exact
   artifact-ID-sorted `legacy_unbound_drafts`
   ID/path/file-digest/record-digest cohort with all four reconciliation fields
   null from that same cutoff, explicitly
   excluding its own then-draft ID. Its later fixed-path schema-v1 approval
   receipt contains the migration
   artifact's own approval snapshot digest in addition to the artifact and
   policy digests. Thus no byte inside the migration artifact hashes itself.
7. Version-2 activation is one journaled transaction under the repository-wide
   index lock. The current version-1 retained-approval cohort must equal exactly
   the enumerated cutoff cohort plus the one migration artifact; any intervening
   approval, supersession, archive, or evidence change requires a successor
   migration artifact and new review. Activation also requires the live
   version-1 unbound flat-draft IDs and exact bytes to equal the enumerated
   legacy-unbound cohort; drift or a new draft likewise requires a successor
   migration artifact and new review. Activation rechecks every enumerated
   snapshot, transition tuple, evidence path/null value, and exact evidence
   digest and bytes, then derives the
   migration artifact's bridge snapshot from its current approved record, exact
   content digest, and signed receipt, and gives every bootstrap commitment the
   candidate version-2 `index_revision` as `commitment_index_revision`. This
   revision means when the immutable commitment was installed, not an invented
   historical approval revision; each snapshot retains the original v1
   approval metadata. The transaction creates exactly those commitments,
   installs the approved policy and schema-version change, and runs
   the verifier before releasing the lock. It never edits, copies, or replaces
   original v1 evidence. Before this transaction, the final implementation must
   also pass the separate context-review gate below; activation installs its
   staged `project-context-r1` record and canonical receipt with the reviewed
   final project-context bytes. A cutoff/cohort mismatch, extra approved record,
   changed digest, failed install, or failed verifier restores the exact
   version-1 state through the transaction recovery protocol.

## Resource and portable-path limits

1. `workspace_rules` allows only UTF-8 text files ending `.md`, `.yaml`,
   `.yml`, `.json`, `.toml`, `.html`, `.css`, `.svg`, or `.excalidraw`, and
   no binary format. Extension comparisons are lowercase and exact. Executables,
   archives, PDFs, images, media, databases, device files, and every unlisted
   extension fail. An `.excalidraw` file must pass `bmad-excalidraw-v1`: a JSON
   object with exactly top-level keys `type`, `version`, `source`, `elements`,
   `appState`, and `files`; `type` is `excalidraw`, `version` is integer `2` but
   not a boolean, `source` is `https://excalidraw.com`, `elements` is an array,
   `appState` is exactly `{gridSize:null,viewBackgroundColor:"#ffffff"}`, and
   `files` is exactly `{}`. Each element has exactly the locked common fields
   `id`, `type`, `x`, `y`, `width`, `height`, `angle`, `strokeColor`,
   `backgroundColor`, `fillStyle`, `strokeWidth`, `strokeStyle`, `roughness`,
   `opacity`, `groupIds`, `frameId`, `roundness`, `seed`, `version`,
   `versionNonce`, `isDeleted`, `boundElements`, `updated`, `link`, `locked`,
   and `index`, plus only its closed type-specific fields. Version 2 permits
   types `rectangle`, `ellipse`, `diamond`, `line`, `arrow`, and `text`.
   Strings, finite numbers, non-boolean integers, booleans, nulls, ID arrays,
   colors, styles, and roundness use the exact Excalidraw-compatible types and
   bounded enums transcribed into the migration artifact; unknown or duplicate
   keys and non-finite or out-of-range values fail. `id` and every referenced ID
   use bounded ASCII `[A-Za-z0-9_-]+`; `frameId` and `link` are null,
   `isDeleted` is false, and every `index` is exactly two unique ASCII
   alphanumeric characters. Shape elements add no fields. Line and arrow
   elements add exactly `points`, `startBinding`, `endBinding`,
   `lastCommittedPoint`, `startArrowhead`, `endArrowhead`, and `elbowed`;
   bindings and last point are null, points are a bounded array of finite
   numeric pairs, arrowheads use a closed inert enum or null, and `elbowed` is
   false. Text elements add exactly `text`, `fontSize`, `fontFamily`,
   `textAlign`, `verticalAlign`, `baseline`, `containerId`, `originalText`, and
   `lineHeight`, with bounded text/font/alignment types and a null or same-file
   safe-ID container. Every non-null same-file ID reference resolves exactly one
   type-compatible element. Other than the fixed non-fetching top-level `source`
   metadata value, no field may contain a remote, protocol-relative, file, or
   data resource reference.
   It is permitted only in UX support inventory with role `working` or
   `wireframe`, never as lifecycle content. Adding a format or changing this
   signature requires a reviewed schema-policy migration.
   Every admitted `.html`, `.css`, and `.svg` file under either canonical root,
   whether lifecycle or support and regardless of owning workflow, must also pass the closed
   `bmad-inert-web-v1` stdlib validator before any workflow consumption,
   render, or open; only the bounded validation read precedes it. Reject
   scripts, inline/event-handler code, iframe/object/embed, forms or navigation,
   meta refresh, SVG `foreignObject`, active animation, CSS `@import`/expression,
   and every remote, protocol-relative, absolute-file, or unlisted resource
   reference. A support-origin relative reference must resolve canonically to an
   inventoried file in the same workspace. A lifecycle-origin relative
   reference may resolve only to a same-workspace lifecycle/support entry, the
   exact lifecycle path of a listed immutable `lifecycle_dependencies` entry,
   the canonical project-context path in that workspace's `context_sources`, or
   one exact authoritative-source path and approval-time digest retained in that
   bound project-context review. The last two are read-only governed references.
   For a draft, approval candidate, or executable artifact, the current target
   bytes must equal the bound digest. After a later reviewed source change,
   global verification of an older approved/superseded/archived artifact checks
   its immutable review mapping and link spelling without pretending the old
   target still exists at the live path; that artifact cannot be consumed,
   rendered, opened, or executed until its required current-context successor is
   approved. An authoritative-source byte change first forces a new project-context review
   and therefore the normal context-successor gate before execution. All other
   outside-workspace targets fail. Every `data:` URL is rejected. The complete
   local link/resource closure of every lifecycle file, including Markdown
   links and the installed UX links to `mockups/`, `wireframes/`, and
   `imports/`, is recomputed before approval. Any support file reached by that
   closure and every recursively reached support dependency must carry a
   matching non-null digest and is committed through `linked_support` in
   `lifecycle_digest`; no mutable null-digest support byte can steer approved
   rendered content. HTML parsing uses
   `html.parser`, SVG uses
   defused-by-construction `xml.etree.ElementTree` rules with DTD/entities
   rejected before parse, and CSS uses a bounded token scanner with fail-closed
   escaping. Validated web and Excalidraw previews run with network disabled and
   a sandboxed local origin; the installed suggestion to upload repository bytes
   to excalidraw.com is overridden by an offline-only viewer rule. Validation walks the complete bounded local-reference closure before
   any consumer opens one file. Golden malicious/benign fixtures cover mixed
   case, entities, CSS escapes, namespaces, redirects, and nested references.
2. One workspace may contain at most 256 inventoried files, 8 MiB per file,
   32 MiB total exact bytes, and eight descendant-directory levels. The two
   canonical roots together may contain at most 4,096 files, 4,096 directories,
   16 directory levels below a root, and 128 MiB. An
   index may contain at most 256 workspaces, 64 operational records, 4,096 story
   pins, 256 context reviews, 4,096 draft context bindings, and 4,096 artifact
   records. Each operational
   record has at most 64 binding entries, for at most 4,096 binding receipts.
   The governance-evidence surface may contain at most 24,832 files (one
   approval, pin, supersession, and archive receipt per applicable artifact,
   4,096 binding receipts, 256 context reviews, and 4,096 one-time legacy-draft
   reconciliation receipts), each at most 64 KiB and
   together at most 512 MiB. `_bmad-output/index.yaml` is at most
   16 MiB; `authorities.yaml` is at most 1 MiB; `project-context.md` is at most 8
   MiB; and each lock, policy, customization, or governance-evidence input has
   the stricter applicable limit stated here or in version 1. The verifier stats
   each regular file and rejects an over-limit input before reading it wholly.
   Index string fields retain the version-1 grammar and additionally cap titles
   and scopes at 512 UTF-8 bytes, rationales at 4,096 UTF-8 bytes, and every new
   unconstrained scalar at 1,024 UTF-8 bytes unless a smaller path, ID, digest,
   timestamp, enum, or story-ID rule applies. A plan or urgent incident that
   seeds a PRD has scope at most 400 UTF-8 bytes; a parent-null PRD also starts at
   at most 400. Every governable PRD has title at most 450 and scope at most 420;
   every promoted epics handoff has title at most 480 and scope at most 440; each
   later derived title/scope must remain within the global 512-byte ceiling.
   These cumulative stage bounds reserve the longest Build/readiness/retro
   suffix through an incident→PRD→epics chain. Preflight rejects an
   otherwise-valid but non-child-capable governor before writes. A v2 urgent
   incident must satisfy the seed bound so its mandatory retrospective route is
   total. Counts
   use real inventory entries, never user-declared summaries. Hashing and
   sensitive scanning stream in chunks no larger than 64 KiB; parsers reject an
   over-limit file before loading it wholly.
3. Repository-relative paths are at most 180 UTF-8 bytes and each segment at
   most 80 UTF-8 bytes. In addition to POSIX canonicality and NFC/casefold
   collision rules, every segment rejects trailing spaces or dots, ASCII C0
   controls, DEL, `"`, `:`, `*`, `?`, `<`, `>`, `|`, and a
   case-insensitive basename of `CON`, `PRN`, `AUX`, `NUL`, `COM1`
   through `COM9`, or `LPT1` through `LPT9`, with or without an extension.
   The Win32 aliases `COM¹`, `COM²`, `COM³`, `LPT¹`, `LPT²`, and `LPT³` are
   rejected on the same basis, with or without an extension. Golden tests run
   these rules against Windows and POSIX path fixtures so every accepted path is
   checkout-safe on the supported CI platforms.

## Repository-owned workflow routing

1. Before an enabled writer follows any generated workflow fallback, the
   identical root machine policies require
   `bash scripts/bmad-workflow-entry-guard.sh --skill {canonical-skill-id}`. This
   repository-owned, repository-content-read-only guard validates that the skill
   is on the closed
   enabled-writer list; its exact team override exists and matches every
   lock/provenance/policy digest; and the locked customization resolver
   successfully produces an effective configuration containing the required
   hook sentinels and safe destinations. It writes only a short-lived ignored
   activation token below `target/bmad-reservations`. Its exact canonical JSON
   is serialized with the lifecycle JSON serializer and has exactly
   `checkout_id`, `skill`, `effective_config_sha256`, `index_revision`,
   `token_nonce`, `issued_at`, and `expires_at`. `checkout_id` is lowercase
   SHA-256 of the UTF-8 canonical real checkout path; skill and config/revision
   match the verified state; `token_nonce` is 32 lowercase hex characters from
   at least 128 random bits; timestamps are UTC RFC 3339; and expiry is after
   issue but no more than five minutes later. It is exclusive-created with
   owner-only permissions. The earliest `activation_steps_prepend` bootstrap,
   before fact loading or any possible greeting/user pause, requires exactly one
   unexpired matching token, atomically consumes it, rechecks every binding, and
   converts it to one owner-only durable invocation handle with exactly the
   token's seven fields plus `invocation_nonce`, `consumed_at`, and
   `preload_snapshot`; the token
   expiry becomes historical and does not expire the live handle. Invocation
   nonce has the same random encoding and consumed time is UTC RFC 3339 no
   earlier than issue. `preload_snapshot` begins null and must be CAS-replaced
   before activation continues by exactly
   `{index_sha256,root_inventory_sha256,context_review_id,context_sha256,resume_view_path,resume_view_sha256}`.
   `index_sha256` hashes the exact index bytes. Each tree digest hashes a
   lifecycle-JSON list sorted by UTF-8 path bytes with entries exactly
   `{path,type,mode,sha256}`; type is `file` or `directory`, mode is
   `regular`/`executable` for files and `directory` for directories, and digest
   is lowercase SHA-256 for a file or null for a directory.
   `context_review_id` and `context_sha256` identify the unique latest retained
   review and exact canonical project-context bytes. Before any persistent fact
   is loaded, the preload guard takes the exclusive context-lease-registry lock
   shared with the refresh transaction, re-reads the exact index/latest review/
   context tuple while holding it, and creates an owner-only shared context-read
   lease before releasing that registry lock. The lease is serialized with the
   lifecycle JSON serializer and contains exactly
   `checkout_id`, `invocation_nonce`, `context_review_id`, `context_sha256`,
   `index_sha256`, and `acquired_at`. Context refresh takes the same registry
   lock before inspecting or changing context and must wait or halt while any
   such lease exists, so there is no check-to-lease race. Under that lease the loader hashes the canonical file
   immediately before and after reading it and requires both digests and the
   retained review to equal the preload snapshot. If an installed skill runs
   its append array before persistent-fact loading, as Architecture does, the
   same before/after checks run under the transferred context-bearing target
   reservation instead. The append preflight
   atomically transfers that same context identity into the long-lived target
   reservation and only then CAS-removes the read lease. A mismatch or missing
   lease halts before artifact consumption or writes. The resume fields
   are both null outside PRD/UX and otherwise name and hash the owner-only safe
   view below. Durable ownership is possession of that nonce plus the
   matching checkout/config/reservation bindings, not a short-lived helper PID.
   The active workflow retains that
   nonce across human pauses; only a matching nonce may resume or let the later
   preflight consume the handle when it acquires the target reservation.
   In that same earliest prepend, before persistent facts or any locked
   workflow resume scan, a repository-owned bounded preload guard validates the
   complete index and canonical-root ownership, resource, path, symlink,
   lifecycle, format, and sensitive-content rules. For PRD and UX it then
   builds an owner-only ignored `resume-view` below the invocation reservation:
   exact copies of only the already validated draft candidate primary files in
   their expected run-folder names, plus a lifecycle-JSON mapping from each
   copied path/digest to its canonical workspace record. It temporarily binds
   the locked `prd_output_path` or `ux_output_path` scan to that view, so
   activation step 5 never opens a live-root candidate. The later append hook
   maps a selected resume candidate back through that exact record, rechecks the
   live digest and full workspace, copies it to owned staging, and binds
   `doc_workspace` there; a Create route instead binds a new staging target.
   Mismatch or inability to replace either scan halts before consumption or
   write. Other enabled writers also complete the bounded preload validation
   before any artifact read. Tests cover an unindexed, oversized, non-UTF-8,
   symlinked, and race-changed PRD/UX primary and prove only the safe view is
   read.
   Cancellation or normal failure removes the owned context-read lease and
   handle by CAS. A lost-nonce
   orphan cannot be adopted. If it has no related reservation or staging, then
   after proving there is no mutation lock, journal, or canonical-root write, an
   explicit human abandonment decision may CAS-delete its exact bytes and is
   recorded in ignored local recovery state. If a dead owner left a handle,
   target reservation, and pre-journal staging output, an exclusive recovery
   lock first requires explicit human confirmation that the invocation is
   abandoned and proves the recorded index/evidence/canonical-target preimages
   are still exact. The human may then approve *discard*, which hashes and
   inventories the orphan staging tree into ignored recovery state and
   compare-and-swap deletes only that exact staging tree, reservation, and
   handle. It never installs, adopts, or changes a live root. Any preimage drift,
   competing nonce claim, mutation lock, journal, malformed state, or ambiguous ownership
   halts for the normal journal recovery/escalation path. Until a permitted
   cleanup completes, new entry halts. Orphan cleanup treats the context-read
   lease as part of the same nonce-owned state and may remove it only in the
   corresponding proven no-write or staged-discard branch. Under
   an exclusive token-directory lock,
   guard startup may CAS-delete only provably expired, unconsumed tokens whose
   checkout identity and owner match; it never removes a live, foreign,
   malformed, or ambiguous token. Reuse, ownership/clock mismatch, and multiple
   live matches halt. Crash before token consumption, crash between handle and
   reservation, lost-nonce abandonment, long human pause, cancellation, expiry,
   reissue, safe-GC, and crash-after-staged-write discard tests are mandatory.
   Missing `uv`, resolver
   error, invalid TOML, missing/hash-drifted override, unsafe effective config,
   or absent/stale token at bootstrap or absent/mismatched handle at preflight is
   a mandatory halt before code or artifact writes,
   even when the installed PRD, Architecture, UX, Spec, or Sprint workflow says
   to fall back to generated defaults. The team preflight consumes the matching
   handle and rechecks its configuration digest. For enabled writers, the guard rejects
   personal overrides of activation arrays, persistent facts, destination or
   routing keys, and every key outside the approved preference-only allowlist;
   personal configuration cannot run after or undo the team sanitizer.
   Production-shaped tests must break each
   failure mode and prove the generated fallback is never entered.
2. Every artifact-writing workflow enabled by this migration uses a team hook
   that runs the repository-owned write preflight before its first possible
   write. PRD, Architecture, UX, Spec, Epics, Build, and Retrospective place the
   full target preflight in `activation_steps_append`; Sprint Planning is the
   exact ordering exception and places it in the earliest
   `activation_steps_prepend`, because its installed activation step 5 executes
   readiness/generate/status/validate/fix before the append array and readiness
   may stop there. The preflight reads the fully merged base, team, and
   personal customization, validates the effective destination, acquires an
   exclusive long-lived target reservation below `target/bmad-reservations`,
   resolves one effective approved governing root, and presents the complete
   proposed metadata for human confirmation. A root is normally an approved
   `plan` or `prd`. For a brand-new initiative with no eligible root, only PRD
   Create may establish a parent-null root after explicit human confirmation of
   its owner, repository scope, title, and classification; nothing downstream
   may use it until it is approved. For urgent work, PRD Create may instead use
   one approved `urgent-security-incident` as parent; all later full-BMM outputs
   select the resulting approved PRD or its descendants, preserving an exact
   parent chain back to the incident. The reservation pins the selected
   governing artifact ID and approval-commitment digest and receives the exact
   `{id,digest}` context review from the preload lease before that short-lived
   lease is removed. It remains a context-bearing reservation through all human
   pauses and workflow writes. Under the same context-lease-registry lock, a
   refresh scans and conflicts with every such target reservation, not only
   Build leases; only the sealed Build exception below may proceed. Candidate verification
   and final CAS require that same record to remain the unique effective approved
   governor. If it changed during human input, the run must explicitly reconcile
   to a successor and restart/fork or abort without installing stale-parent
   output. The reservation owns an isolated
   same-filesystem staging area outside the canonical roots and remains held
   until successful installation or complete rollback. A separate
   repository-wide index-mutation lock serializes only commit and recovery
   transactions. A personal scalar may change a preference but cannot remove
   the earlier team array entry; a destination differing from the confirmed
   reservation halts. No critical completion behavior relies on replaceable
   scalar `on_complete`.
3. `delegated_write_rules` permits a parent holding that reservation to issue a
   one-shot capability only for an installed workflow-declared `review`,
   `distill`, `ux-creative`, `input-reconciliation`, or `memory-recovery`
   operation. The lifecycle-JSON-serialized
   capability has exactly `parent_skill`, `reservation_nonce`, `delegate`,
   `operation`, `targets`, `persistent_facts`, `context_review`, `issued_at`,
   `expires_at`, and `nonce`. `targets` is a non-empty ordered list of at most
   eight exact objects `{path,role,content_class}` naming staging targets;
   `persistent_facts` is the parent's exact sanitized ordered list;
   `context_review` is exact `{id,digest}`; timestamps and 32-lowercase-hex
   nonces follow the token rules. The child receives and
   consumes it
   before writing; it may write only that target set in the parent's staging area and
   may not touch a canonical live root, index, evidence, configuration, source,
   or another target. The parent inventories and transactionally installs the
   result. This covers workflow-required `bmad-review`/review subagents, UX
   distill/mockup/wireframe producers, PRD/UX input-reconciliation children, and
   the PRD Update bootstrap memory-recovery child without making them
   independent artifact owners. Input reconciliation may write only one
   pre-resolved `reconcile-{slug}.md` support target. Memory recovery may append
   only to the exact staged `.memlog.md` lifecycle target through the locked
   memlog helper after the approved workspace has been forked; it cannot edit an
   approved predecessor. A delegate must use the parent-supplied facts without re-resolving its
   own wildcard defaults or falling back; an effective fact mismatch halts.
   Missing, broad, reused, expired, mismatched, or unlisted delegation
   halts. Direct invocation of an unsupported writer without such a capability
   remains forbidden, and production-shaped tests exercise both allowed and
   denied delegated writes.
4. For a new PRD, architecture, UX, or spec workspace, artifact ID equals the
   unique workspace directory basename; type is respectively `prd`,
   `architecture`, `ux-design`, or `plan`. A root PRD has null parent and its
   explicitly confirmed scope; an incident-seeded PRD has the incident parent
   and `{incident-scope}/prd`; every other record has its selected approved
   governing root as parent and that scope plus respectively `/prd`,
   `/architecture`, `/ux`, or `/spec`. Title is non-empty and human-confirmed;
   owner is explicitly confirmed for a root PRD and otherwise copied from the
   governor; classification is explicitly human-confirmed; status
   is `draft`; every approval/transition/independence/archive field is null;
   and path is the workspace primary. The preflight snapshots current project
   context and prepares the exact workspace inventory. Resuming a draft must
   match its record; updating an approved run uses the fork transaction.
5. Critical team activation instructions remain in force through the run. All
   governed BMAD artifact, index, and evidence output is written and
   inventory-reconciled in the reserved staging area. Build's product/source
   edits remain normal working-tree changes authorized by its approved story;
   they are not silently swept into this governance transaction and retain the
   installed revert/review rules. After human input is complete, the transaction acquires the
   repository-wide index-mutation lock while retaining the target reservation;
   records the expected index revision and exact index SHA-256 plus every
   destination's expected absence or byte digest; prepares the candidate index,
   evidence, and files; and verifies the complete candidate in a disposable
   checkout. It then compare-and-swaps every live preimage, writes a durable
   journal and rollback copies, promotes each same-filesystem file by an
   individually atomic rename, and replaces the index last as the logical commit
   point. The multi-path transaction is journal-recoverable, not instantaneously
   atomic. A normal read-only verifier fails closed whenever it sees an active
   mutation lock or journal. The transaction invokes the same full public
   verifier in a narrowly scoped post-install mode that first proves a matching
   lock-owner nonce, journal phase, candidate index digest, and every installed
   target digest; no other invocation may bypass the fail-closed check. Every
   workflow-required Git checkpoint uses the journal's exact `commit_intent`
   object before any branch-ref movement. That lifecycle-JSON object has exactly
   `branch_ref`, `parent`, `tree`, `message`, `author_name`, `author_email`,
   `author_time`, `committer_name`, `committer_email`, `committer_time`,
   `pathspecs`, and `commit_oid`; times include exact UTC offsets, pathspecs are
   sorted unique canonical repository paths and equal the transaction's allowed
   installed set, and all other scalar bytes are fixed before hashing. Starting
   from an index equal to the parent tree, the helper stages only those
   pathspecs, writes and verifies the exact proposed tree, creates the commit
   object from that fixed metadata without moving a ref, records and fsyncs its
   computed object ID in the journal, then compare-and-swaps `branch_ref` from
   `parent` to `commit_oid`. Recovery under the same lock has only two accepted
   ref states: at `parent`, it may re-prove and retry the exact ref CAS or roll
   back before ref movement; at `commit_oid`, it verifies the unique commit's
   parent/tree/message/identity/times and exact pathset, restores the index to
   that tree if needed, proves the worktree, and completes the pending receipt,
   lease, and journal finalization. Any other ref, commit, tree, index, path, or
   metadata state halts with the journal. Once the ref equals `commit_oid`,
   rollback or history rewrite is forbidden. This same protocol governs Build
   draft, approval, handoff/cancellation, sealed implementation, and completion
   commits, closing every ref-update-to-state-CAS crash window. Only a
   successful post-install verification—and every exact durable checkpoint
   commit required by the owning workflow—marks newly promoted receipts final,
   deletes the journal and backups, and releases the index lock. A non-Build
   transaction releases its target reservation when its workflow is complete.
   A Build draft-save transaction instead retains the same invocation/target
   reservation (and, for a successor, the predecessor's `replanning` lease) for
   Checkpoint 1; a draft has no approval commitment or checkpoint preimage and
   must never be converted to an execution lease. Only the successful Build
   approval transaction, after its exact checkpoint commit, atomically converts
   that retained base reservation or replaces the `replanning` predecessor lease
   with the fully populated execution lease below. On any failure it rolls back only when a
   second compare-and-swap proves the index and targets still equal this
   transaction's installed digests; otherwise it preserves the journal, halts,
   and never overwrites concurrent state. Startup encounters with a journal or
   abandoned reservation must recover or escalate before another write.
   `index_revision` is a strict non-boolean integer from 1 through
   9,223,372,036,854,775,807. Every successful logical index mutation, including
   v2 activation, sets it to exactly the locked preimage value plus one. An
   aborted transaction restored to the prior exact state consumes no revision;
   a later live transaction may never reuse or decrease the current preimage.
   The journal records both revisions and the transaction helper proves the
   one-step CAS while it exists. Because Git commits may squash several valid
   local transactions, CI does not claim to reconstruct erased intermediate
   revisions: when index bytes differ, it requires the head revision to be
   strictly greater than the trusted-base revision and applies the separate
   append-only commitment, evidence, pin, review, transition, and binding-prefix
   checks. Reaching
   the maximum halts for a separately approved schema migration.
   Interruption tests at every boundary must prove that mandatory recovery
   yields either the prior valid state or the complete new valid state, and that
   verification before recovery fails closed rather than accepting an
   intermediate tree. In particular, a Build draft-save transaction that
   creates a story-pin receipt retains its journal, provisional receipt, lock,
   and rollback state through the exact `chore(bmad): draft …` commit; only that
   verified clean commit finalizes the receipt. Before ref movement, failure may
   retry or roll back under the same journal. After the ref equals the recorded
   draft `commit_oid`, recovery must adopt the exact commit and finalize its pin
   receipt/reservation state; it may not roll back. No boundary leaves a
   finalized pin beside an uncommitted draft or a committed draft beside
   provisional evidence. Recovery leaves no unclaimed file, duplicate reservation,
   or stale lock. Real rendered create, resume, update,
   personal-override, cancellation, verifier-failure, and crash-recovery flows
   must demonstrate the protocol; if an installed workflow cannot honor it,
   implementation halts rather than weakening verification.
6. Keep the two current canonical roots. Route optional `bmad-spec` output via
   `_bmad/custom/bmad-spec.toml` using the literal repository-relative scalar
   `spec_output_path = "_bmad-output/planning-artifacts/specs"`; do not use the
   unavailable `{planning_artifacts}` token and do not add default
   `_bmad-output/specs` as a third root.
7. Add exact team overrides for `_bmad/custom/bmad-prd.toml`,
   `_bmad/custom/bmad-architecture.toml`, `_bmad/custom/bmad-ux.toml`, and
   `_bmad/custom/bmad-spec.toml`.
   Their exposed `activation_steps_append` hooks must read the effective index
   state and refuse an approved destination before the workflow writes. All
   eight enabled-writer overrides append the literal persistent fact
   `file:{project-root}/_bmad-output/project-context.md` and add an exact
   `activation_steps_prepend` normalization that runs before fact loading. It
   removes only the installed generated literals
   `file:{project-root}/project-context.md` and
   `file:{project-root}/**/project-context.md` from the in-memory resolved list,
   rejects any other project-context path or glob and every additional tracked
   or visible untracked file named `project-context.md`, and requires the
   canonical fact exactly once. Fact loading must occur while either the
   invocation's matching context-read lease or its exact transferred
   context-bearing target reservation is live and must pass the before/after
   digest checks above; a live-path read without one of those guards is
   forbidden. It never writes a
   compatibility file or generated customization. If the runtime cannot prove
   that normalized list before loading facts, it halts. Preflight hashing is not
   a substitute for loading the canonical file into the workflow. The
   UX override must inspect every proposed `.working/` or `imports/` source
   before copy and admit only the closed format/signature set above. It also
   overrides the installed external Design-handoff instruction: tools write
   first to reservation-owned ignored quarantine outside the canonical roots,
   never directly to `doc_workspace`; preflight safety/signature validation may
   promote only a supported export into workspace staging. An
   unsupported Figma source, sketch format, brand-deck PDF, media file, archive,
   or mixed image folder remains outside the workspace; the workflow may inspect
   it transiently only when repository data policy permits, but must neither
   copy it nor persist its potentially sensitive external path. It asks for a
   supported export or halts before the first workspace write. Tests cover both
   import-copy and direct Design-handoff paths, including unsupported native
   output that never enters the repository. The
   append-only activation contract must invoke the inventory helper after
   creation, update, validation, or review output so every lifecycle, context,
   dependency, and support file is classified before final verification.
   Activation reconciles interrupted draft runs before resume. The rendered
   effective customization and create, resume-draft, dynamic-companion/support,
   canonical-context-load, personal-override, and fork-approved behaviors must
   be tested for each workflow.
8. Add `_bmad/custom/bmad-create-epics-and-stories.toml`. Its exposed
   `activation_steps_append` must read the effective indexed workspace-backed
   `bmad-prd` and `bmad-architecture` primaries, display each candidate's exact
   path and repository lifecycle status, require explicit selection, retain the
   generated prerequisite confirmation, carry the selected paths into the
   workflow's prerequisite candidate set, and write the chosen paths to
   `inputDocuments`. A real rendered end-to-end workflow test must prove the
   generated prerequisite step consumes those nested primaries. If the exposed
   hook cannot make that contract effective, implementation must halt for a new
   approved approach; generated workflow edits and compatibility shadow files
   remain forbidden.
9. Treat `_bmad-output/planning-artifacts/epics.md` as a reserved operational
   working document, not an approvable singleton artifact. Before final `[C]`,
   the epics override must resolve one effective approved PRD and a compatible
   approved architecture in the same governing-root lineage, and
   resolve `governing_prd_lineage_id` as the unique oldest PRD in that PRD's
   supersession lineage. It derives `governing_key` as the lowercase unpadded
   52-character base32 of
   `SHA256(UTF8(governing_prd_lineage_id))`, so a reconciled PRD successor keeps
   the same epics namespace. It prevalidates this complete proposed
   draft record: ID `epics-{governing_key}` for the first handoff or the next
   unused `epics-{governing_key}-rN`, with positive `N` capped at 9,999; type
   `plan`; title `Epics and stories for {governing-prd-title}`; owner copied from
   the PRD; scope `{governing-prd-scope}/epics`; parent equal to the PRD ID;
   human-confirmed classification; and every approval,
   transition, independence, and archive field null. Its path is
   `_bmad-output/planning-artifacts/{id}.md`. The user must confirm the proposed
   ID, parent, scope, and classification before `[C]`; the hook may not infer
   approval. The operational working document's source bindings include the
   required selected approved PRD and architecture commitments plus every other
   confirmed `inputDocuments` path, each of which must resolve to exactly one
   effective approved artifact and commitment; unindexed, draft, support-only,
   duplicate, or ambiguous inputs halt. They also bind the current reviewed
   context, and the content digest is refreshed after each owner write.
10. After `[C]`, one repository-owned transactional helper consumes exactly
   that prevalidated metadata, moves rather than copies `epics.md`, and writes
   the draft index record with rollback so failure leaves the prior state
   recoverable and never strands two contracts. It installs the matching flat
   draft context binding and then halts for explicit
   artifact approval. Sprint readiness accepts only the promoted effective
   approved handoff and must not consume either the mutable working file or a
   draft promotion. If exactly one unapproved draft already exists for the same
   governing PRD lineage, the next owning-workflow entry acquires its
   reservation, validates its record and context binding, copies its bytes to an
   isolated staging `epics.md`, and resumes that same ID. Re-finalization
   compare-and-swap replaces the same draft and source binding; it never
   allocates `-rN` or creates a competing record. A stale context or changed
   effective PRD/architecture must be reconciled before resume. Once a handoff
   is approved, later revisions—including mandatory context/parent
   reconciliation—repeat promotion under the next ID and use normal explicit
   supersession; prior approved handoffs are never overwritten.
11. Add `_bmad/custom/bmad-sprint-planning.toml`. Its earliest
   `activation_steps_prepend`, after the independent entry guard but before
   persistent-fact loading and installed activation step 5, must determine and
   persist exactly one intent from `readiness`, `sprint-planning`, `status`,
   `validate`, or `fix`; ambiguity is resolved there rather than deferred to the
   generated greeting. It consumes the invocation handle, completes the bounded
   preload, transfers the context-read lease, runs the full governing-handoff
   and target preflight, and acquires the long-lived reservations needed for
   both the sprint singleton and any optional readiness report. It validates
   and copies an existing sprint singleton into reservation staging, exposes
   that copy as the only status/validate input, and rebinds every generated
   script output and readiness-save destination to reservation staging. A
   repository wrapper rejects any canonical-root output argument or direct live
   write. Each generate/fix/write command returns only after its staged result
   passes the closed validator and journaled operational transaction. The
   prepend-installed readiness wrapper owns every terminal branch: after a
   saved FAIL it completes the flat-draft transaction, final invariant audit,
   and CAS release of the reservation/handle; after PASS, CONCERNS, or a
   declined FAIL save it proves no governed write, performs the same audit, and
   releases them. Only then may the installed `report, stop` return. The later
   append hook performs the final invariant audit and reservation teardown for
   the non-readiness paths that reach it; it is neither the first write guard nor
   the sole cleanup path. A crash before either release leaves the exact owned
   state for normal recovery rather than an unexplained live reservation. If the exposed prepend cannot force these
   paths for every step-5 branch, Sprint Planning halts before that branch and
   the migration cannot activate. Thus the contract reserves and, after every
   write, validates `_bmad-output/implementation-artifacts/sprint-status.yaml`
   through `operational_files` with the closed sprint-status validator. The
   execution-state vocabulary remains mutable workflow state, not artifact
   lifecycle. It binds the sprint singleton to the effective approved promoted
   epics handoff, records that handoff commitment and its context review as the
   exact source set, and refreshes the content digest after each validated
   owner write. Reuse for a later initiative follows the evidenced append-only
   rebinding transaction for either a present or absent target. A present prior
   sprint file is hashed, retained in rollback storage, identified in the
   binding receipt, disposed only under the reservation and index lock, and
   replaced with the newly validated bytes by the same journaled CAS; an absent
   target records the null preimage. A readiness `FAIL` may be saved
   only after the user confirms an exact staged
   flat record. Resolve `governing_epics_lineage_id` as the unique oldest
   handoff in the selected handoff's supersession lineage. Here `governing_key`
   is the lowercase unpadded 52-character base32 of
   `SHA256(UTF8(governing_epics_lineage_id))`. Its ID is
   `readiness-{governing_key}` or the next unused
   `readiness-{governing_key}-rN` with the same 9,999 cap; type is `plan`; title
   is `Implementation readiness for {epics-handoff-title}`; parent and owner
   come from the effective approved epics handoff; classification is explicitly
   human-confirmed, with the parent's value only a proposed default; scope is
   `{epics-handoff-scope}/readiness`; and every approval, transition,
   independence, and archive field is null. The team hook redirects the
   installed fixed `implementation-readiness.md` offer to the exact unique
   staging path and transactionally installs the draft at
   `_bmad-output/planning-artifacts/{id}.md`. It never writes or overwrites the
   fixed canonical-root path. If exactly one unapproved readiness draft already
   exists for the same effective handoff lineage, a later FAIL-save run restores
   it to staging, validates/reconciles its flat draft context binding, and
   replaces that same ID by CAS after human confirmation; it does not allocate a
   competing suffix. After approval, a later saved run uses a successor ID and
   normal supersession; declining the offer writes nothing. Rendered tests must
   prove the prepend reservation/redirection is established before installed
   step 5 and cover leak-free readiness PASS, CONCERNS, FAIL-save, FAIL-decline,
   and early-stop cleanup, plus full generation,
   status, validate, fix, canonical-output rejection, collision, readiness
   early-stop finalization, and approved-predecessor behavior.
12. Add `_bmad/custom/bmad-build.toml`. Its append-only activation preflight
    requires the plan-code-review route for every repository Build operation
    and halts before code or artifact writes if the installed one-shot route is
    selected; one-shot has no preimplementation Checkpoint 1 and is therefore
    not a governed implementation path in this repository. The preflight
    makes the installed version-control sanity check a machine gate before any
    locked Step 1 artifact read. It may first read only the owner-only ignored
    reservation state needed to distinguish a new invocation from a closed
    resume. For a new invocation, Git and a real reachable `HEAD` are required;
    merge, rebase, cherry-pick, bisect, detached HEAD, a branch ref not
    explicitly confirmed for this intent, staged or
    tracked-worktree changes, and any non-ignored untracked path all halt. A
    human may move the work to a clean dedicated branch/worktree, but cannot
    waive this preimage rule. Ignore classification for the entire run uses only
    the repository ignore rules in that clean `HEAD` tree; later `.gitignore`
    edits and global or `.git/info` excludes cannot hide a path from a change
    manifest. The only ignored paths permitted to change outside the manifest
    are the closed derived-output set `target/**` (with
    `target/bmad-reservations/**` separately nonce-owned),
    `forgetest-results/**`, and untracked files ending in `.sarif`, `.pdb`, or
    `.rs.bk`. At entry, every other pre-existing ignored path is inventoried by
    path/type/mode/exact digest in owner-only `ignored-preimage.json`; its bytes
    are never echoed, and the file/list use the change-manifest size and count
    caps. Such a path must remain byte/mode identical through completion, and
    any new ignored path outside the closed set halts. Thus an ignore-rule
    change that would newly hide source, configuration, or evidence remains a
    manifest difference or an explicit failure. Verified closed-set build output
    and repository-owned reservation state are outside the commit set and may
    not be loaded as governed planning/source input. The preflight records exact
    `HEAD` and `HEAD^{tree}` and acquires one checkout-wide Build mutation
    reservation bound to the invocation nonce and the confirmed exact branch
    ref; it holds that reservation through
    approval, implementation, review, explicit commit, or verified revert. Thus
    every later non-ignored Git change begins from one clean preimage and belongs
    to this invocation; a branch-ref change, conflicting writer, or reservation
    halts. A resume may
    bypass the new-entry cleanliness test only when the same invocation nonce
    proves an existing lease, checkpoint commit, and exact current change
    manifest; every live difference must match that state. A lost nonce or
    malformed/mismatched lease uses the explicit human recovery path below and
    can never become a generic dirty-tree override.
    The preflight
    rejects any implementation `spec_file` inside a planning workspace. Only a
    folder-plus-ID dispatch from governed `bmad-spec` `stories.yaml` derives the
    exact `governance_story_key` above. It routes that story to
    `_bmad-output/implementation-artifacts/spec-{governance-story-key}.md`; a digest-key
    collision or an unrelated occupant halts rather than suffixing or
    overwriting, and the artifact ID is exactly the filename stem. It never
    overwrites the installed runtime variable `story_key`, which remains the
    validated full sprint-status key used by status synchronization and epic
    selection. Sprint/epics and ad-hoc Build routes retain the installed unique
    safe-slug file and native `story_key` behavior and use a null story pin.
    A sprint route requires the exact effective approved epics handoff named by
    its dispatch binding as parent. Only an ad-hoc route may select another
    effective approved plan or epic parent; PRD is not a valid story parent and
    the preserved parent rule is not widened.
    Before locked Step 1 lists or reads planning/implementation artifacts, the
    override replaces both live-root inputs with a reservation-owned read-only
    bundle. A Spec route contains only the effective approved Spec lifecycle,
    its committed dependencies, current dispatch entry, current reviewed
    context, and allowed authoritative sources. A sprint route contains only the
    effective epics handoff, its committed input/dependency closure, bound
    sprint status/cache, and current context. An ad-hoc route contains only the
    explicitly human-selected effective plan/epic governor, its committed
    dependency closure, and current context. Freeform discovery may select more
    inputs only after each resolves to a same-lineage effective approved
    commitment and is added to that bundle; drafts, support-only paths, another
    initiative, and raw live-root scans are forbidden. Previous-story continuity
    is filtered by the exact `governance_dispatch` lineage and effective governed
    status, never a shared epic number, filename order, or globally lower story
    key. Codebase/source inspection remains available outside artifact roots.
    If Step 1 cannot be redirected to this bundle, it halts before planning or
    code writes. Rendered tests seed tempting unrelated/draft artifacts and
    same-number stories in another initiative and prove none are loaded.
    For sprint and ad-hoc routes, before writing it stages a flat `story` record whose ID equals the exact
    `spec-{safe-slug}[-N]` filename stem, with suffix `N` chosen by the installed
    collision rule and capped at 9,999; title is the human-confirmed dispatch or
    intent title;
    parent is the selected effective governor; owner is inherited;
    classification is explicitly human-confirmed (the parent's value is only a
    proposed default); scope is `{parent-scope}/stories/{artifact-id}`; status is `draft`;
    and every approval, transition, independence, and archive field is null.
    Missing governor, overlong/invalid slug, exhausted suffix, or occupied
    unrelated path halts before writes.
    The activation preflight captures the immutable `governance_dispatch` and
    `governance_context_review` before any draft resume and sets
    `governance_successor_reason` to null for a base story. It also presents and
    captures the exact sorted `governance_authority_paths`, empty unless the
    human explicitly authorizes named machine-policy sources. After every generated
    Step 2 render it reinserts and revalidates all four exact fields in staging
    before Checkpoint 1, because the locked template does not know them. Missing
    or changed governance state halts instead of inferring it from a filename.
    When the source is a `bmad-spec`
    `stories.yaml` entry, the dispatcher consumes all six materialized fields:
    exact `title` and `description` plus `invoke_dev_with` are presented as the
    planning input, with `invoke_dev_with` appended verbatim only after the
    normal safety scan; `spec_checkpoint` is surfaced and enforced (the
    repository's mandatory Checkpoint 1 still applies when it is false); and
    `done_checkpoint` makes the caller pause after accepted completion before
    dispatching another story. No field is dropped, inferred, or reinterpreted,
    and the exact six-field digest is the dispatch version committed below. The
    same transaction installs the first external story record and its exact
    `story_pins` entry. The draft record type is `story`;
    title comes from the selected approved Spec dispatch; parent is exactly the
    pin's approved `spec_artifact_id`; scope is
    `{parent-scope}/stories/{governance-story-key}`; owner is inherited; classification is
    explicitly human-confirmed; and every approval, transition, independence,
    and archive field is null.
    For every route, generated Step 2 writes only the reservation-owned staged
    story. Before presenting Checkpoint 1, a draft-persistence transaction
    installs or CAS-replaces the exact canonical draft file, index record, flat
    context binding, and initial Spec pin/receipt when applicable, runs the
    public verifier, and creates a local commit containing only that journal's
    path set with message `chore(bmad): draft {artifact-id}`. The first Spec
    draft transaction is the pin-install transaction above. An `[E]` edit or
    regeneration returns to staging and repeats the same verified draft-save
    transaction with message `chore(bmad): revise draft {artifact-id}` before
    the checkpoint is offered again. No approval fields or commitment exist in
    those commits. Thus a pause or lost invocation leaves either ignored staged
    bytes that the orphan-recovery rule may discard, or one clean, complete,
    verifier-valid durable draft commit that a new invocation can resume; it
    never leaves unexplained dirty governance state. Immediately before `[A]`,
    the staged candidate sets `baseline_commit` to that clean current draft
    commit and the human reviews the exact candidate that the approval
    transaction will promote.
    On every later folder-plus-ID dispatch, the preflight follows that pin through
    the Build-story supersession lineage and requires the pin's exact raw ID and
    materialized dispatch record to remain active in the unique effective
    approved Spec head. Before first approval it resumes the one pin-matching
    base draft; if the Spec head, dispatch digest, or context advanced, it
    reconciles that mutable draft in staging, updates parent/title/dispatch
    version/context binding, and requires human confirmation before approval.
    When the dispatch digest changed, the hook shows the complete old/new
    six-field records, saves the old unapproved block only as read-only
    reconciliation input, removes that exact block from the staged draft before
    generated Step 2 captures `preserved_intent`, and seeds Step 2 from the new
    approved dispatch. It then requires exactly one freshly rendered block and
    proves its intent reflects the new title/description/`invoke_dev_with`; it
    never reinstates the old block. Cancellation leaves the prior draft bytes,
    record, and binding exact. Rendered tests exercise this preapproval rewrite
    separately from the approved dispatch-successor route.
    After approval it binds `spec_file` to the unique latest
    effective successor, except that one pending draft successor may be resumed
    ahead of its predecessor when its exact `governance_successor_reason` is the
    currently required `intent-gap`, `context-reconciliation`, or
    `dispatch-reconciliation`. Every branch must match the required context
    review, parent, scope, classification, complete dispatch union, and next
    revision path. An intent-gap successor retains the predecessor dispatch binding; a
    context successor uses the exact route-specific rebound binding defined in
    item 13; and a dispatch successor binds the final current dispatch-history
    entry. More than one pending draft, a missing,
    forked, cyclic, mismatched, or otherwise ambiguous lineage halts. It never
    resumes a superseded base, creates a second base file, or falls through to
    the installed slug route; a completed effective story also cannot silently
    create another file. Rendered
    sprint tests cover initial routing, explicit-path resume, successor handling,
    epic-cache selection, and in-progress/review status updates with the native
    `story_key` unchanged.
    Retirement detected at initial dispatch, resume, re-entry, Checkpoint 1, or
    immediately before Step 3 follows the cancellation rule above; approval
    state never overrides retirement. At Build Checkpoint 1, `[A]` counts as artifact approval only when the human
    maps to an active scope-eligible authority. Before Step 3, the workflow must
    create the exclusive provisional approval-evidence receipt, write the index
    lifecycle fields and approval commitment, then run the verifier under the
    transaction protocol; otherwise it halts. It then stages only the exact
    story, index, approval/pin evidence, and other files named by that approval
    journal, proves the staged tree equals the verified candidate over the
    original clean `baseline_commit`, and creates one local checkpoint commit
    with message `chore(bmad): approve {artifact-id}`. Build treats successful
    creation and clean-tree verification of that commit as the final step of the
    approval transaction: before it, any failure rolls back the still-provisional
    evidence and candidate; after it, the evidence is immutable. A crash with a
    fully created checkpoint commit is recovered as committed state, while a
    partial stage/journal must complete or roll back by exact CAS before entry.
    Only after post-install verification and that checkpoint commit succeed does the approval helper replace the runtime
    `spec_file` value from its reservation-owned staging path with the exact
    installed canonical story path, then recheck the installed regular-file
    digest, index record, commitment, and dispatch binding. Step 3 reads that
    rebound canonical path; every proposed change to mutable story workflow
    state is first written to lease-owned staging and installed only through the
    repository-owned Build-state CAS helper. That helper verifies the live
    preimage, lease nonce/phase, immutable frontmatter and frozen-region
    commitment, exact allowed outer-section mutation, safety, and candidate
    bytes, then journal-renames the candidate over the same canonical path and
    post-verifies it. It never changes index lifecycle or approval fields. A
    sprint-status write uses the normal operational-file transaction and updates
    content digest/index revision under the index lock. These are the sole
    post-approval governed-output write routes; direct canonical story/status
    writes halt. If the generated Step 3/4 instructions cannot be wrapped this
    way, Build is unsupported. A missing/early/failed rebind halts
    before product changes; it never leaves `spec_file` pointing at a staging
    path removed by promotion. Real rendered tests cover the Step 2 staging
    path, human checkpoint edit, approval promotion, runtime rebind, and Step 3
    read for initial and successor flows. Before releasing the target
    reservation, the helper converts it to an owner-only Build execution lease
    serialized with the lifecycle JSON serializer and containing exactly
    `artifact_id`, `approval_snapshot_sha256`, `context_review`,
    `dispatch_digest`, `parent_artifact_id`, `parent_approval_snapshot_sha256`,
    `baseline_commit`, `checkout_preimage`, `ignored_preimage_manifest`,
    `reviewed_change_manifest`,
    `handoff_change_manifest`, `sealed_change_manifest`,
    `context_refresh_manifest`, `sealed_checkpoint`,
    `final_change_manifest`,
    `authorized_authority_paths`, `phase`, `invocation_nonce`, and `leased_at`.
    `checkout_preimage` is exactly `{head,tree}`; both are lowercase
    repository-native Git object IDs for the clean checkpoint commit and its
    tree. That commit has `baseline_commit` as its sole parent and its complete
    diff is exactly the approval-journal path set. The story's immutable
    `baseline_commit` remains the original preapproval head so installed review
    still covers both the checkpoint delta and later implementation; the change
    manifests use the checkpoint tree as their product/source preimage.
    `ignored_preimage_manifest` is the exact `{path,digest}` reference to
    `ignored-preimage.json` captured by the clean-entry rule; unlike the later
    change manifests it is required and immutable for the lease lifetime.
    `reviewed_change_manifest`, `handoff_change_manifest`,
    `sealed_change_manifest`, `context_refresh_manifest`, and
    `final_change_manifest` are initially null and otherwise exactly
    `{path,digest}`. Each path is inside this invocation's owner-only ignored
    reservation directory: the reviewed basename is
    `reviewed-changes-rN.json` for review iteration 0 through 5, and the other
    basenames are `handoff-changes.json`, `sealed-changes.json`,
    `context-refresh.json`, and `final-changes.json`. Each file is
    exclusive-created, regular, non-symlink, at most 4 MiB, and its lowercase
    exact-byte SHA-256 equals `digest`. Its bytes are one lifecycle-JSON list,
    capped at 4,096 entries and sorted by UTF-8 path bytes. Each list entry
    has exactly `path`, `class`, `preimage_sha256`, `preimage_mode`,
    `current_sha256`, and `current_mode`; `class` is exactly `source`,
    `governed-output`, or `context-refresh` and is derived from the closed policy
    path sets rather than caller input. Digests or modes are null for an absent side, allowed modes
    are `regular` and `executable`, and rename is represented as one deletion
    plus one addition. Paths are canonical repository-relative non-`.git` paths;
    symlinks, submodules, special files, path escapes, and protected-study
    targets are forbidden. Before creating any reviewed, handoff, sealed,
    context-refresh, or final manifest, the hook requires the Git index tree to
    equal the current `HEAD` tree. Staged content therefore cannot diverge from
    the worktree while a two-state manifest is formed; any staged path halts for
    an explicit non-lossy unstage and complete recheck rather than being reset
    or guessed. Before each manifest the helper also revalidates the immutable
    ignored preimage and closed derived-output set against the checkpoint ignore
    rules; a new or changed ignored path outside that set fails instead of being
    omitted. The first review manifest then covers every tracked-worktree and
    every untracked difference not in that closed set from the recorded clean
    tree, including governed story/index/evidence changes. No difference may be omitted. Before
    every Step 4 reviewer dispatch, the hook creates the
    next reviewed manifest and constructs the reviewed diff from the exact
    `baseline_commit`-to-checkpoint approval delta plus those manifest entries.
    Any patch, loopback, or later byte/mode/path change invalidates that
    manifest and requires a new bounded review iteration. Once review accepts,
    entering `sealed-output-refresh` exclusive-creates `sealed-changes.json`
    from the exact current state and sets the matching lease reference; any
    subsequent product or previously governed-output drift invalidates the seal.
    That manifest describes the exact prospective tree obtained by applying its
    entries to `checkout_preimage.tree`. If the permitted context refresh is
    selected, the context transaction also exclusive-creates
    `context-refresh.json` and sets `context_refresh_manifest`. It contains
    exactly the three context-transaction paths—canonical project context, the
    new context-review receipt, and `_bmad-output/index.yaml`—with class
    `context-refresh`. Each entry's preimage digest/mode must equal the value in
    the prospective sealed tree, not merely the earlier checkpoint or live
    caller claim, and its current value equals the verified context candidate.
    The only path permitted to occur in both manifests is
    `_bmad-output/index.yaml`; for that overlap the context entry's preimage must
    equal the sealed entry's current digest/mode and the context entry's current
    value deterministically replaces it. Every non-overlap applies normally.
    This ordered sealed-then-context overlay defines one unambiguous combined
    target tree and is rechecked before promotion, after promotion, and before
    commit. With no refresh, `context_refresh_manifest` stays null. Any other
    overlap, missing path, preimage mismatch, context delta, or extra path
    invalidates the seal and rolls back the still-provisional context
    transaction.
    `sealed_checkpoint` is initially null and otherwise exactly `{head,tree}`
    for the clean implementation commit defined below.
    `authorized_authority_paths` equals the approval-bound
    `governance_authority_paths` byte-for-byte; `phase` starts as `implementing`.
    Dispatch digest is null for non-Spec routes; all other values
    equal the installed approved state. Step 3, every Step 4 loop, any resume,
    and every story/sprint-status write revalidate the lease and its live
    commitments. `phase` is exactly `implementing`, `reviewing`, `replanning`,
    `sealed-output-refresh`, or `finalizing`. CAS permits
    `implementing`→`reviewing`, `reviewing`→`implementing` for an accepted patch
    loop, `reviewing`→`replanning` for a successor handoff, and
    `reviewing`→`sealed-output-refresh`→`finalizing`; `replanning` terminates only
    through the atomic successor-lease replacement or exact clean cancellation
    below. While it exists, project-context refresh, relevant Spec
    successor/pin-history or retirement changes, parent/handoff transition, and
    operational rebind transactions conflict and must wait or halt; they cannot
    invalidate an executing story. The sole context-refresh exception is this
    same lease in `sealed-output-refresh`: implementation and review are
    complete, every product and governed-output digest is frozen in local lease
    state, no further semantic write is permitted, and every changed authority
    path is in `authorized_authority_paths`. The human-reviewed refresh may then
    bind the post-implementation authority bytes as context for future work; it
    does not rewrite this story's historical input-context commitment. When a
    refresh runs under this phase, its context receipt and journal remain
    provisional through a mandatory sealed checkpoint. The Build hook combines
    the exact sealed and context-refresh manifests with the ordered overlay
    above, rejects every extra/staged difference, stages those exact pathspecs,
    proves the resulting tree, and creates the conventional local
    implementation commit derived from the story title through the pre-recorded
    `commit_intent` protocol. After its branch-ref CAS, it atomically records
    that exact commit/tree as `sealed_checkpoint`, advances the lease to
    `finalizing`, proves the checkout clean, and only then finalizes the context
    receipt and clears its journal. With no refresh, the
    same sealed checkpoint commits the sealed manifest alone. A failure before
    commit either completes that exact commit or first rolls back the still-
    provisional context transaction and then restores `source` changes to
    `checkout_preimage`; it can never retain a new context review while reverting
    its authoritative bytes. A crash after the expected commit's ref CAS but
    before the lease CAS is recognized by the still-durable commit intent: after
    re-proving the exact HEAD/parent/tree/message/pathset and provisional context
    journal, recovery must adopt that commit, CAS-record `sealed_checkpoint`,
    enter `finalizing`, and finalize the receipt; it may not roll back or rewrite
    the ref. A crash after the sealed checkpoint retains the committed source
    and context state and must resume finalization; it never reverts them.
    After that clean checkpoint, `finalizing` permits only installed declared status/review/evidence
    and validated sprint-status completion writes. On a sprint route, the
    locked Step 5 `target_status=review` write first passes through the normal
    operational transaction. The Build terminal hook then requires the story
    frontmatter to be `done`, every built-in Step 4 decision/patch finding to be
    resolved, verification green, the native story key unchanged, and the live
    lease in `finalizing`; in a second operational transaction it changes only
    that exact sprint entry from `review` to `done` and its declared update date.
    This parent-mediated terminal hook is the sole normal governed
    `review`-to-`done` route. Independent `bmad-code-review` is not an enabled
    artifact writer and must halt before changing a governed story or sprint
    singleton; Build obtains review advice only through its already authorized
    capability-scoped Step 4 layers. Any later issue enters a new governed Build
    successor/reconciliation route. A
    missing check or failed terminal transaction leaves `review` and the lease
    intact rather than falsely completing. It then exclusive-creates
    `final-changes.json` relative to `sealed_checkpoint.tree`; that manifest may
    contain only the closed deterministic finalizing writes. Any intent, dispatch,
    authority, or product byte change invalidates the seal and requires
    revert/review before another attempt. The repository commit hook compares
    every live path, mode, and digest with the final manifest, rejects any
    staged-index preimage or extra difference, stages only the manifest's
    explicit pathspecs, proves the resulting index tree contains exactly those
    changes over `sealed_checkpoint.tree`, and creates the local completion
    commit `chore(bmad): complete {artifact-id}`. It never uses a whole-tree add. Commit failure or post-stage drift
    leaves the lease for recovery and commits nothing; success must leave no
    difference outside the closed mutable derived-output set and must revalidate
    the exact ignored preimage. An `intent_gap` or `bad_spec` revert first requires
    every current path to match the latest reviewed manifest, restores only
    `source`-class tracked bytes and modes from the recorded clean commit, and
    deletes only `source`-class manifest additions whose preimage is null.
    Approved index/evidence and the story's permitted outer workflow state stay
    governed by their own CAS routes; they are never mistaken for product code.
    Any out-of-manifest or CAS drift is refused rather than clobbered. The lease
    is CAS-released only after accepted
    completion or after the installed revert proves product/source and governed
    state restored on cancellation. A lost-nonce or explicitly abandoned lease
    requires human-approved recovery and verified baseline/revert before
    removal. For post-approval but pre-sealed-checkpoint abandonment, that recovery restores every
    source-class entry to `checkout_preimage`, restores later mutable governed
    state to the approval checkpoint through its CAS journal, retains the
    immutable approval checkpoint commit/evidence, and removes only the exact
    ignored lease/manifests after verification. The resulting clean tree may
    start a new invocation that resumes the approved story. After a sealed
    checkpoint, source/context rollback and lease removal are forbidden. The
    sole lost-nonce adoption exception is an explicitly human-authorized
    finalization-only recovery: under the exclusive recovery lock it proves
    `HEAD`/tree equal `sealed_checkpoint`, and either the index/worktree are clean
    or every difference is owned by an exact recoverable finalizing journal and
    final manifest. It also proves the story approval/context/dispatch/parent/
    operational commitments remain exact and no semantic write is pending, then reloads the same owner-only nonce and
    runs only the closed finalizing writes and completion commit. Any mismatch
    keeps the lease and requires escalation. Uncertainty keeps the
    lease and blocks conflicting transitions. Crash, concurrent context/Spec/
    parent change, review-loop, accepted completion, and revert-failure tests
    are mandatory. The commitment uses content kind
    `bmad-build-story-v2`, binds the applicable story pin and immutable
    frontmatter projection, and hashes the exact
    inclusive frozen-intent byte region. The file must contain exactly one
    unnested opening sentinel line
    `<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">`
    and exactly one later closing line `</frozen-after-approval>`, with no other
    opening/closing tag substring. Both are UTF-8 LF-terminated lines. The
    committed region begins at the opening line's first byte and ends after the
    closing line's LF. Any alternate attribute, CRLF, duplicate, nesting,
    missing final LF, or malformed delimiter fails rather than selecting a
    region. Real rendered tests must prove post-approval `context`, title, type,
    or created-field injection fails while the exact installed mutable fields
    and sections may evolve; they must also prove one-shot is
    rejected before implementation and the generated spec-folder route cannot
    restore `{spec_folder}/stories/**`; if the exposed hook cannot enforce
    either rule, implementation halts for a new plan. The team rule also
    intercepts the installed no-subagent review fallback: if synchronous review
    delegates are unavailable, Build halts before writing any prompt under the
    implementation root. It may resume when compliant delegates are available;
    unindexed prompt-file fallback is never emitted. A rendered no-subagent test
    proves the canonical roots and story bytes stay unchanged.
    The team rule likewise intercepts every reachable writer of the separately
    indexed, differently scoped `deferred-work.md`: Step 1 and Step 2 `[S] Split`
    store deferred goals only in ignored invocation state until the story can
    place them, and Step 4 `defer` writes its finding directly. All land under a
    repository-defined `## Deferred Work` section in this story's mutable outer
    workflow state and are transactionally verified with it. The global file
    remains byte-identical. Promoting an entry into separate durable work
    requires its own governed planning route and human approval. Rendered tests
    cover both branches before and after a story file exists.
13. A Build `intent_gap` never edits the approved story's committed frozen
    region. The installed review step may first increment
    `review_loop_iteration` and record review state only in the explicitly
    mutable section outside that region. Immediately after the `intent_gap`
    branch is known and after reverting implementation changes as the installed
    workflow requires, the repository hook first completes an exact lease
    handoff transaction. It proves every `source` path and mode equal the
    approval-checkpoint preimage, moves the existing lease from `reviewing` to
    `replanning`, exclusive-creates `handoff-changes.json`, stores its exact
    path/digest in `handoff_change_manifest`, and permits that manifest to
    contain only the predecessor's
    permitted mutable outer review state plus any validator-approved
    route-bound operational/index state. It installs those bytes by the same
    CAS helper and creates one clean local commit with message
    `chore(bmad): hand off {predecessor-artifact-id} intent gap`, staging only
    the explicit repository pathspecs enumerated by that manifest. The ignored
    owner-only manifest itself is never staged or committed. Under the same invocation nonce and still-held checkout-wide
    mutation reservation, it then acquires exactly one successor target
    reservation; no second Build lease or ownership gap exists. The successor's
    draft-save and approval checkpoints may proceed under this `replanning`
    lease even though ordinary new Build entry is excluded. The repository hook
    then reserves a new unique flat successor.
    A Spec-pinned story uses `spec-{governance-story-key}-rN.md`; a sprint or
    ad-hoc story uses `spec-r-{revision-key}.md`, where `revision-key` is the
    lowercase unpadded 52-character base32 of
    `SHA256(UTF8(predecessor_artifact_id) || 0x00 || UTF8(decimal N))`. In both
    cases the artifact ID equals the stem and `N` is the next positive unused
    revision capped at 9,999; a digest collision halts. It does not copy the
    committed frozen block. The new index record has artifact type `story`,
    retains the predecessor title, owner, and classification, uses the new
    ID/path, has status `draft`, and sets every approval, transition,
    independence, and archive field to null. Route-specific parent resolution is
    mandatory: a Spec story uses the unique effective approved Spec head for its
    pin lineage and scope `{parent-scope}/stories/{governance-story-key}`; a
    sprint story uses the exact effective approved epics handoff in its current
    dispatch binding and the corresponding story scope; an ad-hoc story retains
    its parent only if still effective or selects the unique compatible approved
    successor in that parent lineage and recomputes the same-shaped story scope.
    Missing, unrelated, ambiguous, or scope-incompatible parent succession
    halts. Its `created` value is the successor
    creation date and its `governance_dispatch` value is copied exactly from the
    predecessor; `governance_context_review` binds the unique latest review,
    `governance_successor_reason` is `intent-gap`, and the matching flat draft
    context binding is installed. `governance_authority_paths` is explicitly
    reconfirmed at the new checkpoint, with the predecessor list only a proposed
    default. It
    carries the predecessor's non-frozen continuity only as read-only planning
    context and retains the same immutable dispatch binding, including the
    story-pin lineage when present, and
    ensures the reserved successor path is absent before rebinding `spec_file`
    to it. The human-resolved intent is then the input to generated Step 2, whose
    draft-resume preservation branch cannot see an old frozen block. Before the
    next checkpoint the helper proves the successor contains one newly rendered
    frozen block whose digest differs from the predecessor and incorporates the
    resolution. A fresh Checkpoint 1 approval creates its own
    immutable evidence and commitment; a separately evidenced, scope-eligible
    supersession authorization transactionally approves the successor and transitions
    the predecessor under existing chronology rules. After the exact successor
    approval checkpoint commit, that same journal atomically CAS-replaces the
    predecessor `replanning` lease with one successor execution lease whose
    artifact, commitment, parent, dispatch, context, baseline, and checkpoint
    preimage fields equal the approved successor. That atomic replacement also
    CAS-consumes the exact successor target reservation and rekeys the retained
    checkout-wide ownership to the new lease; it never exposes two active leases
    or leaves a stale context-bearing reservation. A failure before the handoff commit rolls back only the current
    handoff journal to its immediate preimage and retains the lease for retry;
    it never undoes a previously successful story or operational transaction or
    decreases its index revision. Product/source bytes stay restored to the
    approval-checkpoint preimage. To abandon instead of retrying, the human must
    authorize a compensating operational transaction at the next revision when
    needed, followed by an exact-path clean commit with message
    `chore(bmad): cancel {predecessor-artifact-id}` whose diff contains only the
    explicit repository pathspecs in the validated predecessor/operational
    cancellation manifest; the ignored manifest remains local. Rollback of a successful
    prior revision is forbidden. When no successor draft was installed, that
    clean commit may CAS-release the successor target reservation, the
    `replanning` lease, and the checkout-wide reservation/handle together. After
    a handoff commit, cancellation retains exactly its verified predecessor
    outer/operational state. If a later durable successor-draft commit exists,
    cancellation may retain that one valid draft and skip the no-draft
    cancellation commit; after proving the tree clean and the draft/index/
    binding exact, it likewise CAS-releases the successor target reservation,
    `replanning` lease, checkout-wide reservation, and handle together so a new
    invocation can resume the draft. Until successor
    approval succeeds, every path is clean at one of those explicit boundaries
    and the predecessor's frozen region, commitment, evidence, effectiveness,
    and lifecycle links remain exact. A failed approval transaction rolls back
    its provisional successor approval/supersession changes to the last such
    boundary. A real rendered
    intent-gap loopback must prove the counter write is outside the frozen
    region, the predecessor is not opened for write after branch redirection,
    source rollback and the clean handoff commit are exact, the checkout-wide
    reservation never lapses, the runtime variable is redirected, and Spec,
    sprint, and ad-hoc successors
    restore the correct dispatch binding. If the team hook cannot redirect the
    installed same-path loopback, it halts before frozen bytes change. The same
    clean handoff and atomic lease-replacement protocol applies when a context or
    dispatch reconciliation is initiated from an active execution lease. If the
    invocation is cancelled after a clean handoff but before successor approval,
    it follows the applicable no-draft or saved-draft release transaction above;
    a saved draft remains resumable under normal routing. A lost nonce in `replanning`
    permits only journal recovery or this exact human-authorized clean closure,
    never adoption of product-writing authority.
    A mandatory context reconciliation without an `intent_gap` uses the same
    route-specific successor naming, revision cap, collision checks, and
    complete draft-record mapping. The preflight selects the unique effective
    predecessor (or its one already-staged matching context successor) and never
    resumes a superseded base. Retirement is checked first. A changed Spec
    dispatch digest takes precedence and uses dispatch reconciliation, which
    also binds current context. Otherwise a Spec context successor updates its
    parent to the effective Spec head and its `dispatch_version` to that head's
    equal-digest final history entry. A sprint context successor updates parent
    and dispatch handoff ID to the effective approved epics successor, preserves
    the native story key only when it still validates against the rebound sprint
    status, and completes the evidenced operational rebind before resume. An
    ad-hoc successor uses the effective parent resolution above. Any changed or
    missing native dispatch meaning halts for a new governed story instead of
    being inferred. It copies the predecessor's frozen intent exactly, updates
    `governance_context_review` and the matching flat draft binding to the latest
    review, sets `governance_successor_reason` to `context-reconciliation`,
    records the reconciliation in mutable workflow state, and requires a fresh Checkpoint 1
    approval plus separately evidenced supersession authorization before Step
    3. For this branch the frozen digest may remain equal, but the immutable
    frontmatter digest and approval commitment must differ. Cancellation and
    rollback leave the predecessor exact. The rule forbidding a completed story
    from silently creating another file does not block this explicitly required
    reconciliation successor. More than one pending successor or any binding,
    parent, scope, classification, path, or revision mismatch halts.
    A dispatch reconciliation is mandatory whenever the pin's final
    `dispatch_digest` differs from the digest in the effective Build head's
    `dispatch_version`, whether that head is incomplete or accepted as complete. It uses
    the same route-specific successor naming, revision cap, complete draft
    record, rollback, approval, and supersession protocol. If implementation is
    in progress, the installed revert path runs before successor authoring. The
    successor's parent is the exact effective Spec head, its title is the current
    dispatch title after explicit human confirmation, and its scope is derived
    from that parent and the existing governance story key. It sets `governance_successor_reason` to
    `dispatch-reconciliation`, binds the unique latest context review and flat
    draft context entry, retains the immutable pin identity, and replaces only
    `dispatch_version` with the final current dispatch-history entry. This
    branch also satisfies simultaneous context drift; it takes precedence
    over a separate context successor. Generated Step 2 receives the complete changed canonical six-field dispatch record and
    the predecessor only as read-only continuity; it renders a new frozen intent
    rather than preserving the old block. The human is shown the exact old/new
    dispatch records and resolves their effect before fresh Checkpoint 1
    approval. The frozen digest may be equal only when the human confirms that
    the changed dispatch field does not alter intent; the changed immutable
    dispatch projection and new approval commitment are still mandatory. A
    later dispatch-history entry with an equal digest needs no successor and
    does not rewrite the approved Build binding. Omission follows
    retirement instead, never this branch. A matching pending dispatch successor
    is resumed before a predecessor; cancellation leaves the predecessor and
    pin history exact.
14. Build-created `epic-N-context.md` files use the exact operational cache
    contract above and bind to the effective approved epic handoff. Their
    `source_artifacts` are exactly the handoff plus the effective approved
    artifacts named by its validated `inputDocuments` and their transitive
    lifecycle-dependency closure, sorted by artifact ID; the context source is
    exactly the context review bound by the handoff commitment. The compiler may
    not scan the live planning root. The preflight verifies every commitment and
    digest, constructs a reservation-owned read-only source bundle containing
    only those exact bytes, and passes that bundle as the generated compiler's
    planning directory. The resulting cache digest and source bindings are
    recorded in the operational record. If the compiler cannot be redirected to
    that isolated bundle, it halts. Before Build reads a cache, its binding must
    name the selected effective handoff, its content digest and complete source
    set must still match, and its validator must pass; the installed
    modification-time freshness check is not an authority check. On a mismatch,
    under the target reservation and index
    lock, the transaction records the old binding and exact cache digest in
    disposal evidence, makes the target absent, stages and validates regenerated
    bytes, appends the next binding-history entry, and installs the new binding
    and cache together. Failure restores the old binding and file exactly.
    Stale bytes are never consumed. Build may update the
    already-bound sprint-status
    file but may neither rebind it nor bypass its validator. Existing
    `deferred-work.md` remains its separately indexed draft artifact.
15. Add `_bmad/custom/bmad-retrospective.toml` so the higher-tier urgent-security
   deadline always has a governed full-BMM retrospective route. Its preflight
   selects one effective approved completed epic handoff supported by the
   installed sprint or stories mode and binds the exact unit: a positive epic
   number for sprint mode, or one governed approved Spec workspace plus its
   applicable pin set for stories mode. Resolve `governing_epics_lineage_id` as
   the unique oldest handoff in the handoff lineage. Sprint `unit-id` is the
   unpadded ASCII decimal epic number. Stories `unit-id` is the unique oldest
   Spec lineage ID, while the current effective Spec artifact ID is retained
   separately; that Spec must share the handoff's governing-root lineage and its approval
   commitment must appear in the handoff's validated source set. Its pins must
   be exactly the applicable set for that Spec lineage. It derives
   `governing_key` as the lowercase
   unpadded 52-character base32 of
   `SHA256(UTF8(governing_epics_lineage_id) || 0x00 || UTF8(mode) || 0x00 ||
   UTF8(unit-id))` and
   reserves a unique flat record ID `retro-{governing_key}` or the next unused
   `retro-{governing_key}-rN`, with positive `N` capped at 9,999; type is
   `retrospective`; title is `Retrospective for {governing-artifact-title}`;
   parent is the governing ID; scope is
   `{governing-scope}/retrospectives/{governing_key}`; owner
   is inherited; classification is explicitly human-confirmed; and every
   approval, transition, independence, and archive field is null while the
   report is written. The report frontmatter has the exact common fields `date`,
   `verdict`, `criteria`, `headless`, `governance_mode`,
   `governance_handoff_artifact_id`, `governance_unit_id`,
   `governance_spec_artifact_id`, and `governance_incident_id`. `date` uses the installed normalized
   `MM-DD-YYYY HH:MM` string; `verdict` is `accepted`,
   `accepted-with-open-items`, or `rejected`; `criteria` is `declared` or
   `profiled`; and `headless` is boolean. Governance mode is `sprint` or
   `stories`; handoff is the exact approved epics artifact ID; unit ID is the
   unpadded positive epic number string in sprint mode or exact oldest Spec
   lineage ID in stories mode. `governance_spec_artifact_id` is null in sprint
   mode and the current effective approved Spec ID in stories mode; incident is
   null except for the one linked urgent incident.
   Sprint mode additionally requires the installed `epic` field as the matching
   non-boolean positive integer; stories mode forbids `epic`. No other
   frontmatter field is allowed. The flat exact-byte approval commitment binds
   this complete union, and the prose Acceptance verdict must equal `verdict`.
   If exactly one matching draft
   already binds the same governing handoff, mode, and unit, the workflow resumes
   that exact record/path after
   reconciliation; it does not allocate `-rN`. A new suffix is used only for a
   genuinely new run or successor, and multiple matching drafts halt. Both modes
   write the staged
   document to `_bmad-output/implementation-artifacts/{id}.md`; stories mode is
   forbidden from creating `{spec-folder}/RETROSPECTIVE.md` and instead resolves
   each `stories.yaml` entry through the applicable story pin and latest Build
   successor. A missing, retired, duplicate, or status-inconsistent mapping
   halts. Sprint mode may update the already-bound sprint-status operational
   file through its validator but may not rebind it. Finalization installs the
   draft report transactionally and halts for explicit artifact approval. When
   satisfying an urgent-security deadline, the selected handoff must trace
   through parent links to exactly one approved incident record. The report
   records that incident ID, its validated Europe/London deadline, the complete
   epic acceptance criteria, gathered evidence, unfinished-story result, and
   installed acceptance verdict; it does not invent an incident-only analysis
   mode. All revisions for the same incident/mode/unit form one normal
   supersession lineage; retained predecessors do not count as duplicates, and
   at most one matching draft successor may exist. After the deadline, the v2
   special-record verifier requires exactly one effective approved head, with
   the lineage's first approved record approved no later than the recorded
   deadline and matching incident/lineage bytes. "No later" means that first
   `approved_at` UTC instant converted to `Europe/London` has a local calendar
   date less than or equal to the recorded ISO date; enforcement begins on the
   next local date. Missing, late, duplicate, or
   unapproved output fails closed and cannot be repaired by changing the
   incident deadline. Real rendered tests cover both modes, external
   story status, sprint write-back, an incident-linked full-BMM epic, before/at/
   after-deadline enforcement, and refusal of the
   generated nested output. If the hook cannot redirect that output before its
   first skeleton write, implementation halts for a new approved approach.
16. Migrate `bmad.lock.json` and lock verification to recognize only the eight
   exact team workflow overrides `bmad-architecture`, `bmad-build`,
   `bmad-create-epics-and-stories`, `bmad-prd`, `bmad-spec`,
   `bmad-sprint-planning`, `bmad-ux`, and `bmad-retrospective`, alongside existing
   `_bmad/custom/config.toml`, as hashed `human-owned-team-override` files
   excluded from generated inventory. Do not add a wildcard exception for
   `_bmad/custom`. Add the closed `_bmad-output/governance-evidence` surface to
   the identical human-owned, never-overwrite upgrade policy in both machine
   policy copies. Preserve package/version identity, installation manifest,
   generated path set and hashes, regeneration provenance, launcher bytes, and
   user-local `*.user.toml` handling. Update every singular proof surface
   together: `scripts/bmad_verify_lock.py`,
   `scripts/bmad_verify_promotion.py`, `scripts/bmad_verify_provenance.py`,
   `scripts/bmad_verify_policy.py`, the exact lock evidence, and the identical
   AGENTS/CLAUDE machine-policy blocks. Adversarial tests must reject an unlisted
   team override, a missing/hash-drifted listed override, one claimed as
   generated content, or divergence between any two proof surfaces.
17. Add an exact repository-owned, human-reviewed project-context refresh
   transaction; do not misregister `bmad-project-context`, whose installed
   contract owns the root AGENTS instruction block, as this file's writer. The
   operator supplies a regular, non-symlink candidate at a reservation-owned
   ignored path outside `_bmad-output`. The helper validates its complete schema,
   repository-visible classification, active verified human reviewer metadata,
   current authoritative-source identities, protected-study tree identities,
   CI proof contract, UTF-8/LF bytes, size, and sensitive/publication rules. It
   reserves the context path, next unused `project-context-rN` with `N` capped
   at 256, matching canonical review receipt, and index preimage; presents the
   exact candidate and receipt hashes for explicit reviewer confirmation. It
   acquires the context-lease-registry lock and refuses while any context-read
   lease or context-bearing target reservation is live or unrecovered. It also
   refuses while any Build execution lease is live or unrecovered except the one
   matching lease in its sealed-output-refresh phase under the terminal
   protocol above; then it
   holds the index lock through disposable candidate verification, journaled
   CAS promotion of candidate/receipt/index, and post-install verification. The
   appended `context_reviews` entry and receipt use the exact schema above and
   the current file becomes byte-for-byte the approved candidate. Outside an
   active Build this completes the transaction normally. Under the permitted
   `sealed-output-refresh` lease, its journal and receipt remain provisional
   until the exact Build sealed checkpoint commit completes as specified above;
   recovery must complete that boundary or roll back both context and source
   preimages. Any failure
   restores the prior context, index, and evidence or preserves the journal on
   CAS ambiguity. Existing approvals and their historical context bindings are
   never rewritten. Existing workspace and flat drafts may retain stale review
   bindings but become inert until individually reconciled as specified above;
   every later execution of an approved artifact requires the applicable
   successor route. The transaction cannot alter AGENTS.md, CLAUDE.md, lock,
   policy, or another artifact; those require their own approved policy change.
   Tests cover first post-activation refresh, stale drafts, unchanged bytes,
   reviewer/identity drift, revision exhaustion, crash recovery, concurrent
   index mutation, and rollback.

## Standard workflow coverage

The version-2 model must represent these installed outputs without renaming
workspace files owned by their workflows:

| Workflow | Primary or durable artifact | Lifecycle files | Support or operational files |
| --- | --- | --- | --- |
| PRD | Workspace `prd.md` | `.memlog.md` as `memory`; `addendum.md` as `companion` | Reviewer, reconciliation, and validation reports as support |
| Architecture | Workspace `ARCHITECTURE-SPINE.md` | `.memlog.md` as `memory`; any load-bearing design rendering as `companion` | Reviewer and validation reports as support |
| UX | Workspace `DESIGN.md` | `EXPERIENCE.md` as `companion`; `.memlog.md` as `memory` | Mockups and wireframes, including every `.excalidraw`, remain support; any load-bearing decision is distilled into the two lifecycle spines |
| Epics and stories | Unique promoted flat epic/story handoff | None outside the promoted document | Mutable `epics.md` as registered operational working state |
| Sprint planning | Unique flat `readiness-{governing_key}[-rN].md` when a failed readiness report is saved | None | `sprint-status.yaml` as registered operational tracking state; fixed `implementation-readiness.md` is never written under the live root |
| Spec | Workspace `SPEC.md` below the customized planning root | `.memlog.md` as `memory`; every spec-authored contract companion as `companion`; `stories.yaml` as `dispatch`; adopted companions as frozen `lifecycle_dependencies`; project context as the exact governed source | Reviews as support; external story pins remain indexed across spec successors |
| Build | Flat indexed Spec-dispatch `spec-{governance-story-key}[-rN].md` or governed native safe-slug story contract | Frozen intent region, immutable frontmatter/context/dispatch-version projection, and applicable story-pin identity committed at approval; intent, context, or dispatch reconciliation forks to a new committed successor | Declared workflow state outside the frozen region; immutable approval evidence; exact epic-context cache and sprint-status operational records; execution lease through accepted completion |
| Retrospective | Unique flat indexed `retro-{governing_key}[-rN].md` | Approved report bytes as the retrospective contract | External story-pin status inputs or validated sprint-status operational update |

Only the eight named workflow overrides are enabled to write governed output by
this migration. Any other installed workflow may read or validate existing
artifacts but must halt before writing under a canonical root until a later
approved policy migration adds its exact team override, output contract, and
lock/provenance proof, except for one capability-scoped staging invocation over
an exact ordered set of at most eight targets under an exact
parent-issued delegated capability described above. A delegate is never an
artifact owner and cannot install its own bytes. Such a future independent
writer must use the same generic model;
neither schema nor verifier may grant an ownership bypass from a skill name.
The approved machine policy and project context contain the exact
`enabled_artifact_writers` list for these eight skills and the fail-closed action
`halt-before-code-or-artifact-write` for every other installed skill. Rendered
activation tests must prove at least `bmad-product-brief` and `bmad-build-auto`
stop without repository changes, including folder-plus-ID input that would
otherwise look for `stories/<id>-*.md`. They
must direct the user to a supported workflow or a new policy migration; they may
not create a compatibility child, fallback result, retrospective, or code
change.

## Migration and safety controls

1. Keep `_bmad` and `_bmad-output` internal by default. Existing forbidden-data
   scanning must inspect every tracked and staged workspace or operational blob,
   including non-Markdown files and symlinks. Publication verification must keep
   rejecting internal material from mdBook sources, helper copy rules, and
   rendered output.
   Add reviewed `.gitattributes` rules that force `text eol=lf` for every
   governed text artifact, `_bmad/custom` TOML, governance-evidence JSON,
   `bmad.lock.json`, BMAD verifier/test source, root machine policy, and approval
   evidence. No workspace binary format is admitted. Repository writers emit LF
   bytes explicitly and verification rejects
   CRLF before Git staging. Lock/policy proof binds the exact attributes bytes;
   Windows tests create CRLF-converting checkouts and prove committed blobs,
   worktree verification, lifecycle hashes, sentinels, and receipts remain exact.
2. Record the cross-version transition with the flat artifact
   `_bmad-output/planning-artifacts/artifact-governance-schema-v1-to-v2.md` of
   type `schema-migration`. While schema version 1 is still active, that artifact
   must contain the complete normative version-2 contract: every top-level
   field; every record and rules object with exact field sets, types,
   nullability, enums, regexes, ordering, cardinalities, and limits; the unchanged
   artifact schema and conditional workspace-primary exception; all workspace,
   operational, story-pin, commitment, evidence, context, dependency,
   lifecycle, format-signature, portable-path, resource, history-base, and
   transaction/CAS rules; the closed operational tuples and validator
   semantics; the legacy-approval and legacy-unbound-draft cohorts; the canonical
   five-field context-source projection; all serializers and golden vectors;
   and the exact eight-override, `.gitattributes`, and proof-surface allowlist. It embeds the
   canonical policy as recursively key-sorted compact UTF-8 JSON plus LF and
   states that block's exact SHA-256. Its fixed schema-v1 approval evidence path
   is
   `docs/gangsta/bmad-artifact-governance-v2/specs/schema-v2-approval.md`, whose
   signed review has mapping frontmatter with exactly `artifact_id`, `decision`,
   `approver`, `approved_at`, `classification`, `artifact_sha256`,
   `version_2_policy_sha256`, `approval_snapshot_sha256`,
   `independence_exception_approver`,
   `independence_exception_approved_at`, and
   `independence_exception_rationale`, followed by the fixed heading
   `# Approval: BMAD artifact governance schema version 2` and a one-paragraph
   approval statement. `artifact_id` is
   `artifact-governance-schema-v1-to-v2`, `decision` is `approved`, both
   authorities are `repository-don-001`, both timestamps are the same UTC RFC
   3339 instant, classification is `repository-visible`, and the three lowercase
   SHA-256 values bind the reviewed artifact, embedded policy, and bridge
   snapshot without including the receipt bytes in that snapshot. Schema
   version 2 is invalid without that
   retained approved migration record and evidence. After approval,
   implementation may only transcribe those approved semantics; any addition or
   change requires a successor migration artifact and another approval before
   activation.
3. Update repository instructions, `.gitattributes`, artifact READMEs, the production index,
   focused verifier modules, production-shaped fixtures, and adversarial tests
   as one reviewed change. Do not modify generated BMAD framework, workflow, or
   launcher files.
4. Preserve separate lifecycle vocabularies: index lifecycle remains
   `draft`/`approved`/`superseded`/`archived`; PRD and architecture
   `draft`/`final`, epics `stepsCompleted`, and sprint execution statuses remain
   workflow-local state rather than aliases for repository approval.

## Implementation sequence and human gates

1. After this plan itself is explicitly approved, draft the complete flat
   cross-version schema-migration artifact and the fixed-path evidence payload
   template described above under schema version 1. This is contract authoring
   only: do not change production policy, verifier code, workflow overrides, or
   schema version.
2. Validate the artifact's canonical policy block and all three declared hashes,
   then **halt and ask the Don to review the complete artifact, the exact
   artifact, policy, and bridge-snapshot hashes, and the
   exact evidence destination and payload fields.** Do not mark it approved and
   do not activate schema version 2 until the Don separately and explicitly
   approves the presented artifact and policy hashes. Only then write the signed
   evidence receipt, record its exact digest, the UTC RFC 3339 approval, and all
   four Don independence-exception fields. A changed artifact or policy byte
   after review invalidates this gate.
3. Add production-shaped failing tests that transcribe, without extending, the
   approved schema for every accepted workspace and operational form and for
   unindexed files, invalid roles/validators, duplicate claims, overlapping
   roots, traversal, symlinks, portable-name collisions, unsupported roots,
   invalid primaries, dependency drift, lifecycle-digest drift, staged-blob
   drift, approved-path reuse, exact root-README preservation, team-override
   lock drift, invalid or erased binding history, unauthorized/live-target
   rebinding, stale-dependency fork reconciliation or refusal, transaction/CAS
   failure, and public-output leakage. Cover every allowed and forbidden
   extension, all numeric resource boundaries including the 24,832-file
   evidence ceiling and legacy-receipt maximum, ASCII and superscript Windows
   device names, approval-commitment/evidence/pin drift against each CI event's
   trusted base, same-metadata context-body drift, context-review chronology,
   retained authoritative-source identity projections, and
   current-versus-historical receipt validation for both workspace and flat
   commitments, flat-draft context-binding TOCTOU, legacy-unbound-draft
   allowlisting plus file/record drift and one-way reconciliation receipt,
   mandatory successor creation
   on every context-digest change, post-activation human-reviewed context
   refresh with stale workspace/flat drafts and Build-lease exclusion,
   effective persistent-fact loading in all eight writers and every
   delegate, early bounded PRD/UX resume views plus serialized context-read-lease
   TOCTOU closure and Architecture's transferred-reservation fact load, Sprint
   step-5 prepend reservation/redirection and readiness-stop
   finalization, mandatory canonical Build-child frontmatter context,
   resolver/`uv`/team-override entry-guard/handle recovery failure,
   disallowed personal hooks, delegated multi-target writes, approved-Validate
   forks, supported and unsupported UX import/direct-handoff paths, bounded
   uppercase/repeated-dash story IDs and same-ID cross-Spec separation, strict
   canonical six-field stories dispatch, same-digest and changed-digest Spec
   successor history, preapproval dispatch re-render without stale
   `preserved_intent`, Spec story retirement at dispatch, resume, approval, and
   pre-implementation gates, native sprint-key restore, Build
   one-shot/intent-gap/context-reconciliation/resume/no-subagent/deferred-work
   routing, isolated source bundles, staged mutable-state writes, clean-checkout
   ownership, checkpoint-ignore reclassification and ignored-preimage drift,
   staged-index-divergence refusal, reviewed/sealed/context-overlay/
   final change manifests, draft-pin receipt finalization at the durable commit,
   clean intent-gap handoff, monotonic sprint-state cancellation recovery, and
   atomic successor-lease replacement, pre-recorded commit-intent recovery at
   every draft/approval/handoff/sealed/completion ref-update boundary,
   explicit-path commit and
   CAS-safe revert, execution leases, pre/post-sealed crash recovery, sealed
   post-implementation context refresh, and the sprint `review`-to-`done`
   terminal transaction, epics/readiness
   flat-draft resume, readiness
   report routing, operational source-set/cache integrity, global inert-web
   validation and complete local-reference closure, transition/binding
   receipts and reconciliation, monotonic index revision, LF/CRLF behavior,
   root-PRD and urgent-incident initiative bootstrap, urgent retrospective
   routing/deadline enforcement, and nested Build-output escape.
4. Implement fail-fast parsing and validation in focused verifier modules while
   preserving stable diagnostics, Python 3.10 compatibility, Bash 3.2 entry,
   read-only execution, streaming scans, and the exact new resource ceilings.
5. Migrate only the human-owned portion of every lock, promotion, provenance,
   and policy proof surface for the eight exact team override files; add the
   overrides; prepare the candidate production index and fixtures; and update
   identical AGENTS/CLAUDE machine-policy blocks, `.gitattributes`, and onboarding documentation.
   Keep the live index at version 1 until the activation transaction.
6. Exercise representative PRD, architecture, UX, epics promotion, sprint
   status, spec, build, retrospective, and root-initiative cases. Prove exact effective customizations, the real
   rendered epics prerequisite handoff, external Build story routing and
   checkpoint approval, all three Build dispatch/resume and intent-gap successor
   routes, no-subagent/defer interception, one-shot and unsupported-writer
   refusal, approved-path Update/Validate fork behavior, canonical context in
   every writer/delegate, canonical Spec
   context loading, UX Excalidraw/import validation, urgent Retrospective in
   sprint and external-story modes, current flat records,
   standard-library sprint validation with locked-script parity vectors, and all
   existing 211 assertions remain green.
7. Run candidate-mode BMAD verification and every unchanged CI proof gate.
   Confirm Cargo/product, protected-study, generated-launcher,
   generated-inventory, CI-baseline, and public-mdBook boundaries are
   byte-clean.
8. Obtain a fresh context-free governance and security review of the complete
   staged candidate.
9. Prepare the final human-owned project-context bytes with all new source
   identities and a proposed `project-context-r1` record and canonical JSON
   receipt. Present the exact context SHA-256, reviewer, UTC review time, and
   receipt bytes; **halt for explicit Don review of that exact context**. Any
   later context-byte change invalidates the review and repeats this gate.
10. Under the journaled cutoff/CAS protocol, activate schema version 2, the
    bootstrap commitments, reviewed context record and receipt, workflow
    overrides, evidence surfaces, and candidate files. Run post-install
    verification before releasing either lock; recover to exact v1 on failure.
11. Re-run the full harness, real public verifier, event-base history fixtures,
    and a final context-free review before closing the migration or beginning
    dependency-remediation planning.

## Acceptance criteria

- Registered representatives for every required full-BMM stage and optional
  installed UX/spec route pass with their installed filenames, explicit
  workspace, flat, governed-source, or operational ownership, and exact
  lifecycle/support inventories.
- With no existing initiative plan, a root PRD can be staged, explicitly
  classified, approved, and used by the remaining full-BMM chain. An approved
  urgent incident can instead seed that PRD, and the completed epics handoff and
  unit-specific Retrospective retain a verifiable parent chain to it.
- The same representatives fail for a missing or extra file, path escape,
  symlink, portable collision, ownership overlap, invalid operational validator,
  unapproved governing artifact, dependency drift, approved-path reuse, or
  approved workspace identity drift. The two exact root README governance
  surfaces continue to pass, no third unindexed README does, and an unregistered
  YAML, HTML, image, or other non-Markdown file anywhere under either canonical
  root fails.
- Existing flat approved and draft artifacts retain their IDs, paths, status,
  approval evidence, and verification behavior. Workspace-backed primary paths
  use only the explicit conditional exemption from the flat stem rule.
- The schema-migration record is separately approved under schema version 1 by
  an eligible authority, with real evidence and Don independence fields, before
  any version-2 state is accepted. Its signed evidence binds the complete
  canonical v2 policy bytes and its non-self-referential bridge snapshot; the
  cutoff cohort and commitment-install revision are exact, and implementation
  introduces no post-approval schema decision.
- Nested registered PRD and architecture primaries are presented and consumed by
  a real rendered epics workflow; failure of the exposed customization hook
  halts rather than creating generated edits or shadow contracts. Every enabled
  writer's independent entry guard fails before writes when resolution, `uv`,
  TOML, team-override identity, effective routing, activation token, invocation
  handle, or safe recovery fails. Sprint's guarded prepend owns every possible
  activation-step-5 read/write and cannot be bypassed by readiness early stop.
  Approved parent workspaces fork before Update
  or report-writing Validate, and exact delegated review/distill/render targets
  cannot escape their parent's staging reservation.
- A completed epics run leaves one unique durable handoff and preserves older
  approved handoffs. Its draft record has the exact plan-type/parent/scope/owner
  mapping above and sprint readiness blocks until that record is effective and
  approved; `epics.md` and `sprint-status.yaml` remain explicitly registered
  mutable operational state rather than lifecycle artifacts.
- Operational singleton ownership is always traceable through consecutive,
  evidenced binding history; rebinding a present file or binding to a
  non-effective governing artifact fails, and rebind authority covers both old
  disposal and new governing scopes. The public verifier validates sprint
  YAML without `uv`, network access, or non-standard Python packages. An
  `epic-N-context.md` cache bound to another handoff or stale approved source set
  is disposed and regenerated only from the committed handoff input closure and
  reviewed context through the same evidenced CAS flow before Build may read it.
- Approved specs freeze every adopted companion through a compatible,
  independently indexed lifecycle dependency; dependency replacement or byte
  drift cannot leave the dependent digest unchanged. Forking a dependent after
  a dependency transition either records an explicitly selected effective
  replacement in the staged contract and memlog or leaves the predecessor and
  repository unchanged.
- Approved `stories.yaml` dispatch bytes are lifecycle-bound. Build creates and
  registers story specs only under the implementation root, maps every
  repository-governable non-empty 1–256-byte raw story ID—including uppercase
  and repeated-dash forms—to its exact safe
  full-digest `governance_story_key`, transactionally pins that ID across Spec and Build
  successor lineages, and binds the
  human-approved frozen intent before implementation, and never writes a
  `stories/**` child inside a planning workspace. A retired pin cannot reappear;
  an `intent_gap` rebinds the installed loop to a separately approved successor
  with newly resolved frozen intent while its predecessor's committed frozen
  region remains byte-identical and only declared outer review state may change;
  while native sprint stories retain and restore their full installed
  `story_key`; and one-shot, no-subagent prompt output, global deferred-work
  appends, and every named
  unsupported writer halt before code or artifact writes. Build reads planning
  context only from its route-bound source bundle, stages every mutable story
  write through the lease CAS helper, freezes checkpoint ignore semantics and
  non-output ignored preimages, uses the pre-recorded commit-intent/ref-CAS
  protocol at every durable commit, and holds its execution lease so context,
  pins, parent handoffs, or operational bindings cannot change mid-run; an
  authorized source-changing story refreshes context only in the sealed terminal
  phase through the exact sealed-then-context index overlay, with a clean Git
  index required before each review or commit manifest is formed.
- The enabled Retrospective route writes one governed flat report in sprint or
  external-story mode, validates any sprint-status update, and can produce an
  approved full-BMM retrospective for an urgent-security incident by its
  recorded deadline without touching an approved Spec workspace. Retained
  retrospective revisions form one unit-specific lineage rather than false
  duplicates.
- Approval commitments bind artifact and workspace identity, workflow/root,
  approval metadata, content identity, inventories, and context baseline. Every
  new v2 approval has one exact canonical-JSON evidence receipt that becomes
  immutable only after successful post-install verification and any exact
  workflow-required checkpoint commit; the
  closed legacy cohort retains its original evidence unchanged through the
  journaled activation transaction. CI rejects coordinated rewrites,
  commitment/pin-base/evidence changes, pin dispatch-history or binding-history
  truncation relative to the
  exact PR base or direct-push `before` commit and fails closed when that base is
  not trustworthy. Every live successful index transaction advances the strict
  integer revision by exactly one and rollback consumes none; across a possibly
  squashed Git change, CI proves strict increase rather than fictional
  intermediate revision continuity.
- Fresh workspace registration uses the append-only pre-write handshake and
  rejects effective personal overrides that escape the confirmed destination.
  Its reservation and index lock remain held through staged verification,
  revision-and-preimage CAS, installation, post-install verification, and
  rollback. During a journaled transition, ordinary verification fails closed;
  after mandatory recovery the repository is exactly old or exactly new.
  Project context is governed by its one exact source exception, loaded as the
  sole canonical persistent fact by all eight writers and their delegates,
  recorded in workspace state, flat-draft bindings, or flat approval
  commitments as applicable, and bound by an immutable
  exact-byte human-review receipt no later than the same artifact-approval
  transaction. An owner-bound context-read lease keeps the preload digest stable
  through actual persistent-fact loading and transfers that identity into the
  writer reservation; the refresh registry conflicts with every such
  context-bearing reservation until it closes. Historical records resolve their retained review while only
  current/draft work binds the latest bytes; execution after any digest change
  requires a newly reconciled and approved successor even when the change is
  decision-neutral. The closed v1 cohort alone uses the explicit empty legacy
  binding. A post-activation refresh can advance that source only through the
  exact human-reviewed, journaled context-review transaction and cannot race an
  active Build execution lease.
- Only the eight named workflow customizations plus existing team config are
  accepted as hashed human-owned `_bmad/custom` files. Generated package and
  launcher identities remain unchanged.
- Every accepted path and file stays within the exact format, file-count, byte,
  depth, Windows portability, and streaming-resource limits. UX `.excalidraw`
  support bytes pass their closed JSON signature validator. Every admitted
  HTML, CSS, or SVG file anywhere under either canonical root passes
  `bmad-inert-web-v1`, including its complete bounded local-reference closure,
  before workflow consumption, render, or open. Unsupported UX imports and
  direct handoff formats never enter the workspace; governed text stays
  LF-exact across Windows checkouts; and ASCII and superscript Win32 device names
  fail.
- Sensitive-content, protected-study, local-state, CI-baseline, and publication
  negative tests remain effective for every newly supported file type.
- `bash scripts/test-verify-bmad.sh` and `bash scripts/verify-bmad.sh` pass, and
  removing the additive BMAD CI marker reconstructs the locked baseline workflow
  bytes exactly.
- No generated BMAD file, Rust source, Cargo dependency, protected study, or
  public mdBook source changes as part of this migration.
- The Rust dependency-remediation initiative can then enter the required PRD
  stage without producing an unindexable or verifier-breaking artifact.

## Non-goals

- Updating `h2`, `chacha20`, any Cargo manifest, or `Cargo.lock` in this
  migration.
- Changing BMAD Method 6.11.0, selected modules, generated skills, generated
  workflow filenames, installation-manifest identity, or generated inventory.
  The narrowly scoped human-owned declarations and evidence in `bmad.lock.json`
  are explicitly in scope.
- Treating repository visibility as confidentiality or access control.
- Publishing `_bmad` or `_bmad-output` content.
- Relaxing full-BMM planning, human approval, or protected-study rules.

## Approval needed

Approval of this plan is the first human gate only. It must identify
`repository-don-001`, use a UTC RFC 3339 time, cite repository-relative evidence,
at
`docs/gangsta/bmad-artifact-governance-v2/specs/workspace-compatibility-plan-approval.md`,
and record the Don independence exception because the registered owner and sole
available approver are the same verified human alias. Implementation must still
halt for the separate schema-migration-artifact approval described above.
