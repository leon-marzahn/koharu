'use client'

import { useTranslation } from 'react-i18next'

import { InferenceControl } from '@/components/editor/InferenceControl'
import { call } from '@/lib/backend'
import { runBulk } from '@/lib/bulk'
import { usePage, usePages } from '@/lib/queries'
import { useKoharuStore, type PipelineScope } from '@/lib/store'
import { commands, type Operation, type Scope } from '@koharu/bridge/protocol'

export function CanvasCommandBar() {
  const { t } = useTranslation()
  const page = usePage().data
  const pages = usePages().data ?? []
  const selectedPages = useKoharuStore((state) => state.selectedPages)
  const jobs = useKoharuStore((state) => state.jobs)
  const running = Object.values(jobs).find((job) => job.state === 'running')

  const run = (selection: PipelineScope, operation: Operation) => {
    if (!page) return
    const start = (scope: Scope) =>
      void call(commands.process, scope, operation).catch(() => undefined)
    if (selection === 'project') runBulk(pages, start, t)
    else
      start({ scope: 'pages', value: selection === 'selected-pages' ? selectedPages : [page.id] })
  }

  return (
    <header className='flex h-10 shrink-0 items-center gap-2 border-b border-border/80 bg-[var(--surface-toolbar)] px-2.5'>
      <div className='min-w-0 flex-1' />
      <InferenceControl disabled={!page || Boolean(running)} onRun={run} />
    </header>
  )
}
