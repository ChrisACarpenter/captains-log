<!--
  Step 2 — Tell me about you.

  Three optional fields: display name, Bamboo title, Jira project keys.
  All blank-OK. Each field is bound via $bindable into the parent
  Wizard's state so the values survive Back/Continue navigation.

  The "Bamboo" word in the field label links out to Prodigy's BambooHR
  so the user can copy their exact title verbatim. Opens in the system
  browser via `tauri-plugin-opener` (already a project dependency).

  Jira keys come in as a single comma-separated string from the user
  (e.g. "MAGE, LIVE, FENIX"). The backend tokenizes + uppercases on
  save; we don't pre-validate in the wizard — gates on optional fields
  feel hostile.
-->
<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import StepHeader from './StepHeader.svelte';
  import TipBubble from './TipBubble.svelte';
  import InputField from '$lib/InputField.svelte';
  import PointerFinger from '$lib/PointerFinger.svelte';

  type Props = {
    name: string;
    userEmail: string;
    bambooTitle: string;
    jiraKeys: string;
  };

  let {
    name = $bindable(),
    userEmail = $bindable(),
    bambooTitle = $bindable(),
    jiraKeys = $bindable(),
  }: Props = $props();

  // First field whose trim()ed value is still empty. The pointer
  // attaches to whichever field this resolves to; once everything is
  // filled, every per-row PointerFinger goes hidden.
  const nextUnfilledId = $derived(
    name.trim() === '' ? 'ob-name' :
    userEmail.trim() === '' ? 'ob-user-email' :
    bambooTitle.trim() === '' ? 'ob-bamboo-title' :
    jiraKeys.trim() === '' ? 'ob-jira-keys' :
    null
  );

  // Prodigy's BambooHR tenant. Externalized as a const so it's easy to
  // grep if the URL ever shifts (BambooHR tenant URLs are stable in
  // practice, but the company-name slug isn't part of any other
  // configuration so it deserves a one-line home here).
  const BAMBOO_URL = 'https://prodigyeducation.bamboohr.com/';

  function openBamboo(e: Event): void {
    e.preventDefault();
    openUrl(BAMBOO_URL).catch((err) => {
      console.error('[onboarding] bamboo opener failed:', err);
    });
  }
</script>

<section class="step">
  <StepHeader
    title="Tell me about you."
    lead="Helps personalize the app. Anything you skip you can always set later in Settings."
  />

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-name'} />
    <InputField
      id="ob-name"
      label="What should we call you?"
      placeholder="Chris"
      bind:value={name}
    />
  </div>

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-user-email'} />
    <InputField
      id="ob-user-email"
      label="Your email"
      type="email"
      placeholder="you@prodigygame.com"
      hint="Used as the sender in Mail.app mode and to route the right Gmail account."
      bind:value={userEmail}
      autocomplete="email"
    />
  </div>

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-bamboo-title'} />
    <InputField
      id="ob-bamboo-title"
      labelSnippet={bambooLabel}
      placeholder="Staff QA Analyst"
      bind:value={bambooTitle}
    />
  </div>

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-jira-keys'} />
    <InputField
      id="ob-jira-keys"
      label="Your Jira project key(s)"
      placeholder="MAGE, LIVE"
      bind:value={jiraKeys}
      hint="Comma-separated. Captain's Log will uppercase them."
    />
  </div>

  {#snippet bambooLabel()}
    Your job title — the one on
    <!-- svelte-ignore a11y_invalid_attribute -->
    <a href="#" onclick={openBamboo}>Bamboo</a>
  {/snippet}

  <TipBubble heading="Tip">
    These show up later in features like the weekly
    <strong>Send-to-Manager</strong> email (your title in the signature) and
    link enrichment for Jira tickets. Leave anything blank if you don't want
    to share — none of it leaves your machine until you press Send.
  </TipBubble>
</section>

<style>
  /* Field-trio (.field + label + .field-hint) styles live in
     <InputField>. Inter-field spacing is driven by the step's own flex
     gap — earlier shape relied on .field's margin-bottom which doesn't
     reach InputField under Svelte scoping.

     .guide-row layout (pointer + field) lives in Wizard.svelte as a
     shared :global() rule so the three form steps don't drift. */
  section.step {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
</style>
