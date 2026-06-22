<script lang="ts">
  // Placeholder welcome screen. The real main journal window lives here
  // (or under /journal) once Phase 1 capture works.
</script>

<main>
  <section class="hero">
    <h1>Captain's Log</h1>
    <p class="subtitle">Weekly work journal that makes self-reviews painless.</p>

    <div class="actions">
      <button class="btn btn-emerald">Capture a note</button>
      <button class="btn btn-marble">Open journal</button>
    </div>

    <p class="footnote">
      Phase 1 scaffold. Real capture and journal flows land next.
    </p>
  </section>
</main>

<style>
  main {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: var(--space-8);
  }

  .hero {
    max-width: 520px;
    text-align: center;
  }

  .subtitle {
    margin-top: var(--space-3);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    color: var(--text-secondary);
  }

  .actions {
    margin-top: var(--space-8);
    display: flex;
    justify-content: center;
    gap: var(--space-3);
  }

  .footnote {
    margin-top: var(--space-12);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-muted);
  }
</style>
