# Feature Specification: Spec Kit command center for macOS

**Feature Directory**: `specs/001-command-center`
**Feature Branch**: `001-command-center`
**Created**: 2026-11-08
**Status**: Clarified. All brainstorm questions answered by the human on 2026-11-08. Ready for `/speckit-plan`.
**Input**: A standalone macOS app that, when pointed at a project that uses Spec Kit, reads the `specs/` tree and renders a command center: a project view that shows where every spec stands, and a per-spec view that shows the spec, plan, tasks, progress, open questions, and review health in one clean interface. Read-only. Personal tool first; may be shared with the team later.
**Working name**: Spec Deck (placeholder)
**Reference repo**: `proj-nava` (104 feature folders, 2,044 tasks, 196 clarification sessions, 68 reviews), checked 2026-11-08.

## Overview

Spec Kit leaves a rich, structured trail in each feature folder, but the only way to read it today is to open markdown files one by one. With dozens of specs, an agent-driven pipeline, and work spread over many branches, it is hard to answer simple questions: what is in flight, what is blocked, what is next, and what did the agent decide without asking.

The app answers those questions by reading the files. It never writes to the repo. It treats the files on disk as the only source of truth, derives state from which artifacts exist and what they contain, and updates live while an agent works.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See where every spec stands (Priority: P1)

I open a project and see every spec grouped by lifecycle stage, with the ones that need attention first.

**Why this priority**: This is the reason the app exists. Without it, the app is a markdown viewer.

**Independent Test**: Open `proj-nava`. Within a few seconds, the project view lists all 104 folders, shows 12 as Building with task progress, and puts finished specs out of the way.

**Acceptance Scenarios**:

1. **Given** a repo with a `specs/` folder, **When** I open it, **Then** I see one entry per feature folder with its title, ticket, stage, task progress, review verdict, and last activity date.
2. **Given** specs at different stages, **When** I look at the project view, **Then** in-flight specs (Planned through Building) appear before Done specs, and Done specs are collapsed by default.
3. **Given** several folders share a ticket prefix (for example nine `26683-*` folders), **When** I group by ticket, **Then** they appear together under that ticket with combined progress.
4. **Given** `.specify/feature.json` names a feature, **When** I open the project, **Then** a "Now working on" banner shows that feature, its branch, and its ClickUp link.
5. **Given** I type in the filter box, **When** the text matches a title, ticket, or slug, **Then** only matching specs remain.

---

### User Story 2 - Open one spec's command center (Priority: P1)

I select a spec and see its full state on one screen: stage pipeline, tasks by phase, user-story progress, the next task, and links to every artifact.

**Why this priority**: Drilling in is the second half of the core loop.

**Independent Test**: Open `27450-shared-sign-in-host`. The screen shows Building, 20 of 24 tasks done, phase 7 (US1 integration) and phase 8 (Deployment) incomplete, and T060 as the next task.

**Acceptance Scenarios**:

1. **Given** a spec with `tasks.md`, **When** I open it, **Then** I see each phase with done/total counts and can expand a phase to see its tasks with ID, parallel flag, story tag, and text.
2. **Given** tasks tagged `[US1]`, `[US2]`…, **When** I view user stories, **Then** each story from `spec.md` shows its priority and the progress of its tagged tasks. A story with no tagged tasks is marked "no tasks".
3. **Given** unchecked tasks, **When** I open the spec, **Then** the first unchecked task in file order is highlighted as "Next".
4. **Given** the spec header, **When** I open the spec, **Then** I see the title, input summary, created date, branch, and ClickUp link.

---

### User Story 3 - Watch progress live (Priority: P1)

While an agent runs `/speckit-implement`, I keep the app open and watch tasks check off without refreshing.

**Why this priority**: Live feedback is what makes the app worth leaving open, and it costs little to build in from the start.

**Independent Test**: Check off a task in `tasks.md` with an editor. The progress bar and the "Next" task update within 2 seconds.

**Acceptance Scenarios**:

