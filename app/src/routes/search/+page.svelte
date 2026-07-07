<!--
  /search — full-text search across every Weekly Summary (Phase 3b Slice 1).

  Scope: MVP restricts search to Weekly Summary content only (not Note
  bodies). Optional label filter narrows results to summaries whose
  ### Labels subsection contains ≥ 1 of the selected labels. Results
  are grouped by (year, week) — one card per Summary that contains
  matches — with up to 3 snippets per card and a "…N more matches"
  footer when truncated.

  Reachable from the /journal sidebar (Search button below the Journal
  header) — no other entry points yet (landing-page + Cmd+K deferred
  to a possible Slice 3 polish pass).

  Result click → goto('/journal?year=Y&week=W'). Since Slice 1 targets
  Summary content only and the Summary block always sits at the top of
  a weekly file, no scroll-to-position plumbing is needed for MVP —
  clicking lands the user at the top of the target week with the
  Summary already visible. Slice 2 adds scroll-to-note if we expand
  scope to Note bodies later.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import LabelInput from '$lib/LabelInput.svelte';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';

  // Match the Rust SearchResult / SearchSnippet shape (serde rename_all
  // = "camelCase" on both structs; the kind enum uses lowercase).
  type SearchSnippet = { snippet: string };
  type SearchResult = {
    year: number;
    week: number;
    kind: 'summary' | 'note';
    labels: string[];
    /** Populated for Note results only; null for Summary results. */
    noteTimestamp: string | null;
    /** Populated for Note results with an explicit title; null otherwise. */
    noteTitle: string | null;
    /** Byte offset for deep-link scroll-to. Summary = 0, Note = heading offset. */
    scrollOffset: number;
    snippets: SearchSnippet[];
    totalMatches: number;
  };

  // Matches the flat `SettingsBundle` struct (Rust serde rename_all
  // camelCase). We only read `colorfulLabels` here; other fields are
  // present but irrelevant to search.
  type SettingsBundle = {
    colorfulLabels: boolean;
  };

  // Same floor the Rust command enforces — kept in sync so the Search
  // button's disabled state matches the backend's short-query guard.
  const MIN_QUERY_LENGTH = 2;
  // Mirrors the Rust MAX_RESULTS constant. When the backend caps a
  // dense query, results.length equals this exactly and we surface a
  // "narrow your query" TipBubble so the user knows to refine.
  const MAX_RESULTS = 200;

  let query = $state('');
  let labelFilter = $state<string[]>([]);
  let results = $state<SearchResult[]>([]);
  let searching = $state(false);
  let searchError = $state('');
  // Guards the results panel — hidden entirely until the user runs at
  // least one search so the page doesn't shout "0 results" on mount.
  let hasSearched = $state(false);
  // Mirrored from settings so the LabelInput chip picker paints its
  // suggestions with the user's chosen coloring convention.
  let colorfulLabels = $state(false);

  onMount(async () => {
    try {
      const bundle = await invoke<SettingsBundle>('get_settings');
      colorfulLabels = bundle.colorfulLabels ?? false;
    } catch {
      // Non-fatal — search works without knowing the setting; chips
      // just fall back to the palette treatment.
    }
  });

  async function runSearch(e: Event): Promise<void> {
    e.preventDefault();
    const trimmed = query.trim();
    if (trimmed.length < MIN_QUERY_LENGTH) return;
    searching = true;
    searchError = '';
    try {
      results = await invoke<SearchResult[]>('search_journal', {
        query: trimmed,
        labelFilter,
      });
      hasSearched = true;
    } catch (err) {
      searchError = String(err);
      results = [];
    } finally {
      searching = false;
    }
  }

  // Split a snippet into alternating [plain, match, plain, match, …]
  // segments for highlighted rendering. Case-insensitive scan against
  // the current query. Kept simple — plain substring iteration.
  function highlightMatch(snippet: string): Array<{ text: string; isMatch: boolean }> {
    const trimmed = query.trim();
    if (trimmed.length < MIN_QUERY_LENGTH) {
      return [{ text: snippet, isMatch: false }];
    }
    const lowerSnippet = snippet.toLowerCase();
    const lowerQuery = trimmed.toLowerCase();
    const parts: Array<{ text: string; isMatch: boolean }> = [];
    let cursor = 0;
    while (cursor < snippet.length) {
      const idx = lowerSnippet.indexOf(lowerQuery, cursor);
      if (idx === -1) {
        parts.push({ text: snippet.slice(cursor), isMatch: false });
        break;
      }
      if (idx > cursor) {
        parts.push({ text: snippet.slice(cursor, idx), isMatch: false });
      }
      parts.push({
        text: snippet.slice(idx, idx + lowerQuery.length),
        isMatch: true,
      });
      cursor = idx + lowerQuery.length;
    }
    return parts;
  }

  async function openResult(r: SearchResult): Promise<void> {
    // Summary results scroll to top (offset 0 is a no-op for the
    // MarkdownEditor scroll effect, but we still include it for
    // consistency); Note results scroll to their heading byte offset.
    const url =
      r.scrollOffset > 0
        ? `/journal?year=${r.year}&week=${r.week}&scrollTo=${r.scrollOffset}`
        : `/journal?year=${r.year}&week=${r.week}`;
    await goto(url);
  }
