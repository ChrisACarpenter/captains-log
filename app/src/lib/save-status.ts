/**
 * Shared 5-state auto-save status union used by routes with an
 * auto-save loop against `write_week` / `create_note` / etc.:
 *
 *   - `idle`    — settled, nothing dirty and nothing to advertise
 *   - `dirty`   — user typed something; the autoSaveTimer is armed
 *   - `saving`  — invoke in flight; retries and cross-route ExternalUpdate
 *                 events check this to avoid clobbering an in-progress write
 *   - `saved`   — last write succeeded; UI shows the timestamp
 *   - `error`   — last write threw; UI shows a Retry affordance
 *
 * Consolidated here so `/journal`, `/summary`, `/capture`, and the
 * shared `<SaveStatus>` component all speak the same vocabulary. Named
 * `AutoSaveStatus` (not `SaveStatus`) so importers don't collide with
 * the `<SaveStatus>` component's default-exported identifier.
 */
export type AutoSaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';
