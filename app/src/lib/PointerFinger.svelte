<!--
  PointerFinger — onboarding guide that points at the next unfilled
  field. Asset is /branded/guide-hand.png. Bobs gently via a CSS
  keyframe so the user's eye is drawn to it.

  Props:
    hidden?: boolean — when true, the component renders an empty
                       spacer to keep flex layout stable. Default
                       false. Callers gate visibility based on their
                       own per-step nextUnfilledId logic.
-->
<script lang="ts">
  type Props = { hidden?: boolean };
  let { hidden = false }: Props = $props();
</script>

<span class="pointer-finger" class:hidden aria-hidden="true">
  <img src="/branded/guide-hand.png" alt="" width="32" height="32" />
</span>

<style>
  .pointer-finger {
    display: inline-flex;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    /* Rotate so the finger points right at the adjacent input. The
     * branded sprite ships pointing down-right by default. */
    transform: rotate(-15deg);
    animation: pointer-bob 1.6s var(--ease-oscillate) infinite;
  }
  .pointer-finger.hidden {
    visibility: hidden;
  }
  .pointer-finger img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  @keyframes pointer-bob {
    0%, 100% { transform: rotate(-15deg) translateX(0); }
    50%      { transform: rotate(-15deg) translateX(4px); }
  }
</style>