</script>

<svelte:head>
  <title>Search — Captain's Log</title>
</svelte:head>

<main class="search-page">
  <header class="page-header">
    <h1>Search</h1>
    <p class="lead">
      Full-text search across every Weekly Summary and every Note you've
      written. Filter by label to narrow the surface. Reachable from
      any screen with <kbd>Cmd</kbd> + <kbd>K</kbd>.
    </p>
  </header>

  <form onsubmit={runSearch} class="search-form">
    <div class="field">
      <label for="query">Query</label>
      <input
        id="query"
        type="text"
        class="text-input"
        placeholder="Search…"
        bind:value={query}
        autocomplete="off"
        autocorrect="off"
        spellcheck="false"
      />
    </div>

    <div class="field">
      <!-- LabelInput is a composite component, not a single input, so
           we use <span class="field-heading"> instead of <label> —
           matches the pattern in /summary. -->
      <span class="field-heading">Filter by labels (optional)</span>
      <LabelInput
        bind:labels={labelFilter}
        placeholder="Add label filters…"
        {colorfulLabels}
      />
    </div>

    <div class="actions">
      <button
        type="submit"
        class="btn btn-emerald"
        disabled={searching || query.trim().length < MIN_QUERY_LENGTH}
      >
        {searching ? 'Searching…' : 'Search'}
      </button>
      <button
        type="button"
        class="btn btn-ruby"
        onclick={() => goto('/journal')}
      >
        Back to Journal
      </button>
    </div>
  </form>

  {#if searchError}
    <p class="status status-error">Couldn't search: {searchError}</p>
  {/if}

  {#if hasSearched && !searching}
    {#if results.length === 0}
      <p class="empty-state">
        No matches. Try a shorter or different query — or clear the
        label filter if you added one.
      </p>
    {:else}
      {#if results.length >= MAX_RESULTS}
        <TipBubble>
          Showing the first {MAX_RESULTS} matches (newest first). Your
          query hit a lot of surfaces — refine it (longer term, add a
          label filter) to see the rest.
        </TipBubble>
      {/if}
      <p class="result-count">
        {results.length} {results.length === 1 ? 'result' : 'results'}
        {#if results.length >= MAX_RESULTS}(capped){/if}
        — newest first.
      </p>
      <ul class="results" role="list">
        {#each results as r (`${r.year}-${r.week}-${r.kind}-${r.scrollOffset}`)}
          <li>
            <button
              type="button"
              class="result-card"
              onclick={() => void openResult(r)}
            >
              <div class="result-header">
                <span class="kind-badge" class:kind-badge-note={r.kind === 'note'}>
                  {r.kind === 'note' ? 'Note' : 'Summary'}
                </span>
                <span class="result-week">
                  {r.year}-W{String(r.week).padStart(2, '0')}
                </span>
                {#if r.kind === 'note' && r.noteTimestamp}
                  <span class="result-note-meta">
                    <span class="result-note-timestamp">{r.noteTimestamp}</span>
                    {#if r.noteTitle}
                      <span class="result-note-title">— {r.noteTitle}</span>
                    {/if}
                  </span>
                {/if}
                {#if r.labels.length > 0}
                  <span class="result-labels">
                    {#each r.labels as label (label)}
                      <span class="result-label">#{label}</span>
                    {/each}
                  </span>
                {/if}
                <span class="result-count-badge">
                  {r.totalMatches} match{r.totalMatches === 1 ? '' : 'es'}
                </span>
              </div>
              <ul class="result-snippets">
                {#each r.snippets as sn, i (i)}
                  <li>
                    {#each highlightMatch(sn.snippet) as part, j (j)}
                      {#if part.isMatch}
                        <mark>{part.text}</mark>
                      {:else}{part.text}{/if}
                    {/each}
                  </li>
                {/each}
              </ul>
              {#if r.totalMatches > r.snippets.length}
                <p class="more-matches">
                  … and {r.totalMatches - r.snippets.length} more match{r.totalMatches - r.snippets.length === 1 ? '' : 'es'} in this week
                </p>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</main>

<style>
  .search-page {
    max-width: 720px;
    margin: 0 auto;
    padding: var(--space-6) var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .page-header h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    line-height: var(--text-display-lg-lh);
  }
  .page-header .lead {
    margin: var(--space-2) 0 0;
    color: var(--text-secondary);
  }

  /* Form — three stacked rows (query, labels, actions). Fields use the
     app-wide .text-input utility so styling matches Settings, capture,
     onboarding. */
  .search-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .field label,
  .field-heading {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }
  .actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
  }

  .empty-state {
    color: var(--text-secondary);
    font-size: var(--text-body);
    text-align: center;
    padding: var(--space-6) var(--space-4);
    border: 1px dashed var(--border-structural);
    border-radius: var(--radius-md);
  }

  .result-count {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-caption);
    font-style: italic;
  }

  .results {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  /* Result card — an entire button so the whole thing is clickable
     with a single hit target. Card has visible focus + hover states so
     keyboard nav is obvious. */
  .result-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-3) var(--space-4);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-base), border-color var(--transition-base);
  }
  .result-card:hover {
    background: var(--bg-elevated);
    border-color: var(--accent-primary);
  }
  .result-card:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 2px;
  }

  .result-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .result-week {
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: var(--text-caption);
    color: var(--text-secondary);
  }

  /* Kind badge visuals live in app.css as the .kind-badge utility
     (shared with LabelDetailsModal's Referenced-In list). */

  /* Note metadata — timestamp + optional title, shown on Note results
     between the week label and the label chips. Monospace timestamp so
     the columns align across cards. */
  .result-note-meta {
    display: inline-flex;
    align-items: baseline;
    gap: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-caption);
    min-width: 0;
    overflow: hidden;
  }
  .result-note-timestamp {
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    white-space: nowrap;
  }
  .result-note-title {
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result-labels {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
    /* Push labels to fill the space between the week label and the
       match count. */
    flex: 1;
  }
  .result-label {
    font-family: var(--font-display);
    font-size: var(--text-caption);
    color: var(--accent-primary-text);
    padding: 1px var(--space-2);
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--accent-primary) 10%, var(--bg-elevated));
  }
  .result-count-badge {
    font-family: var(--font-display);
    font-size: var(--text-caption);
    color: var(--text-secondary);
    background: var(--bg-elevated);
    border: 1px solid var(--border-decorative);
    border-radius: var(--radius-pill);
    padding: 1px var(--space-2);
    white-space: nowrap;
    /* Always push the match-count badge to the far right, whether or
       not the result has any labels. When labels are present, the
       .result-labels container's flex:1 already does the work; when
       labels are absent, this margin picks up the slack so the badge
       column stays aligned across every card. */
    margin-left: auto;
  }

  .result-snippets {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .result-snippets li {
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    color: var(--text-primary);
  }

  /* <mark> is the native highlight element — accessible + carries
     semantic meaning. Restyle to match the app's accent palette
     instead of the browser's default (usually yellow-on-black which
     clashes with the theme). */
  mark {
    background: color-mix(in srgb, var(--accent-primary) 32%, transparent);
    color: var(--text-primary);
    padding: 0 2px;
    border-radius: 2px;
  }

  .more-matches {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-caption);
    font-style: italic;
  }
</style>