1. **Given** the app is showing a project, **When** any file under `specs/` or `.specify/` changes, **Then** the affected views update without user action.
2. **Given** a file is mid-write or briefly invalid, **When** the app reparses it, **Then** it keeps showing the last good state and does not flash errors.
3. **Given** a new feature folder appears, **When** it gets a `spec.md`, **Then** it appears in the project view.

---

### User Story 4 - Review the decisions made for me (Priority: P2)

I see every clarification question and answer, labelled by who decided it (human, agent, or flagged conflict), so I can challenge decisions the pipeline made on its own.

**Why this priority**: In proj-nava, most clarifications are resolved by the pipeline without a human ("Clarify pass (no human input)", "Analyst defaults"). This is the main place hidden risk lives. It is P2 only because P1 has to exist first.

**Independent Test**: Open a spec with an "Analyst defaults" session. Its questions show an "Agent" badge, separate from "Human-locked" sessions.

**Acceptance Scenarios**:

1. **Given** `## Clarifications` with `### Session <date>: <label>` subsections, **When** I open the Decisions pane, **Then** I see the sessions in order, each with a kind badge and its `Q: → A:` pairs.
2. **Given** the project view, **When** I filter to "Agent decisions", **Then** I see a count of agent-made and flagged decisions per spec.
3. **Given** real `[NEEDS CLARIFICATION: …]` markers in a spec, **When** I open it, **Then** they appear as open questions, and the project view shows a count. Text that only mentions the marker name (for example "zero `[NEEDS CLARIFICATION]` markers") is not counted.

---

### User Story 5 - See review health (Priority: P2)

I see the analyze or review verdict and the findings sorted by severity, so I know whether a spec is safe to build on.

**Why this priority**: `review.md` is the strongest quality signal in proj-nava (68 reviews, PASS or FAIL verdicts, severity tables).

**Independent Test**: Open `27450-shared-sign-in-host`. The Review pane shows pass 3 of 3 (post-implementation REVIEW, PASS) with its 10 open findings. Switching to pass 1 shows the ANALYZE FAIL and finding A-01.

**Acceptance Scenarios**:

1. **Given** `review.md`, **When** I open the spec, **Then** I see the current verdict. When the file states a "Current verdict", that takes precedence over earlier verdicts.
2. **Given** findings tables with a Severity column, **When** I open the Review pane, **Then** findings are listed by severity (CRITICAL, HIGH, MEDIUM, LOW, INFO) with ID, owner, finding, and fix, whatever the column order.
3. **Given** the latest review pass has a FAIL verdict, **When** I look at the project view, **Then** that spec appears in "Waiting on me".
4. **Given** `review.md` has several passes, **When** I open the Review pane, **Then** it shows the latest pass, and I can switch to earlier passes.

---

### User Story 6 - Read any artifact in place (Priority: P2)

I read `spec.md`, `plan.md`, `research.md`, `data-model.md`, `quickstart.md`, `contracts/*`, `checklists/*`, `review.md`, and `deployment.md` without leaving the app, rendered well, including Mermaid diagrams and tables.

**Why this priority**: The derived views are summaries. Sometimes I need the source.

**Independent Test**: Open the Plan tab of any planned spec. Headings, tables, code blocks, and Mermaid diagrams render.

**Acceptance Scenarios**:

1. **Given** a spec folder, **When** I open the Documents area, **Then** I see a tab for each artifact that exists, and none for missing ones.
2. **Given** an unknown extra file in the folder (for example `todo.md`, `notes.md`, `adrs/`), **When** I open Documents, **Then** it appears under "Other".

---

### User Story 7 - Hand off to my tools (Priority: P3)

From any spec I can open the folder in my editor, reveal it in Finder, open the ClickUp ticket, or copy the next Spec Kit command for that spec.

**Why this priority**: These keep the app read-only while making it useful in the workflow.

**Acceptance Scenarios**:

1. **Given** a spec at stage Clarified, **When** I choose "Copy next command", **Then** the clipboard holds `/speckit-plan` with the feature folder as context.
2. **Given** a spec with a ClickUp link, **When** I choose "Open ticket", **Then** the link opens in the browser.

