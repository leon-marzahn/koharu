import type { TFunction } from 'i18next'

import type { PageSummary, Scope } from '@koharu/bridge/protocol'
import { toast } from '@koharu/ui/components/toast'

/**
 * Bulk runs ("All pages", "Retranslate stale", "Retranslate all") leave Done pages alone.
 * `only` narrows the candidates (the stale pages). Explicit selections never come through
 * here, so they still run Done pages.
 */
export function bulkScope(pages: PageSummary[], only?: string[]) {
  const candidates = only ? pages.filter((page) => only.includes(page.id)) : pages
  const runnable = candidates.filter((page) => page.status !== 'done')
  const skipped = candidates.length - runnable.length
  const scope: Scope | null =
    skipped === 0 && !only
      ? { scope: 'project' }
      : runnable.length > 0
        ? { scope: 'pages', value: runnable.map((page) => page.id) }
        : null
  return { scope, skipped }
}

export function runBulk(
  pages: PageSummary[],
  run: (scope: Scope) => void,
  t: TFunction,
  only?: string[],
) {
  const { scope, skipped } = bulkScope(pages, only)
  if (skipped > 0)
    toast.add({ type: 'info', title: t('navigator.skippedDone', { count: skipped }) })
  if (scope) run(scope)
}
