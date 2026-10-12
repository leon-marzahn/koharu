'use client'

import { BookUser, Bot, SlidersHorizontal } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { AgentPanel } from '@/components/editor/AgentPanel'
import { Inspector } from '@/components/editor/Inspector'
import { ProjectPanel } from '@/components/editor/ProjectPanel'
import { useProject } from '@/lib/queries'
import { Button } from '@koharu/ui/components/button'

export function RightSidebar() {
  const { t } = useTranslation()
  const [panel, setPanel] = useState<'agent' | 'properties' | 'project'>('properties')
  const proposals = useProject().data?.proposals
  const pending = (proposals?.characters.length ?? 0) + (proposals?.terms.length ?? 0)

  return (
    <aside className='flex h-full min-h-0 flex-col bg-[var(--surface-panel)]'>
      <div className='flex h-10 shrink-0 items-center gap-1 border-b border-border/80 px-2.5'>
        <Button
          variant={panel === 'properties' ? 'secondary' : 'ghost'}
          size='sm'
          className='h-7 flex-1 text-[10px]'
          onClick={() => setPanel('properties')}
        >
          <SlidersHorizontal className='size-3' /> {t('agent.properties')}
        </Button>
        <Button
          variant={panel === 'project' ? 'secondary' : 'ghost'}
          size='sm'
          className='h-7 flex-1 text-[10px]'
          onClick={() => setPanel('project')}
        >
          <BookUser className='size-3' /> {t('project.title')}
          {pending > 0 && (
            <span className='rounded-full bg-primary px-1 text-[9px] text-primary-foreground'>
              <span aria-hidden='true'>{pending}</span>
              <span className='sr-only'>{t('project.pendingCount', { count: pending })}</span>
            </span>
          )}
        </Button>
        <Button
          variant={panel === 'agent' ? 'secondary' : 'ghost'}
          size='sm'
          className='h-7 flex-1 text-[10px]'
          onClick={() => setPanel('agent')}
        >
          <Bot className='size-3' /> {t('agent.title')}
        </Button>
      </div>
      <div className={panel === 'agent' ? 'flex min-h-0 flex-1 flex-col' : 'hidden'}>
        <AgentPanel />
      </div>
      <div className={panel === 'properties' ? 'min-h-0 flex-1' : 'hidden'}>
        <Inspector />
      </div>
      <div className={panel === 'project' ? 'min-h-0 flex-1' : 'hidden'}>
        <ProjectPanel />
      </div>
    </aside>
  )
}