---

### User Story 8 - Switch between projects (Priority: P3)

I keep several Spec Kit repos in a sidebar and switch between them.

**Acceptance Scenarios**:

1. **Given** I have opened repos before, **When** I launch the app, **Then** they appear in a recent-projects list and the last one reopens.

---

### User Story 9 - See specs over time (Priority: P3)

I see specs on a timeline, each bar running from its Created date to its merge (or to today while in flight), so I can see what overlapped, how long things took, and what has gone quiet. Selecting a bar shows what it builds on and what refers to it.

**Why this priority**: It answers "how is work flowing" rather than "where does each spec stand". Useful, but the P1 views come first.

**Independent Test**: Open the Timeline with "Show done" off. The 18 in-flight specs appear, each running to the today line, with hatching for the time since their last commit. Select `25195-org-authz-workspace-scope` with "Show done" on: arrows connect it to the specs it references and the specs that reference it.

**Acceptance Scenarios**:

1. **Given** specs with a `**Created**:` date, **When** I open the Timeline, **Then** each bar starts at that date (falling back to the first commit that added the folder) and ends at its last commit, or at today when the spec is in flight.
2. **Given** an in-flight spec whose last commit is days old, **When** I view it, **Then** the time since that commit is hatched and labelled "idle Nd".
3. **Given** "Show done" is off, **When** I open the Timeline, **Then** only in-flight specs show. Turned on, done specs from the last 120 days show too.
4. **Given** I select a bar, **When** it references other tickets or is referenced by them, **Then** arrows connect the related bars, unrelated bars fade, and a detail strip lists the links.
5. **Given** I switch between Week, Month, and Quarter, **Then** the scale changes and the view keeps today in sight.

---

### Edge Cases

- A feature folder has no `spec.md` (8 in proj-nava, for example `008-cosmetics` with only `notes.md`). Show it as a "Fragment" outside the pipeline.
- A spec has no `tasks.md` but its Status says "Implemented" (several in proj-nava, for example `24945-assets-edit`). See clarification marker 1.
- A phase is deliberately deferred (`## Phase 3: Deferred — …` in `26283-dam-adapter-pattern`). Its open tasks are not "stalled" work and should not count against the spec as if they were.
- Clarification lines are numbered (`- Q1: … → A: …`), not only `- Q: … → A: …`. Session labels may sit in parentheses (`### Session 2026-11-05 (human decisions)`).
- Task lines vary: `[x]` and `[X]`, bold IDs (`**T012**`), custom tags (`[DG-1]`), tasks outside any `## Phase` heading.
- `tasks.md` has non-phase sections (`## Format`, `## Dependencies`, `## MVP scope`, `## Requirement to task map`) that look like content but hold no tasks.
- User story headings vary: `(Priority: P1)`, `(P1)`, `### US3 …`, extra parentheses in the title.
- A review has several passes, and later passes repeat or resolve earlier finding IDs.
- The `**Status**:` header is free text and often stale (33 specs say "Draft", most of them finished). It is shown as a note, never used as the stage.
- Two folders share a ticket number.
- The repo is very large, or `specs/` is missing. The app says what it expected to find.

## Requirements *(mandatory)*

### Functional Requirements

**Reading**

- **FR-001**: The app MUST open any local folder and recognise it as a Spec Kit project when it contains `specs/` or `.specify/`.
- **FR-002**: The app MUST NOT write to, create, or delete anything in the project folder.
- **FR-003**: The app MUST treat each direct subfolder of `specs/` as one feature. A leading number before the first `-` is the ticket; the rest is the slug.
- **FR-004**: The app MUST read the spec header lines (`**Key**: value`) for title, status, feature branch, created date, input, and ClickUp link.
- **FR-005**: The app MUST tolerate format variations and degrade gracefully. When it cannot parse a section, it MUST still show the raw rendered document.

**Stage**

