<!--
  Step 3 — Tell me about your manager.

  Two optional fields: name + email. Both feed the Phase 2.6 Send-to-
  Manager flow on /summary; without them, the Send button stays
  disabled but nothing else suffers.

  Mirrors StepAboutYou's structure for visual rhythm.
-->
<script lang="ts">
  import StepHeader from './StepHeader.svelte';
  import TipBubble from './TipBubble.svelte';
  import InputField from '$lib/InputField.svelte';
  import PointerFinger from '$lib/PointerFinger.svelte';

  type Props = {
    managerName: string;
    managerEmail: string;
  };

  let {
    managerName = $bindable(),
    managerEmail = $bindable(),
  }: Props = $props();

  // First field whose trim()ed value is still empty. Pointer attaches
  // to whichever the result names; once both are filled, both rows go
  // hidden.
  const nextUnfilledId = $derived(
    managerName.trim() === '' ? 'ob-manager-name' :
    managerEmail.trim() === '' ? 'ob-manager-email' :
    null
  );
</script>

<section class="step">
  <StepHeader
    title="Tell me about your manager."
    lead="Personalizes the weekly email greeting and pre-fills the To: field. Leave blank to send to whoever you like at the time."
  />

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-manager-name'} />
    <InputField
      id="ob-manager-name"
      label="Their name"
      placeholder="Arthur"
      bind:value={managerName}
    />
  </div>

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-manager-email'} />
    <InputField
      id="ob-manager-email"
      label="Their email"
      type="email"
      placeholder="arthur.manager@prodigygame.com"
      bind:value={managerEmail}
    />
  </div>

  <TipBubble heading="Tip">
    Used to personalize the <strong>Send weekly summary</strong>
    button on the /summary screen — name in the greeting, email in the
    To: field. Skip this step and the Send button still works; it'll
    open a draft with a generic greeting and a blank To: line for you
    to fill in.
  </TipBubble>
</section>

<style>
  /* Field-trio styles live in <InputField>. Step uses flex-gap for
     inter-child spacing (was per-field margin-bottom; doesn't reach
     into InputField under Svelte scoping).

     .guide-row layout (pointer + field) lives in Wizard.svelte as a
     shared :global() rule. */
  section.step {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
</style>
