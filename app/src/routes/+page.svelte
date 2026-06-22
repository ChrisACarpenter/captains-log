<main>
  <section class="welcome">
    <h1>Captain's Log</h1>
    <p class="lead">Weekly work journal that makes self-reviews painless.</p>

    <div class="card">
      <h2>You're in the main window.</h2>
      <p>
        This space will become the journal browser — year/week sidebar, past
        notes, weekly summaries. That's coming in Phase 2.
      </p>
      <p>
        For now, capture a note by clicking the
        <strong>book icon in your menu bar</strong> (top of the screen).
      </p>
    </div>
  </section>
</main>

<style>
  main {
    display: flex;
    justify-content: center;
    padding: var(--space-12) var(--space-4);
    min-height: 100vh;
  }

  .welcome {
    max-width: 560px;
    width: 100%;
  }

  h1 {
    margin-bottom: var(--space-3);
  }

  .lead {
    color: var(--text-secondary);
    margin-bottom: var(--space-12);
  }

  .card {
    padding: var(--space-6);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
  }

  .card h2 {
    margin-bottom: var(--space-3);
  }

  .card p {
    color: var(--text-secondary);
    margin-bottom: var(--space-3);
  }

  .card p:last-child {
    margin-bottom: 0;
  }

  strong {
    color: var(--text-primary);
  }
</style>