- **FR-010**: The app MUST derive stage from artifacts, in this order of checks: no `spec.md` → Fragment; all tasks done → Done; some tasks done → Building; no `tasks.md` and the Status line starts with Implemented/Delivered/Shipped/Done/Complete → Done, or with "In progress" → Building; tasks plus `review.md` → Analyzed; tasks → Tasked; `plan.md` → Planned; clarification sessions → Clarified; otherwise Specified.
- **FR-011**: The app MUST show the free-text Status line alongside the derived stage. When the stage came from the Status line (FR-010), the app MUST mark it as such.
- **FR-012**: The app MUST treat a phase whose title starts with "Deferred" as deferred. Open tasks in deferred phases do not make a spec "stalled".

**Tasks and stories**

- **FR-020**: The app MUST parse task lines of the form `- [ ] T001 [P] [US1] text`, accepting `x` or `X`, bold IDs, and any number of bracketed tags.
- **FR-021**: The app MUST group tasks under `## Phase N: …` headings and ignore other `##` sections. Tasks outside any phase go to "Other".
- **FR-022**: The app MUST parse user story headings in the variants listed in Edge Cases and link stories to tasks through `[US#]` tags.
- **FR-023**: The app MUST identify the next task as the first unchecked task in file order.

**Decisions and open questions**

- **FR-030**: The app MUST parse `## Clarifications` → `### Session …` → `- Q: … → A: …`, also accepting `->`, numbered `Q1:`/`A1:`, and bold `**Q**:`.
- **FR-031**: The app MUST classify each session as Human, Agent, Flagged, or Unattributed from its heading label, matching whole words and checking in this order: flagged/conflict → Flagged; "no human" → Agent; human, lock(ed), product decision, accepted, binding → Human; analyst default, agent, clarify pass → Agent; otherwise Unattributed. (Substring matching misreads "blocking" as "lock".)
- **FR-032**: The app MUST count only `[NEEDS CLARIFICATION: <text>]` (with colon and text) as an open question, and MUST ignore markers inside inline code or code blocks, which only mention the syntax.

**Review**

- **FR-040**: The app MUST split `review.md` into passes. A pass starts at a `##` section that states a verdict; sections without one (for example `## Findings`) belong to the pass before them. Pass names such as "first pass" or "second pass" MUST NOT be read as a PASS verdict.
- **FR-041**: The app MUST show the latest pass by default, label its gate (Analyze before implementation, Review after), and let me switch to earlier passes.
- **FR-042**: The app MUST read findings from any markdown table that has a Severity column, mapping columns by header name, and MUST normalize severity words (blocker → CRITICAL, major → HIGH, moderate → MEDIUM, minor → LOW, trivial/nit → INFO).
- **FR-043**: The app MUST treat a finding as resolved when its Status column starts with resolved, closed, fixed, accepted, addressed, done, verified, or n/a. Resolved findings are hidden by default and can be shown.

**Liveness and navigation**

- **FR-050**: The app MUST update within 2 seconds of a change to any file under `specs/` or `.specify/`.
- **FR-051**: The app MUST show `.specify/feature.json` as the current feature when it exists.
- **FR-052**: The app MUST support filtering by text and by stage, and grouping by stage or by ticket.
- **FR-057**: The app MUST offer a Timeline view as described in User Story 9, using `**Created**:` (or the first commit adding the folder) as the start and the last commit touching the folder as the end.
- **FR-058**: The app MUST derive links between specs from ticket numbers (`WB-12345` or `12345`) in `spec.md` that match another feature folder's ticket. Squash-merged history makes per-day commit activity unreliable, so the Timeline MUST NOT depend on it.
- **FR-055**: The app MUST offer a "Waiting on me" view: specs (including Done specs) with open `[NEEDS CLARIFICATION: …]` markers, a FAIL verdict in the latest review pass, unresolved HIGH or CRITICAL findings in the latest pass, or flagged-conflict decision sessions on in-flight specs.
- **FR-056**: The app MUST offer an "Unfinished" view: in-flight specs that are not fully planned (Specified, Clarified, Planned) or not fully built (Tasked, Analyzed, Building), marked "Stalled" after 14 days without a commit unless all open tasks are deferred.
- **FR-053**: The app MUST remember recently opened projects.
- **FR-054**: The app SHOULD offer open in editor, reveal in Finder, open ticket link, and copy next command.

