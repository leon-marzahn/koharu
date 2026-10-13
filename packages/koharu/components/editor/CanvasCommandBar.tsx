'use client'

import { InferenceControl } from '@/components/editor/InferenceControl'
import { call } from '@/lib/backend'
import { usePage } from '@/lib/queries'
import { useKoharuStore, type PipelineScope } from '@/lib/store'
import { commands, type Operation, type Scope } from '@koharu/bridge/protocol'

export function CanvasCommandBar() {
  const page = usePage().data
  const selectedPages = useKoharuStore((state) => state.selectedPages)
  const jobs = useKoharuStore((state) => state.jobs)
  const running = Object.values(jobs).find((job) => job.state === 'running')

  const run = (selection: PipelineScope, operation: Operation) => {
    if (!page) return
    const scope: Scope =
      selection === 'project'
        ? { scope: 'project' }
        : selection === 'selected-pages'
          ? { scope: 'pages', value: selectedPages }
          : { scope: 'pages', value: [page.id] }
    void call(commands.process, scope, operation).catch(() => undefined)
  }

  return (
    <header className='flex h-10 shrink-0 items-center gap-2 border-b border-border/80 bg-[var(--surface-toolbar)] px-2.5'>
      <div className='min-w-0 flex-1' />
      <InferenceControl disabled={!page || Boolean(running)} onRun={run} />
    </header>
  )
}