### Key Entities

- **Project**: a repo folder. Has features, the current feature (from `feature.json`), and the constitution.
- **Feature**: one folder under `specs/`. Has ticket, slug, header fields, stage, and artifacts.
- **Phase / Task**: an ordered group and its checkbox items with ID, done state, parallel flag, story tag, and other tags.
- **User Story**: number, title, priority, linked tasks.
- **Decision Session**: date, label, kind (Human, Agent, Flagged, Unattributed), question-and-answer pairs.
- **Open Question**: a `[NEEDS CLARIFICATION: …]` marker and where it appears.
- **Review**: verdict and findings (ID, severity, owner, status, text, fix).
- **Checklist**: name and done/total items.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Opening proj-nava (104 folders) shows the project view in under 2 seconds on a recent Mac.
- **SC-002**: Task counts match a direct count of checkbox task lines for 100% of `tasks.md` files in proj-nava.
- **SC-003**: Zero false-positive open questions in proj-nava (today it has no real markers, so the count must be 0).
- **SC-004**: From launch, I can name the next task of the current feature in under 10 seconds.
- **SC-005**: A task checked off in an editor is reflected in the app within 2 seconds.
- **SC-006**: Every artifact file in a feature folder is reachable from the spec view.

## Assumptions

- The user runs macOS. The app is Mac-only.
- Projects are local git checkouts. No network access is needed except to open links.
- The proj-nava conventions (extra artifacts `review.md` and `deployment.md`, ticket-number folders, `feature.json`, session-labelled clarifications) are the primary target. Upstream Spec Kit templates must also work, since they are a subset.
- One person uses the app. No accounts, sync, or sharing in v1.
- Visual direction (2026-11-08): dark-first with a light variant; low-contrast layered surfaces; one violet accent; a single gradient reserved for the feature currently being worked on; a small stage-colour stripe on each row and bar; hatching for idle or off time; a white primary button. Native macOS chrome and system type.

## Out of Scope (v1)

- Editing any file, checking off tasks, or answering questions in the app.
- Launching agent commands from the app (only copying them).
- Reading specs from branches that are not checked out. Planned for v2: list `feature/*` and `plan/*` branches and read their `specs/` through git without checking out.
- Requirement-level traceability (FR/SC to tasks), beyond counts.
- Windows and Linux.
- Team distribution (signing, notarization, auto-update).

## Clarifications

### Session 2026-11-08: Brainstorm decisions (human)

- Q: Who is the app for? → A: Just me for now. I may share it with the team if it proves useful.
- Q: Read-only or interactive? → A: Read-only to begin.
- Q: Which Spec Kit setup? → A: "Stock". In practice, proj-nava extends it (see Assumptions), and the app targets those extensions.
- Q: Platform? → A: A standalone macOS app.

- Q: Stage for specs without `tasks.md` whose Status says implemented? → A: Trust the Status line. In proj-nava this moves 9 specs to Done and 3 to Building (FR-010, FR-011).
- Q: Findings across review passes? → A: Show the latest pass by default, with earlier passes available as an option (FR-040, FR-041).
- Q: Unattributed clarification sessions? → A: Show them as Unattributed.
- Q: What puts a spec in "Needs attention"? → A: Unsure. Instinct: specs not fully planned, or partially implemented.

### Session 2026-11-08: Attention model (proposed by agent, confirmed by human)

- Q: How to turn "not fully planned or partially implemented" into views? → A: Split "Needs attention" in two. "Unfinished" covers the human's two cases (FR-056). "Waiting on me" covers things only a human can unblock (FR-055). In proj-nava today: Unfinished = 18 specs, Waiting on me = 2 (both Done specs whose latest review still has an unresolved HIGH finding or a FAIL).

- Q: Confirm the split into "Waiting on me" and "Unfinished" (FR-055, FR-056), and should Done specs whose latest review still has a FAIL or open HIGH finding count as "Waiting on me"? → A: Yes to both.
